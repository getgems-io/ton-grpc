use tower::Service;
use tower::discover::{Change, Discover};
use tower::load::Load;
use tower::ready_cache::{error::Failed, ReadyCache};
use rand::{rngs::SmallRng, Rng, SeedableRng};
use std::hash::Hash;
use std::marker::PhantomData;
use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};
use futures::{future, ready};
use tracing::{debug, trace};
use tower::BoxError;
use futures::TryFutureExt;
use crate::block::BlockIdExt;
use crate::session::SessionRequest;

pub struct BalanceRequest {
    pub request: SessionRequest,
    pub block: Option<BlockIdExt>
}

impl BalanceRequest {
    pub fn new(block: Option<BlockIdExt>, request: SessionRequest) -> Self {
        Self {
            request,
            block
        }
    }
}

impl From<SessionRequest> for BalanceRequest {
    fn from(request: SessionRequest) -> Self {
        Self {
            request,
            block: None
        }
    }
}

pub struct Balance<D>
    where
        D: Discover,
        D::Key: Hash,
{
    discover: D,

    services: ReadyCache<D::Key, D::Service, SessionRequest>,

    rng: SmallRng,

    _req: PhantomData<SessionRequest>,
}

impl<D> Balance<D>
    where
        D: Discover,
        D::Key: Hash,
        D::Service: Service<SessionRequest>,
        <D::Service as Service<SessionRequest>>::Error: Into<BoxError>,
{
    /// Constructs a load balancer that uses operating system entropy.
    pub fn new(discover: D) -> Self {
        Self::from_rng(discover, &mut rand::thread_rng()).expect("ThreadRNG must be valid")
    }

    /// Constructs a load balancer seeded with the provided random number generator.
    pub fn from_rng<R: Rng>(discover: D, rng: R) -> Result<Self, rand::Error> {
        let rng = SmallRng::from_rng(rng)?;
        Ok(Self {
            rng,
            discover,
            services: ReadyCache::default(),

            _req: PhantomData,
        })
    }
}

impl<D> Balance<D>
    where
        D: Discover + Unpin,
        D::Key: Hash + Clone,
        D::Error: Into<BoxError>,
        D::Service: Service<SessionRequest> + Load,
        <D::Service as Load>::Metric: std::fmt::Debug,
        <D::Service as Service<SessionRequest>>::Error: Into<BoxError>,
{
    /// Polls `discover` for updates, adding new items to `not_ready`.
    ///
    /// Removals may alter the order of either `ready` or `not_ready`.
    fn update_pending_from_discover(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<(), DiscoverError>>> {
        debug!("updating from discover");
        loop {
            match ready!(Pin::new(&mut self.discover).poll_discover(cx))
                .transpose()
                .map_err(|e| DiscoverError(e.into()))?
            {
                None => return Poll::Ready(None),
                Some(Change::Remove(key)) => {
                    trace!("remove");
                    self.services.evict(&key);
                }
                Some(Change::Insert(key, svc)) => {
                    trace!("insert");
                    // If this service already existed in the set, it will be
                    // replaced as the new one becomes ready.
                    self.services.push(key, svc);
                }
            }
        }
    }

    fn promote_pending_to_ready(&mut self, cx: &mut Context<'_>) {
        loop {
            match self.services.poll_pending(cx) {
                Poll::Ready(Ok(())) => {
                    // There are no remaining pending services.
                    debug_assert_eq!(self.services.pending_len(), 0);
                    break;
                }
                Poll::Pending => {
                    // None of the pending services are ready.
                    debug_assert!(self.services.pending_len() > 0);
                    break;
                }
                Poll::Ready(Err(error)) => {
                    // An individual service was lost; continue processing
                    // pending services.
                    debug!(%error, "dropping failed endpoint");
                }
            }
        }
        trace!(
            ready = %self.services.ready_len(),
            pending = %self.services.pending_len(),
            "poll_unready"
        );
    }

    /// Performs P2C on inner services to find a suitable endpoint.
    fn p2c_ready_index(&mut self, min_seqno: Option<i32>) -> Option<usize> {


        match self.services.ready_len() {
            0 => None,
            1 => Some(0),
            len => {
                // Get two distinct random indexes (in a random order) and
                // compare the loads of the service at each index.
                let idxs = rand::seq::index::sample(&mut self.rng, len, 2);

                let aidx = idxs.index(0);
                let bidx = idxs.index(1);
                debug_assert_ne!(aidx, bidx, "random indices must be distinct");

                let aload = self.ready_index_load(aidx);
                let bload = self.ready_index_load(bidx);
                let chosen = if aload <= bload { aidx } else { bidx };

                trace!(
                    a.index = aidx,
                    a.load = ?aload,
                    b.index = bidx,
                    b.load = ?bload,
                    chosen = if chosen == aidx { "a" } else { "b" },
                    "p2c",
                );
                Some(chosen)
            }
        }
    }

    /// Accesses a ready endpoint by index and returns its current load.
    fn ready_index_load(&self, index: usize) -> <D::Service as Load>::Metric {
        let (_, svc) = self.services.get_ready_index(index).expect("invalid index");
        svc.load()
    }

    pub(crate) fn discover_mut(&mut self) -> &mut D {
        &mut self.discover
    }
}

impl<D> Service<BalanceRequest> for Balance<D>
    where
        D: Discover + Unpin,
        D::Key: Hash + Clone,
        D::Error: Into<BoxError>,
        D::Service: Service<SessionRequest> + Load,
        <D::Service as Load>::Metric: std::fmt::Debug,
        <D::Service as Service<SessionRequest>>::Error: Into<BoxError>,
{
    type Response = <D::Service as Service<SessionRequest>>::Response;
    type Error = BoxError;
    type Future = future::MapErr<
        <D::Service as Service<SessionRequest>>::Future,
        fn(<D::Service as Service<SessionRequest>>::Error) -> BoxError,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let _ = self.update_pending_from_discover(cx)?;
        self.promote_pending_to_ready(cx);

        let ready_index = self.p2c_ready_index(None);
        if ready_index.is_none() {

            return Poll::Pending;
        }

        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: BalanceRequest) -> Self::Future {
        let index = self.p2c_ready_index(None).expect("called before ready");
        self.services
            .call_ready_index(index, request.request)
            .map_err(Into::into)
    }
}


#[derive(Debug)]
pub struct DiscoverError(pub(crate) BoxError);

impl fmt::Display for DiscoverError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "load balancer discovery error: {}", self.0)
    }
}

impl std::error::Error for DiscoverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.0)
    }
}
