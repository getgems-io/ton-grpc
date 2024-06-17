use std::collections::HashMap;
use std::future::{Ready, ready};
use std::hash::Hash;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use tower::balance::p2c::Balance;
use tower::discover::{Change, Discover, ServiceList};
use tower::Service;
use ton_client_util::router::{Route, Routed, RouterError};
use crate::error::{Error, ErrorService};

pub(crate) trait Routable {
    fn route(&self) -> Route { Route::Latest }
}

pub(crate) struct Router<S, D>
    where
        D: Discover<Service=S>,
        D::Key: Hash,
{
    discover: D,
    services: HashMap<D::Key, S>
}

impl<S, D, E> Router<S, D>
    where
        D: Discover<Service=S, Error = E> + Unpin,
        D::Key: Hash,
{
    pub(crate) fn new(discover: D) -> Self {
        metrics::describe_counter!("ton_router_miss_count", "Count of misses in router");
        metrics::describe_counter!("ton_router_fallback_hit_count", "Count of fallback request hits in router");
        metrics::describe_counter!("ton_router_delayed_count", "Count of delayed requests in router");
        metrics::describe_counter!("ton_router_delayed_hit_count", "Count of delayed request hits in router");
        metrics::describe_counter!("ton_router_delayed_miss_count", "Count of delayed request misses in router");

        Router { discover, services: Default::default() }
    }

    fn update_pending_from_discover(&mut self, cx: &mut Context<'_>, ) -> Poll<Option<Result<(), E>>> {
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

impl<S, D, Request> Service<&Request> for Router<S, D>
    where
        Request: Routable,
        S: Service<Request> + Routed + Clone,
        S::Error: Into<tower::BoxError>,
        D: Discover<Service=S, Error = anyhow::Error> + Unpin,
        D::Key: Hash
{
    type Response = ErrorService<Balance<ServiceList<Vec<S>>, Request>>;
    type Error = Error;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let _ = self.update_pending_from_discover(cx)
            .map_err(Error::Custom)?;

        for s in self.services.values_mut() {
            if let Poll::Ready(Ok(())) = s.poll_ready(cx) {
                return Poll::Ready(Ok(()))
            }
        }

        cx.waker().wake_by_ref();

        Poll::Pending
    }

    fn call(&mut self, req: &Request) -> Self::Future {
        ready(match req.route().choose(self.services.values()) {
            Ok(services) => Ok(
                ErrorService::new(Balance::new(ServiceList::new(
                    services.into_iter().cloned().collect()
                )))
            ),
            Err(RouterError::RouteUnknown) => {
                metrics::counter!("ton_router_miss_count").increment(1);

                Route::Latest.choose(self.services.values())
                    .map(|services|
                        ErrorService::new(Balance::new(ServiceList::new(
                            services.into_iter().cloned().collect()
                        )))
                    )
                    .map_err(Error::Router)
            },
            Err(RouterError::RouteNotAvailable) => {
                metrics::counter!("ton_router_delayed_count").increment(1);

                Err(Error::Router(RouterError::RouteNotAvailable))
            },
        })
    }
}
