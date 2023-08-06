use std::{
    any::type_name,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    num::ParseIntError,
};

use serde::{Deserialize, Serialize};

macro_rules! resource {
    { $trait_name:ident as $endpoint:ty; use $client:ty; $(fn $func_name:ident ($ref:tt $self:ident $(, $name:ident : $ty:ty)*) -> $ret:ty $impl:block)+ } => {
        #[::async_trait::async_trait]
        pub trait $trait_name: Sized {
            fn endpoint(&self) -> &$endpoint;
            $(
                async fn $func_name(#[allow(unused_mut)] $ref $self, client: &$client $(, $name : $ty)*) -> $crate::request::Result<$ret> {
                    let req = $impl;
                    $crate::request::Request::request(req, client).await
                }
            )+
        }
        impl $trait_name for $endpoint {
            fn endpoint(&self) -> &$endpoint {
                self
            }
        }
    };
}
pub(crate) use resource;

pub trait Endpoint {
    fn uri(&self) -> String;
}

#[derive(Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct Snowflake<T> {
    phantom: PhantomData<fn() -> T>,
    id: u64,
}

impl<T> Snowflake<T> {
    pub fn as_int(&self) -> u64 {
        self.id
    }
}

impl<T> PartialEq for Snowflake<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl<T> Eq for Snowflake<T> {}

impl<T> Hash for Snowflake<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl<T> Clone for Snowflake<T> {
    fn clone(&self) -> Self {
        Snowflake::new(self.id)
    }
}

impl<T> Copy for Snowflake<T> {}

impl<T> Snowflake<T> {
    pub fn new(id: u64) -> Self {
        Snowflake {
            phantom: PhantomData,
            id,
        }
    }
}

impl<T> From<Snowflake<T>> for String {
    fn from(value: Snowflake<T>) -> Self {
        value.id.to_string()
    }
}

impl<T> TryFrom<String> for Snowflake<T> {
    type Error = ParseIntError;

    fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
        Ok(Snowflake::new(value.parse()?))
    }
}

impl<T> fmt::Debug for Snowflake<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("<{}> {}", type_name::<T>(), self.id))
    }
}
