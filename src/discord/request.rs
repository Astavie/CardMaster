use std::{collections::HashMap, marker::PhantomData, sync::Arc, time::Duration};

use isahc::{
    http::{Method, StatusCode},
    AsyncReadResponseExt,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::{sync::Mutex, time::Instant};

pub struct Request<T> {
    phantom: PhantomData<T>,
    pub method: Method,
    pub uri: String,
    pub body: Option<String>,
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

impl<T> Request<T> {
    pub fn get(uri: String) -> Self {
        Request {
            phantom: PhantomData,
            method: Method::GET,
            uri,
            body: None,
        }
    }

    pub fn post(uri: String, body: &impl Serialize) -> Self {
        Request {
            phantom: PhantomData,
            method: Method::POST,
            uri,
            body: Some(serde_json::to_string(body).unwrap()),
        }
    }

    pub fn patch(uri: String, body: &impl Serialize) -> Self {
        Request {
            phantom: PhantomData,
            method: Method::PATCH,
            uri,
            body: Some(serde_json::to_string(body).unwrap()),
        }
    }

    pub fn delete(uri: String) -> Self {
        Request {
            phantom: PhantomData,
            method: Method::DELETE,
            uri,
            body: None,
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
pub struct Discord {
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

impl Discord {
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
            s.strip_suffix("/").unwrap_or(&s).to_owned()
        } else {
            uri.to_owned()
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

    pub async fn request_weak<T: DeserializeOwned + Unpin + Send + Sync>(
        &self,
        request: &Request<T>,
    ) -> Result<T> {
        let bucket = Discord::get_bucket(&request.uri);

        // rate limits
        let now = {
            let mut me = self.limits.lock().await;
            let now = Instant::now();

            let mut time = me.retry_after.duration_since(now);

            // global rate limit
            let global = Discord::bound_to_global_limit(&request.uri);
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
            .method(request.method.clone())
            .uri("https://discord.com/api/v10".to_owned() + &request.uri)
            .header(
                "User-Agent",
                format!("DiscordBot ({}, {})", "https://astavie.github.io/", VERSION),
            )
            .header("Authorization", "Bot ".to_owned() + &self.token);

        let mut response = if let Some(body) = request.body.as_ref() {
            let request = http
                .header("Content-Type", "application/json")
                .body(body.clone())
                .unwrap();
            // println!("{}", request.body());
            isahc::send_async(request)
        } else {
            let request = http.body(()).unwrap();
            isahc::send_async(request)
        }
        .await
        .map_err(|err| {
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
                    me.bucket_cache.insert(bucket, bucket_id.to_owned());
                    me.buckets.insert(bucket_id.to_owned(), limit);
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
            Ok(serde_json::from_str("null").unwrap())
        } else {
            serde_json::from_str(&string).map_err(|_| RequestError::ServerError)
        }
    }

    pub async fn request<T: DeserializeOwned + Unpin + Send + Sync>(
        &self,
        request: Request<T>,
    ) -> Result<T> {
        match self.request_weak(&request).await {
            Err(RequestError::RateLimited) => self.request_weak(&request).await,
            Err(RequestError::Network) => {
                // TODO: retry
                todo!("network error");
            }
            r => r,
        }
    }
}
