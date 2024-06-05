use std::collections::HashMap;
use std::future::{Ready, ready};
use std::hash::Hash;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use tower::discover::{Change, Discover};
use tower::Service;
use ton_client_utils::router::{Route, Routed, RouterError};
use crate::error::Error;
use crate::request::Requestable;

pub(crate) trait Routable {
    fn route(&self) -> Route { Route::Latest }
}

pub(crate) struct Router<S, D, Request>
    where
        S: Service<Request>,
        D: Discover<Service=S, Error = anyhow::Error> + Unpin,
        D::Key: Eq + Hash,
{
    discover: D,
    services: HashMap<D::Key, S>,
    _phantom_data: PhantomData<Request>
}

impl<S, D, Request> Router<S, D, Request>
    where
        S: Service<Request> + Routed + Clone,
        D: Discover<Service=S, Error = anyhow::Error> + Unpin,
        D::Key: Eq + Hash,
{
    pub(crate) fn new(discover: D) -> Self {
        metrics::describe_counter!("ton_router_miss_count", "Count of misses in router");
        metrics::describe_counter!("ton_router_fallback_hit_count", "Count of fallback request hits in router");
        metrics::describe_counter!("ton_router_delayed_count", "Count of delayed requests in router");
        metrics::describe_counter!("ton_router_delayed_hit_count", "Count of delayed request hits in router");
        metrics::describe_counter!("ton_router_delayed_miss_count", "Count of delayed request misses in router");

        Router { discover, services: Default::default(), _phantom_data: Default::default() }
    }

    fn update_pending_from_discover(&mut self, cx: &mut Context<'_>, ) -> Poll<Option<Result<(), anyhow::Error>>> {
        loop {
            match ready!(Pin::new(&mut self.discover).poll_discover(cx)).transpose()? {
                None => return Poll::Ready(None),
                Some(Change::Remove(key)) => {
                    self.services.remove(&key);
                }
                Some(Change::Insert(key, svc)) => {
                    self.services.insert(key, svc);
                }
            }
        }
    }
}

impl<S, D, Request> Service<&Route> for Router<S, D, Request>
    where
        S: Service<Request> + Routed + Clone,
        D: Discover<Service=S, Error = anyhow::Error> + Unpin,
        D::Key: Eq + Hash,
        Request: Requestable + 'static
{
    type Response = Vec<S>;
    type Error = Error;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let _ = self.update_pending_from_discover(cx)
            .map_err(Error::Custom)?;

        for s in self.services.values_mut() {
            match S::poll_ready(s, cx) {
                Poll::Ready(Ok(())) => return Poll::Ready(Ok(())),
                _ => {}
            }
        }

        cx.waker().wake_by_ref();

        Poll::Pending
    }

    fn call(&mut self, req: &Route) -> Self::Future {
        match req.choose(self.services.values()) {
            Ok(services) => ready(Ok(services.into_iter().cloned().collect())),
            Err(RouterError::RouteUnknown) => {
                metrics::counter!("ton_router_miss_count").increment(1);

                ready(
                    Route::Latest.choose(self.services.values())
                        .map(|services| services.into_iter().cloned().collect())
                        .map_err(Error::Router)
                )
            },
            Err(RouterError::RouteNotAvailable) => {
                metrics::counter!("ton_router_delayed_count").increment(1);

                ready(Err(Error::Router(RouterError::RouteNotAvailable)))
            },
        }
    }
}
