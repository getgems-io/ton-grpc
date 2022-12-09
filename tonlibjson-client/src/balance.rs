use tower::Service;
use tower::discover::{Change, Discover};
use tower::load::{CompleteOnResponse, Load};
use tower::ready_cache::{error::Failed, ReadyCache};
use rand::{rngs::SmallRng, Rng, SeedableRng};
use std::hash::Hash;
use std::marker::PhantomData;
use std::{fmt, mem, pin::Pin, task::{Context, Poll}};
use std::cmp::min;
use std::sync::{Arc, Mutex};
use futures::{future, ready};
use tracing::{debug, trace, warn};
use tower::BoxError;
use futures::TryFutureExt;
use crate::block::BlockIdExt;
use crate::cursor_client::CursorClient;
use crate::session::SessionRequest;
use rand::seq::IteratorRandom;
use crate::discover::CursorClientDiscover;

pub struct BalanceRequest {
    pub request: SessionRequest,
    pub lt: Option<i64>
}

impl BalanceRequest {
    pub fn new(lt: Option<i64>, request: SessionRequest) -> Self {
        Self {
            request,
            lt
        }
    }
}

impl From<SessionRequest> for BalanceRequest {
    fn from(request: SessionRequest) -> Self {
        Self {
            request,
            lt: None
        }
    }
}

pub struct Balance
{
    discover: CursorClientDiscover,

    services: ReadyCache<<CursorClientDiscover as Discover>::Key, <CursorClientDiscover as Discover>::Service, SessionRequest>,

    rng: SmallRng,

    _req: PhantomData<SessionRequest>,
}

impl Balance {
    /// Constructs a load balancer that uses operating system entropy.
    pub fn new(discover: CursorClientDiscover) -> Self {
        Self::from_rng(discover, &mut rand::thread_rng()).expect("ThreadRNG must be valid")
    }

    /// Constructs a load balancer seeded with the provided random number generator.
    pub fn from_rng<R: Rng>(discover: CursorClientDiscover, rng: R) -> Result<Self, rand::Error> {
        let rng = SmallRng::from_rng(rng)?;
        Ok(Self {
            rng,
            discover,
            services: ReadyCache::default(),

            _req: PhantomData,
        })
    }
}

impl Balance {
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
    fn p2c_ready_index(&mut self, min_lt: Option<i64>) -> Option<usize> {
        match self.services.ready_len() {
            0 => None,
            1 => Some(0),
            len => {
                let mut idxs = (0 .. len)
                    .map(|index| (index, self.services.get_ready_index(index).expect("invalid index")))
                    .filter(|(index, (_, svc))| {
                        if let Some(min_lt) = min_lt {
                            if svc.load().first_block.as_ref().unwrap().start_lt <= min_lt {
                                debug!(min_tl = min_lt, "start_tl: {}", svc.load().first_block.as_ref().unwrap().start_lt);

                                true
                            } else {
                                false
                            }
                        } else {
                            true
                        }
                    })
                    .choose_multiple(&mut self.rng, 2);

                let aidx = idxs.pop().unwrap().0;
                let bidx = idxs.pop().unwrap().0;

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
    fn ready_index_load(&self, index: usize) -> <<CursorClientDiscover as Discover>::Service as Load>::Metric {
        let (_, svc) = self.services.get_ready_index(index).expect("invalid index");
        svc.load()
    }
}

impl Service<BalanceRequest> for Balance {
    type Response = <<CursorClientDiscover as Discover>::Service as Service<SessionRequest>>::Response;
    type Error = BoxError;
    type Future = future::MapErr<
        <<CursorClientDiscover as Discover>::Service as Service<SessionRequest>>::Future,
        fn(<<CursorClientDiscover as Discover>::Service as Service<SessionRequest>>::Error) -> BoxError,
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
        let index = if let Some(lt) = request.lt {
            debug!(lt = lt, "request with lt");
            // todo fix
            self.p2c_ready_index(Some(lt)).expect("called before ready")
        } else {
            self.p2c_ready_index(None).expect("called before ready")
        };

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
