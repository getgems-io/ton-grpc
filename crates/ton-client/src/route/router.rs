use crate::ToRoute;
use crate::route::Route;
use crate::route::{Error, Routed, choose};
use std::collections::HashMap;
use std::future::{Ready, ready};
use std::hash::Hash;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use ton_tower::IntoRequest;
use tower::balance::p2c::Balance as P2cBalance;
use tower::discover::{Change, Discover, ServiceList};
use tower::{BoxError, Service};

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
    D::Error: Into<BoxError>,
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
    ) -> Poll<Option<Result<(), D::Error>>> {
        loop {
            match ready!(Pin::new(&mut self.discover).poll_discover(cx)).transpose() {
                Ok(None) => return Poll::Ready(None),
                Ok(Some(Change::Remove(key))) => {
                    self.services.remove(&key);
                }
                Ok(Some(Change::Insert(key, svc))) => {
                    self.services.insert(key, svc);
                }
                Err(error) => return Poll::Ready(Some(Err(error))),
            }
        }
    }
}

impl<S, D, R> Service<&R> for Router<S, D>
where
    R: ToRoute + IntoRequest,
    S: Service<R::Request, Error: Into<BoxError>> + Routed + Clone,
    D: Discover<Service = S, Error: Into<BoxError>> + Unpin,
    D::Key: Hash,
{
    type Response = P2cBalance<ServiceList<Vec<S>>, R::Request>;
    type Error = BoxError;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if let Poll::Ready(Some(Err(error))) = self.update_pending_from_discover(cx) {
            return Poll::Ready(Err(error.into()));
        }

        for s in self.services.values_mut() {
            if let Poll::Ready(Ok(())) = s.poll_ready(cx) {
                return Poll::Ready(Ok(()));
            }
        }

        cx.waker().wake_by_ref();

        Poll::Pending
    }

    fn call(&mut self, req: &R) -> Self::Future {
        ready(match choose(&req.to_route(), self.services.values()) {
            Ok(services) => Ok(P2cBalance::new(ServiceList::new(services))),
            Err(Error::RouteUnknown) => {
                metrics::counter!("ton_router_miss_count").increment(1);

                choose(&Route::Latest, self.services.values())
                    .map(|services| P2cBalance::new(ServiceList::new(services)))
                    .map_err(Into::into)
            }
            Err(Error::RouteNotAvailable) => {
                metrics::counter!("ton_router_delayed_count").increment(1);

                Err(Error::RouteNotAvailable.into())
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::BlockCriteria;
    use futures::stream;
    use mockall::mock;
    use std::time::Duration;
    use tokio::time::timeout;
    use ton_tower::request::GetMasterchainInfo;
    use tower::ServiceExt;
    use tower::discover::Change;

    mock! {
        Service {}

        impl Clone for Service {
            fn clone(&self) -> Self;
        }

        impl Service<GetMasterchainInfo> for Service {
            type Response = ();
            type Error = BoxError;
            type Future = Ready<Result<(), BoxError>>;

            fn poll_ready<'a>(&mut self, _cx: &mut Context<'a>) -> Poll<Result<(), BoxError>>;
            fn call(&mut self, _req: GetMasterchainInfo) -> Ready<Result<(), BoxError>>;
        }

        impl Routed for Service {
            fn contains(&self, _chain: &i32, _criteria: &BlockCriteria) -> bool;
            fn contains_not_available(&self, _chain: &i32, _criteria: &BlockCriteria) -> bool;
            fn last_seqno(&self) -> Option<i32>;
        }
    }

    #[tokio::test]
    async fn returns_error_when_discover_errors_and_no_services() {
        let discover = stream::iter(vec![Err::<Change<i32, MockService>, BoxError>(
            "discover failed".into(),
        )]);
        let mut router: Router<MockService, _> = Router::new(discover);

        let error = ServiceExt::<&GetMasterchainInfo>::ready(&mut router)
            .await
            .err()
            .unwrap();

        assert!(error.to_string().contains("discover failed"));
    }

    #[tokio::test]
    async fn returns_pending_when_no_services_and_discover_still_active() {
        let discover = futures::stream::pending::<Result<Change<i32, MockService>, BoxError>>();
        let mut router: Router<MockService, _> = Router::new(discover);
        let deadline = Duration::from_millis(50);

        let future = ServiceExt::<&GetMasterchainInfo>::ready(&mut router);
        let error = timeout(deadline, future)
            .await
            .err()
            .expect("expected discover error");

        assert!(error.to_string().contains("deadline has elapsed"));
    }
}
