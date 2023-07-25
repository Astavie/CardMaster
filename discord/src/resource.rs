use std::{any::type_name, convert::Infallible, fmt, marker::PhantomData, num::ParseIntError};

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::request::Client;

use super::request::{Discord, Request, Result};

#[derive(Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct Snowflake<T> {
    phantom: PhantomData<fn() -> T>,
    id: u64,
}

impl<T> PartialEq for Snowflake<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl<T> Eq for Snowflake<T> {}

impl<T> std::hash::Hash for Snowflake<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
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

pub trait Endpoint {
    type Result: DeserializeOwned;
    type Get: DeserializeOwned = Self::Result;

    type Delete = Infallible;
    type Patch = Infallible;
    type Create = Infallible;

    type Client: Client + ?Sized = Discord;
    fn uri(&self) -> String;
}

impl<T> Resource for T
where
    T: Endpoint,
{
    type Endpoint = Self;

    fn endpoint(&self) -> &Self::Endpoint {
        self
    }
}

#[async_trait]
pub trait Resource {
    type Endpoint: Endpoint;

    fn endpoint(&self) -> &Self::Endpoint;

    fn get_request(
        &self,
    ) -> Request<<Self::Endpoint as Endpoint>::Get, <Self::Endpoint as Endpoint>::Client> {
        Request::get(self.endpoint().uri())
    }

    async fn get(
        &self,
        client: &<Self::Endpoint as Endpoint>::Client,
    ) -> Result<<Self::Endpoint as Endpoint>::Get> {
        self.get_request().request(client).await
    }
}

#[async_trait]
pub trait Patchable
where
    Self: Resource,
    <Self::Endpoint as Endpoint>::Patch: Default + Serialize,
{
    fn patch_request(
        &self,
        f: impl FnOnce(<Self::Endpoint as Endpoint>::Patch) -> <Self::Endpoint as Endpoint>::Patch,
    ) -> Request<<Self::Endpoint as Endpoint>::Result, <Self::Endpoint as Endpoint>::Client> {
        let builder = f(<Self::Endpoint as Endpoint>::Patch::default());
        Request::patch(self.endpoint().uri(), &builder)
    }

    async fn patch(
        &self,
        client: &<Self::Endpoint as Endpoint>::Client,
        f: impl FnOnce(<Self::Endpoint as Endpoint>::Patch) -> <Self::Endpoint as Endpoint>::Patch
            + Send,
    ) -> Result<<Self::Endpoint as Endpoint>::Result> {
        self.patch_request(f).request(client).await
    }
}

impl<T> Patchable for T
where
    T: Resource,
    <T::Endpoint as Endpoint>::Patch: Default + Serialize,
{
}

#[async_trait]
pub trait Editable
where
    Self: Patchable + Sized + Sync,
    <Self::Endpoint as Endpoint>::Patch: Default + Serialize,
    <Self::Endpoint as Endpoint>::Result: Into<Self>,
{
    async fn edit(
        &mut self,
        client: &<Self::Endpoint as Endpoint>::Client,
        f: impl FnOnce(<Self::Endpoint as Endpoint>::Patch) -> <Self::Endpoint as Endpoint>::Patch
            + Send,
    ) -> Result<()> {
        *self = self.patch(client, f).await?.into();
        Ok(())
    }
}

impl<T> Editable for T
where
    T: Patchable + Sync,
    <T::Endpoint as Endpoint>::Patch: Default + Serialize,
    <T::Endpoint as Endpoint>::Result: Into<T>,
{
}

#[async_trait]
pub trait Deletable
where
    Self: Resource + Sized,
    Self::Endpoint: Endpoint<Delete = ()>,
{
    fn delete_request(self) -> Request<(), <Self::Endpoint as Endpoint>::Client> {
        Request::delete(self.endpoint().uri())
    }

    async fn delete(self, client: &<Self::Endpoint as Endpoint>::Client) -> Result<()> {
        self.delete_request().request(client).await
    }
}

impl<T> Deletable for T
where
    T: Resource,
    T::Endpoint: Endpoint<Delete = ()>,
{
}

#[async_trait]
pub trait Creatable
where
    Self: Resource,
    <Self::Endpoint as Endpoint>::Create: Serialize + Sync,
{
    fn create_request(
        &self,
        create: &<Self::Endpoint as Endpoint>::Create,
    ) -> Request<<Self::Endpoint as Endpoint>::Result, <Self::Endpoint as Endpoint>::Client> {
        Request::post(self.endpoint().uri(), create)
    }

    async fn create(
        &self,
        client: &<Self::Endpoint as Endpoint>::Client,
        create: &<Self::Endpoint as Endpoint>::Create,
    ) -> Result<<Self::Endpoint as Endpoint>::Result> {
        self.create_request(create).request(client).await
    }
}

impl<T> Creatable for T
where
    T: Resource,
    <T::Endpoint as Endpoint>::Create: Serialize + Sync,
{
}
