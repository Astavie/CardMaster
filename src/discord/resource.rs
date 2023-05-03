use std::{any::type_name, fmt, marker::PhantomData, num::ParseIntError};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::discord::request::{Client, Request, Result};

#[derive(Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct Snowflake<T> {
    phantom: PhantomData<T>,
    id: u64,
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
        value.to_string()
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
        f.write_fmt(format_args!("<{}> {}", type_name::<T>(), self))
    }
}

impl<T> fmt::Display for Snowflake<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(f)
    }
}

pub trait Resource<T>
where
    T: DeserializeOwned + Unpin,
{
    fn uri(&self) -> String;

    fn get_request(&self) -> Request<T> {
        Request::get(self.uri())
    }

    async fn get(&self, client: &impl Client) -> Result<T> {
        client.request(&self.get_request()).await
    }
}

pub trait Patchable<T, B>: Resource<T>
where
    T: DeserializeOwned + Unpin,
    B: Default + Serialize,
{
    fn patch_request(&self, f: impl FnOnce(&mut B) -> &mut B) -> Request<T> {
        let mut builder = B::default();
        f(&mut builder);
        Request::patch(self.uri(), &builder)
    }

    async fn patch(&self, client: &impl Client, f: impl FnOnce(&mut B) -> &mut B) -> Result<T> {
        client.request(&self.patch_request(f)).await
    }
}

pub trait Editable<T, B>: Patchable<T, B>
where
    T: DeserializeOwned + Unpin,
    B: Default + Serialize,
{
    async fn edit(&mut self, client: &impl Client, f: impl FnOnce(&mut B) -> &mut B) -> Result<()>;
}

impl<S, T, B> Editable<T, B> for S
where
    S: Patchable<T, B>,
    T: DeserializeOwned + Unpin + Into<Self>,
    B: Default + Serialize,
{
    async fn edit(&mut self, client: &impl Client, f: impl FnOnce(&mut B) -> &mut B) -> Result<()> {
        *self = self.patch(client, f).await?.into();
        Ok(())
    }
}

pub trait Deletable<T>: Resource<T> + Sized
where
    T: DeserializeOwned + Unpin,
{
    fn delete_request(self) -> Request<()> {
        Request::delete(self.uri())
    }

    async fn delete(self, client: &impl Client) -> Result<()> {
        client.request(&self.delete_request()).await
    }
}
