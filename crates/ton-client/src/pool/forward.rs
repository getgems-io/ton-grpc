use crate::ToRoute;
use crate::route::Route;
use std::time::Duration;
use ton_tower::{
    IntoRequest, Request,
    service::{retry::Retryable, timeout::ToTimeout},
};

#[derive(Clone, Debug)]
pub struct Forward<T> {
    route: Route,
    inner: T,
}

impl<T> Forward<T> {
    pub fn new(route: Route, inner: T) -> Self {
        Self { route, inner }
    }

    pub fn route(&self) -> Route {
        self.route
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<R> IntoRequest for Forward<R>
where
    R: Request,
{
    type Request = R;

    fn into_request(self) -> R {
        self.inner
    }
}

impl<R> ToRoute for Forward<R>
where
    R: Request,
{
    fn to_route(&self) -> Route {
        self.route
    }
}

impl<T> ToTimeout for Forward<T>
where
    T: ToTimeout,
{
    fn to_timeout(&self) -> Option<Duration> {
        self.inner.to_timeout()
    }
}

impl<T> Retryable for Forward<T>
where
    T: Retryable,
{
    const IS_RETRYABLE: bool = T::IS_RETRYABLE;
}
