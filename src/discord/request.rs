use std::marker::PhantomData;

use isahc::http::{Method, StatusCode};
use serde::{de::DeserializeOwned, Serialize};

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

pub trait Client: Clone + Send {
    async fn token(&self) -> &str;
    async fn request_weak<T: DeserializeOwned + Unpin>(&self, request: &Request<T>) -> Result<T>;

    async fn request<T: DeserializeOwned + Unpin>(&self, request: &Request<T>) -> Result<T> {
        match self.request_weak(request).await {
            Err(RequestError::RateLimited) => self.request_weak(request).await,
            Err(RequestError::Network) => {
                // TODO: retry
                todo!("network error");
            }
            r => r,
        }
    }
}
