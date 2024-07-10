use crate::router::route::{Error, Route, ToRoute};
use crate::router::{route, Routed};
use crate::service::load_ref::LoadRef;
use futures::future::BoxFuture;
use futures::FutureExt;
use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt::Debug;
use std::future::ready;
use std::hash::Hash;
use std::mem;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use tower::balance::p2c;
use tower::discover::{Change, Discover, ServiceList};
use tower::load::Load;
use tower::Service;

pub struct Balance<S, D>
where
    D: Discover<Service = S>,
    D::Key: Hash,
{
    discover: D,
    services: HashMap<D::Key, S>,
    ready: HashMap<D::Key, S>,
}

impl<S, D> Balance<S, D>
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

        Self {
            discover,
            services: Default::default(),
            ready: Default::default(),
        }
    }
}

impl<S, D> Balance<S, D>
where
    D: Discover<Service = S, Key: Hash, Error: Debug> + Unpin,
{
    fn update_pending_from_discover(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<(), Infallible>>> {
        loop {
            match ready!(Pin::new(&mut self.discover).poll_discover(cx)).transpose() {
                Ok(None) => {
                    tracing::info!("No more services to discover");
                    return Poll::Ready(None);
                }
                Ok(Some(Change::Remove(key))) => {
                    tracing::info!("Removing service");
                    self.services.remove(&key);
                    self.ready.remove(&key);
                }
                Ok(Some(Change::Insert(key, svc))) => {
                    tracing::info!("Adding service");
                    self.services.insert(key, svc);
                }
                Err(e) => {
                    tracing::error!(?e);
                    return Poll::Pending;
                }
            }
        }
    }
}

impl<S, D, Request> Service<Request> for Balance<S, D>
where
    Request: ToRoute + 'static,
    S: Service<Request, Response: Send + 'static, Future: Send + 'static>
        + Routed
        + Load
        + 'static,
    S::Error: Into<tower::BoxError>,
    D: Discover<Service = S, Error: Into<tower::BoxError> + Debug> + Unpin,
    D::Key: Hash + Clone,
    <S as Load>::Metric: Debug,
{
    type Response = S::Response;
    type Error = tower::BoxError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let _ = self.update_pending_from_discover(cx);
        self.services.extend(mem::take(&mut self.ready));

        let mut ready_keys = Vec::new();
        for (k, s) in self.services.iter_mut() {
            if let Poll::Ready(Ok(())) = s.poll_ready(cx) {
                ready_keys.push(k.clone());
            }
        }

        if ready_keys.is_empty() {
            cx.waker().wake_by_ref();

            Poll::Pending
        } else {
            for key in ready_keys {
                if let Some(svc) = self.services.remove(&key) {
                    self.ready.insert(key.clone(), svc);
                }
            }

            Poll::Ready(Ok(()))
        }
    }

    fn call(&mut self, req: Request) -> Self::Future {
        if self.ready.is_empty() {
            panic!("called before ready")
        }

        let services = match req.to_route().choose(self.ready.values_mut()) {
            Ok(services) => services,
            Err(Error::RouteUnknown) => {
                metrics::counter!("ton_router_miss_count").increment(1);

                match Route::Latest.choose(self.ready.values_mut()) {
                    Ok(services) => services,
                    Err(e) => return ready(Err(e.into())).boxed(),
                }
            }
            Err(Error::RouteNotAvailable) => {
                metrics::counter!("ton_router_delayed_count").increment(1);

                return ready(Err(route::Error::RouteNotAvailable.into())).boxed();
            }
        };

        let mut balance =
            p2c::Balance::new(ServiceList::new(services.into_iter().map(LoadRef::new)));

        loop {
            let mut cx = Context::from_waker(futures::task::noop_waker_ref());
            match balance.poll_ready(&mut cx) {
                Poll::Ready(Ok(())) => break,
                Poll::Ready(Err(_)) | Poll::Pending => unreachable!(),
            }
        }

        balance.call(req).boxed()
    }
}
