use std::{collections::HashMap, marker::PhantomData, sync::Arc, time::Duration};

use async_trait::async_trait;
use isahc::{
    http::{Method, StatusCode},
    AsyncReadResponseExt,
};
use serde::{de::DeserializeOwned, ser::SerializeSeq, Deserialize, Serialize};
use tokio::{sync::Mutex, time::Instant};

#[async_trait]
pub trait Request<C = Bot>
where
    Self: Sized + Send,
    C: ?Sized + Sync,
{
    type Output;

    async fn request_weak(self, client: &C) -> Result<Self::Output>;
    async fn request(self, client: &C) -> Result<Self::Output>;
}

pub struct HttpRequest<T, C = Bot>
where
    T: DeserializeOwned,
    C: Client + ?Sized,
{
    phantom: PhantomData<fn(&C) -> T>,
    pub method: Method,
    pub uri: String,
    pub body: Option<String>,
    pub files: Vec<Arc<File>>,
}

#[async_trait]
impl<T, C> Request<C> for HttpRequest<T, C>
where
    T: DeserializeOwned,
    C: Client + ?Sized,
{
    type Output = T;

    async fn request_weak(self, client: &C) -> Result<T> {
        client
            .request_weak(self.method, &self.uri, self.body.as_deref(), &self.files)
            .await
    }
    async fn request(self, client: &C) -> Result<T> {
        client
            .request(self.method, &self.uri, self.body.as_deref(), &self.files)
            .await
    }
}

#[derive(Debug)]
pub enum RequestError {
    // https error, authentication error (these errors should just kill the bot)
    Authorization,

    // network error, timeout (these errors could just be retried)
    Network,

    // 429 response
    RateLimited,

    // 4xx reponse
    ClientError(StatusCode),

    // 5xx response, unexpected response
    ServerError,

    // gateway error
    InvalidSession,
}

pub type Result<T> = ::std::result::Result<T, RequestError>;

impl<T, C> HttpRequest<T, C>
where
    T: DeserializeOwned,
    C: Client + ?Sized,
{
    pub fn get<S>(uri: S) -> Self
    where
        S: Into<String>,
    {
        HttpRequest {
            phantom: PhantomData,
            method: Method::GET,
            uri: uri.into(),
            body: None,
            files: Vec::new(),
        }
    }

    pub fn post<S>(uri: S, body: &impl Serialize) -> Self
    where
        S: Into<String>,
    {
        HttpRequest {
            phantom: PhantomData,
            method: Method::POST,
            uri: uri.into(),
            body: Some(serde_json::to_string(body).unwrap()),
            files: Vec::new(),
        }
    }

    pub fn post_attached<S>(uri: S, body: &(impl Serialize + Attachments)) -> Self
    where
        S: Into<String>,
    {
        HttpRequest {
            phantom: PhantomData,
            method: Method::POST,
            uri: uri.into(),
            body: Some(serde_json::to_string(body).unwrap()),
            files: body.attachments(),
        }
    }

    pub fn patch<S>(uri: S, body: &impl Serialize) -> Self
    where
        S: Into<String>,
    {
        HttpRequest {
            phantom: PhantomData,
            method: Method::PATCH,
            uri: uri.into(),
            body: Some(serde_json::to_string(body).unwrap()),
            files: Vec::new(),
        }
    }

    pub fn patch_attached<S>(uri: S, body: &(impl Serialize + Attachments)) -> Self
    where
        S: Into<String>,
    {
        HttpRequest {
            phantom: PhantomData,
            method: Method::PATCH,
            uri: uri.into(),
            body: Some(serde_json::to_string(body).unwrap()),
            files: body.attachments(),
        }
    }

    pub fn delete<S>(uri: S) -> Self
    where
        S: Into<String>,
    {
        HttpRequest {
            phantom: PhantomData,
            method: Method::DELETE,
            uri: uri.into(),
            body: None,
            files: Vec::new(),
        }
    }
}

struct DiscordRateLimits {
    request_rate: f32,
    last_request: Instant,
    retry_after: Instant,

    buckets: HashMap<String, RateLimit>,
    bucket_cache: HashMap<String, String>,
}

#[derive(Clone)]
pub struct Bot {
    token: String,
    limits: Arc<Mutex<DiscordRateLimits>>,
}

struct RateLimit {
    remaining: u64,
    reset_at: Instant,
}

#[derive(Deserialize)]
struct RateLimitResponse {
    retry_after: f64,
}

const GLOBAL_RATE_LIMIT: f32 = 45.0;

impl DiscordRateLimits {
    fn inc_request(&mut self) {
        let now = Instant::now();
        let diff = now.duration_since(self.last_request).as_secs_f32();
        self.request_rate = (self.request_rate + 1.0) / (diff + 1.0);
        self.last_request = now;
    }
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub struct Indexed<T>(pub Vec<T>);

#[derive(Debug)]
pub struct IndexedOr<T, E>(pub Vec<T>, pub Vec<E>);

impl<T> Indexed<T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T> From<Vec<T>> for Indexed<T> {
    fn from(value: Vec<T>) -> Self {
        Self(value)
    }
}

impl<T> Default for Indexed<T> {
    fn default() -> Self {
        Self(Vec::default())
    }
}

impl<T, E> Default for IndexedOr<T, E> {
    fn default() -> Self {
        Self(Vec::default(), Vec::default())
    }
}

#[derive(Serialize)]
struct WithIndex<'a, T> {
    id: usize,
    #[serde(flatten)]
    elem: &'a T,
}

impl<T, E> Serialize for IndexedOr<T, E>
where
    T: Serialize,
    E: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.0.len() + self.1.len()))?;
        for (id, elem) in self.0.iter().enumerate() {
            seq.serialize_element(&WithIndex { id, elem })?;
        }
        for elem in self.1.iter() {
            seq.serialize_element(&elem)?;
        }
        seq.end()
    }
}

impl<T> Serialize for Indexed<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for (id, elem) in self.0.iter().enumerate() {
            seq.serialize_element(&WithIndex { id, elem })?;
        }
        seq.end()
    }
}

pub trait Attachments {
    fn attachments(&self) -> Vec<Arc<File>>;
}

pub struct File {
    pub name: String,
    pub typ: String,
    pub data: Box<[u8]>,
}

#[async_trait]
pub trait Client: Sync {
    async fn request_weak<T: DeserializeOwned>(
        &self,
        method: Method,
        uri: &str,
        body: Option<&str>,
        files: &[Arc<File>],
    ) -> Result<T>;

    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        uri: &str,
        body: Option<&str>,
        files: &[Arc<File>],
    ) -> Result<T> {
        loop {
            match self.request_weak(method.clone(), uri, body, files).await {
                Err(RequestError::RateLimited) => (),
                Err(RequestError::Network) => (),
                r => break r,
            }
        }
    }
}

impl Bot {
    pub fn new<S: Into<String>>(token: S) -> Self {
        Self {
            token: token.into(),
            limits: Arc::new(Mutex::new(DiscordRateLimits {
                request_rate: 0.0,
                last_request: Instant::now(),
                retry_after: Instant::now(),

                buckets: HashMap::new(),
                bucket_cache: HashMap::new(),
            })),
        }
    }
    fn get_bucket(uri: &str) -> String {
        if uri.starts_with("/guilds/") || uri.starts_with("/channels/") {
            let s: String = uri.split_inclusive('/').take(3).collect();
            s.strip_suffix("/").unwrap_or(&s).into()
        } else {
            uri.into()
        }
    }
    fn bound_to_global_limit(uri: &str) -> bool {
        if uri.starts_with("/interactions/") || uri.starts_with("/webhooks/") {
            false
        } else {
            true
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}

pub async fn create_response(
    http: isahc::http::request::Builder,
    body: Option<&str>,
    files: &[Arc<File>],
) -> std::result::Result<isahc::Response<isahc::AsyncBody>, isahc::Error> {
    if files.len() > 0 {
        let mut bytes = Vec::new();

        if let Some(body) = body {
            bytes.extend_from_slice(
                "--boundary\nContent-Disposition: form-data; name=\"payload_json\"\nContent-Type: application/json\n\n"
                .as_bytes(),
            );
            bytes.extend_from_slice(body.as_bytes());
            bytes.extend_from_slice("\n".as_bytes());
        }

        for (i, file) in files.iter().enumerate() {
            bytes.extend_from_slice(format!(
                "--boundary\nContent-Disposition: form-data; name=\"files[{}]\"; filename=\"{}\"\nContent-Type: {}\n\n", 
                i,
                file.name,
                file.typ,
            ).as_bytes());
            bytes.extend_from_slice(&file.data);
            bytes.extend_from_slice("\n".as_bytes());
        }
        bytes.extend_from_slice("--boundary--\n".as_bytes());

        let request = http
            .header("Content-Type", "multipart/form-data; boundary=boundary")
            .body(bytes)
            .unwrap();
        isahc::send_async(request)
    } else if let Some(body) = body {
        let request = http
            .header("Content-Type", "application/json")
            .body(body)
            .unwrap();
        // println!("{}", request.body());
        isahc::send_async(request)
    } else {
        let request = http.body(()).unwrap();
        isahc::send_async(request)
    }.await
}

#[async_trait]
impl Client for Bot {
    async fn request_weak<T: DeserializeOwned>(
        &self,
        method: Method,
        uri: &str,
        body: Option<&str>,
        files: &[Arc<File>],
    ) -> Result<T> {
        let bucket = Bot::get_bucket(uri);

        // rate limits
        let now = {
            let mut me = self.limits.lock().await;
            let now = Instant::now();

            let mut time = me.retry_after.duration_since(now);

            // global rate limit
            let global = Bot::bound_to_global_limit(uri);
            if global && me.request_rate >= GLOBAL_RATE_LIMIT {
                time = time.max(Duration::from_secs_f32(1.0 / GLOBAL_RATE_LIMIT));
            }

            // local rate limit
            if let Some(bucket_id) = me.bucket_cache.get(&bucket) {
                let limit = &me.buckets[bucket_id];

                // check bucket remaining
                if limit.remaining == 0 {
                    time = time.max(limit.reset_at.duration_since(now));
                }
            }

            // sleep
            if !time.is_zero() {
                tokio::time::sleep(time).await;
            }

            if global {
                me.inc_request();
            }

            Instant::now()
        };

        // send request
        let http = isahc::Request::builder()
            .method(method)
            .uri(format!("https://discord.com/api/v10{}", uri))
            .header(
                "User-Agent",
                format!("DiscordBot ({}, {})", "https://astavie.github.io/", VERSION),
            )
            .header("Authorization", format!("Bot {}", self.token));

        let mut response = create_response(http, body, files).await.map_err(|err| {
            if err.is_client() || err.is_server() || err.is_tls() {
                RequestError::Authorization
            } else {
                RequestError::Network
            }
        })?;

        // update rate limit
        if let Some(remaining) = response.headers().get("X-RateLimit-Remaining") {
            let remaining = remaining.to_str().unwrap();
            let remaining: u64 = remaining.parse().unwrap();

            if let Some(reset_after) = response.headers().get("X-RateLimit-Reset-After") {
                let reset_after = reset_after.to_str().unwrap();
                let reset_after: f64 = reset_after.parse().unwrap();

                if let Some(bucket_id) = response.headers().get("X-RateLimit-Bucket") {
                    let bucket_id = bucket_id.to_str().unwrap();
                    let reset_at = now + Duration::from_secs_f64(reset_after);
                    let limit = RateLimit {
                        remaining,
                        reset_at,
                    };

                    let mut me = self.limits.lock().await;
                    me.bucket_cache.insert(bucket, bucket_id.into());
                    me.buckets.insert(bucket_id.into(), limit);
                }
            }
        }

        // check errors
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            // check for global limit
            if let Some(scope) = response.headers().get("X-RateLimit-Scope") {
                if scope == "global" {
                    let response: RateLimitResponse = response
                        .json()
                        .await
                        .expect("429 response contains expected json body");

                    let mut me = self.limits.lock().await;
                    me.retry_after = now + Duration::from_secs_f64(response.retry_after);
                }
            }

            return Err(RequestError::RateLimited);
        }

        let string = response.text().await.unwrap();
        // println!("{}", string);

        if response.status().is_client_error() {
            return Err(RequestError::ClientError(response.status()));
        }

        if response.status().is_server_error() {
            return Err(RequestError::ServerError);
        }

        if response.status() == StatusCode::NO_CONTENT {
            serde_json::from_str("null")
        } else {
            serde_json::from_str(&string)
        }
        .map_err(|e| {
            println!("{}", e);
            RequestError::ServerError
        })
    }
}
