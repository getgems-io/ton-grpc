pub mod balance;
pub mod route;
pub mod shard_prefix;

use crate::router::route::{BlockCriteria, Error, Route, ToRoute};
use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt::Debug;
use std::future::{ready, Ready};
use std::hash::Hash;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use tower::balance::p2c::Balance;
use tower::discover::{Change, Discover, ServiceList};
use tower::{BoxError, Service};

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum BlockAvailability {
    Available,
    NotAvailable,
    NotPresent,
    Unknown,
}

pub trait Routed {
    fn available(&self, chain: &i32, criteria: &BlockCriteria) -> BlockAvailability;
    fn last_seqno(&self) -> Option<i32>;
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
    D: Discover<Service = S> + Unpin,
    D::Key: Hash,
    D::Error: Debug,
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

        Self {
            discover,
            services: Default::default(),
        }
    }

    fn update_pending_from_discover(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<(), Infallible>>> {
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
                    tracing::warn!(?error, "discover error");
                }
            }
        }
    }
}

impl<S, D, Request> Service<&Request> for Router<S, D>
where
    Request: ToRoute,
    S: Service<Request, Error: Into<BoxError>> + Routed + Clone,
    D: Discover<Service = S, Error: Debug> + Unpin,
    D::Key: Hash,
{
    type Response = Balance<ServiceList<Vec<S>>, Request>;
    type Error = BoxError;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let _ = self.update_pending_from_discover(cx);

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
            Err(Error::RouteUnknown) => {
                metrics::counter!("ton_router_miss_count").increment(1);

                Route::Latest
                    .choose(self.services.values())
                    .map(|services| Balance::new(ServiceList::new(services)))
                    .map_err(Into::into)
            }
            Err(Error::RouteNotAvailable) => {
                metrics::counter!("ton_router_delayed_count").increment(1);

                Err(Error::RouteNotAvailable.into())
            }
        })
    }
}
