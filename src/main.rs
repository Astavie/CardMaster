#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use command::CommandData;
use dotenv_codegen::dotenv;
use futures_util::StreamExt;
use gateway::Gateway;
use isahc::{http::StatusCode, AsyncReadResponseExt};
use request::{Client, Request, RequestError, Result};
use serde::{de::DeserializeOwned, Deserialize};
use tokio::sync::Mutex;

use crate::application::Application;
use crate::command::Commands;
use crate::resource::Deletable;

mod gateway;
mod request;
mod resource;

mod application;
mod channel;
mod command;
mod guild;
mod interaction;
mod message;
mod user;

struct Discord {
    token: String,

    request_rate: f32,
    last_request: Instant,
    retry_after: Instant,

    buckets: HashMap<String, RateLimit>,
    bucket_cache: HashMap<String, String>,
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

impl Discord {
    fn new<S: Into<String>>(token: S) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            token: token.into(),

            request_rate: 0.0,
            last_request: Instant::now(),
            retry_after: Instant::now(),

            buckets: HashMap::new(),
            bucket_cache: HashMap::new(),
        }))
    }
    fn inc_request(&mut self) {
        let now = Instant::now();
        let diff = now.duration_since(self.last_request).as_secs_f32();
        self.request_rate = (self.request_rate + 1.0) / (diff + 1.0);
        self.last_request = now;
    }
    fn get_bucket(uri: &str) -> String {
        if uri.starts_with("/guilds/") || uri.starts_with("/channels/") {
            let s: String = uri.split_inclusive('/').take(3).collect();
            s.strip_suffix("/").unwrap_or(&s).to_owned()
        } else {
            uri.to_owned()
        }
    }
}

const NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

const RUSTMASTER: &str = dotenv!("RUSTMASTER");
const CARDMASTER: &str = dotenv!("CARDMASTER");

impl Client for Arc<Mutex<Discord>> {
    async fn request<T: DeserializeOwned + Unpin>(&self, request: Request<T>) -> Result<T> {
        let bucket = Discord::get_bucket(&request.uri);

        // rate limits
        let (now, token) = {
            let mut me = self.lock().await;
            let now = Instant::now();

            // global rate limit
            let mut time = me.retry_after.duration_since(now);

            if me.request_rate >= GLOBAL_RATE_LIMIT {
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

            if !time.is_zero() {
                tokio::time::sleep(time).await;
            }

            me.inc_request();
            (me.last_request, me.token.clone())
        };

        // send request
        let http = isahc::Request::builder()
            .method(request.method.clone())
            .uri("https://discord.com/api/v10".to_owned() + &request.uri)
            .header(
                "User-Agent",
                format!("DiscordBot ({}, {})", "https://astavie.github.io/", VERSION),
            )
            .header("Authorization", "Bot ".to_owned() + &token);

        let mut response = if let Some(body) = request.body {
            let request = http
                .header("Content-Type", "application/json")
                .body(body)
                .unwrap();
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

                    let mut me = self.lock().await;
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

                    let mut me = self.lock().await;
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

    async fn token(&self) -> String {
        self.lock().await.token.clone()
    }
}

async fn purge(commands: Commands, client: &impl Client) -> Result<()> {
    for command in commands.all(client).await? {
        command.delete(client).await?;
    }
    Ok(())
}

async fn run() -> Result<()> {
    let client = Discord::new(RUSTMASTER);

    let application = Application::get(&client).await?;
    purge(application.global_commands(), &client).await?;
    application
        .global_commands()
        .create(
            &client,
            &CommandData::builder()
                .name("test".to_owned())
                .description("test command".to_owned())
                .build()
                .unwrap(),
        )
        .await?;

    let mut gateway = Gateway::connect(&client).await?;
    println!("{:?}", gateway.next().await);
    gateway.close().await;

    Ok(())
}

#[tokio::main]
async fn main() {
    run().await.unwrap()
}
