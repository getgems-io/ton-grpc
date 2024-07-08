pub mod route;
pub mod shards;

use crate::router::route::{BlockCriteria, Route, ToRoute};
use std::collections::HashMap;
use std::fmt::Debug;
use std::future::{ready, Ready};
use std::hash::Hash;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use tower::balance::p2c::Balance;
use tower::discover::{Change, Discover, ServiceList};
use tower::Service;

pub trait Routed {
    fn contains(&self, chain: &i32, criteria: &BlockCriteria) -> bool;
    fn contains_not_available(&self, chain: &i32, criteria: &BlockCriteria) -> bool;
    fn last_seqno(&self) -> Option<i32>;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Route(#[from] route::Error),
    #[error(transparent)]
    Discover(#[from] tower::BoxError),
}

pub struct Router<S, D>
where
    D: Discover<Service = S>,
    D::Key: Hash,
{
    discover: D,
    services: HashMap<D::Key, S>,
}

impl<S, D> Router<S, D>
where
    D: Discover<Service = S>,
    D::Key: Hash,
{
    pub fn new(discover: D) -> Self {
        metrics::describe_counter!("ton_router_miss_count", "Count of misses in router");
        metrics::describe_counter!(
            "ton_router_fallback_hit_count",
            "Count of fallback request hits in router"
        );
        metrics::describe_counter!(
            "ton_router_delayed_count",
            "Count of delayed requests in router"
        );
        metrics::describe_counter!(
            "ton_router_delayed_hit_count",
            "Count of delayed request hits in router"
        );
        metrics::describe_counter!(
            "ton_router_delayed_miss_count",
            "Count of delayed request misses in router"
        );

        Router {
            discover,
            services: Default::default(),
        }
    }
}

impl<S, D, E> Router<S, D>
where
    D: Discover<Service = S, Error = E> + Unpin,
    D::Key: Hash,
    E: Debug,
{
    fn update_pending_from_discover(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<(), E>>> {
        loop {
            match ready!(Pin::new(&mut self.discover).poll_discover(cx)).transpose() {
                Ok(None) => return Poll::Ready(None),
                Ok(Some(Change::Remove(key))) => {
                    self.services.remove(&key);
                }
                Ok(Some(Change::Insert(key, svc))) => {
                    self.services.insert(key, svc);
                }
                Err(error) => {
                    tracing::warn!(?error);

                    return Poll::Pending
                }
            }
        }
    }
}

impl<S, D, Request> Service<&Request> for Router<S, D>
where
    Request: ToRoute,
    S: Service<Request> + Routed + Clone,
    S::Error: Into<tower::BoxError>,
    D: Discover<Service = S, Error: Into<tower::BoxError> + Debug> + Unpin,
    D::Key: Hash,
{
    type Response = Balance<ServiceList<Vec<S>>, Request>;
    type Error = Error;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let _ = self.update_pending_from_discover(cx).map_err(Into::into)?;

        for s in self.services.values_mut() {
            if let Poll::Ready(Ok(())) = s.poll_ready(cx) {
                return Poll::Ready(Ok(()));
            }
        }

        cx.waker().wake_by_ref();

        Poll::Pending
    }

    fn call(&mut self, req: &Request) -> Self::Future {
        ready(match req.to_route().choose(self.services.values()) {
            Ok(services) => Ok(Balance::new(ServiceList::new(services))),
            Err(route::Error::RouteUnknown) => {
                metrics::counter!("ton_router_miss_count").increment(1);

                Route::Latest
                    .choose(self.services.values())
                    .map(|services| Balance::new(ServiceList::new(services)))
                    .map_err(Into::into)
            }
            Err(route::Error::RouteNotAvailable) => {
                metrics::counter!("ton_router_delayed_count").increment(1);

                Err(route::Error::RouteNotAvailable.into())
            }
        })
    }
}
