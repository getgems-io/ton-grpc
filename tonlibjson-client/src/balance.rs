use tower::Service;
use tower::discover::{Change, Discover};
use tower::load::Load;
use tower::ready_cache::ReadyCache;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use std::marker::PhantomData;
use std::{fmt, pin::Pin, task::{Context, Poll}};
use derive_new::new;
use futures::{future, ready};
use tracing::{debug, info, trace};
use tower::BoxError;
use futures::TryFutureExt;
use crate::session::SessionRequest;
use crate::discover::CursorClientDiscover;
use itertools::Itertools;
use rand::seq::index::sample;
use crate::cursor_client::Metrics;
use crate::request::{Requestable, RequestableWrapper, Routable};

#[derive(Debug, Clone, Copy)]
pub enum Route {
    Any,
    Block { chain: i64, criteria: BlockCriteria },
    Latest { chain: i64 }
}

#[derive(Debug, Clone, Copy)]
pub enum BlockCriteria {
    Seqno(i32),
    LogicalTime(i64)
}

impl Route {
    pub fn choose(
        &self,
        cache: &ReadyCache<<CursorClientDiscover as Discover>::Key, <CursorClientDiscover as Discover>::Service, SessionRequest>,
        rng: &mut SmallRng
    ) -> Option<usize> {
        return match self {
            Route::Any => {
                match cache.ready_len() {
                    0 => None,
                    1 => Some(0),
                    len => {
                        let idxs = sample(rng, len, 2);
                        let aidx = idxs.index(0);
                        let bidx = idxs.index(1);

                        let aload = cache.get_ready_index(aidx).expect("invalid index").1.load().expect("service must be ready");
                        let bload = cache.get_ready_index(bidx).expect("invalid index").1.load().expect("service must be ready");

                        let chosen = if aload <= bload { aidx } else { bidx };

                        trace!(
                            a.index = aidx,
                            a.load = ?aload,
                            b.index = bidx,
                            b.load = ?bload,
                            chosen = if chosen == aidx { "a" } else { "b" },
                            "any p2c"
                        );

                        return Some(chosen);
                    }
                }
            },
            Route::Block { chain, criteria} => {
                let mut idxs = (0..cache.ready_len())
                    .filter_map(|i| cache
                        .get_ready_index(i)
                        .and_then(|(_, svc)| svc.load())
                        .map(|m| (i, m)))
                    .filter(|(_, metrics)| {
                        let (first_block, last_block) = match chain {
                            -1 => (&metrics.first_block.0, &metrics.last_block.0),
                            _ => (&metrics.first_block.1, &metrics.last_block.1)
                        };

                        match criteria {
                            BlockCriteria::LogicalTime(lt) => first_block.start_lt <= *lt && *lt <= last_block.end_lt,
                            BlockCriteria::Seqno(seqno) => first_block.id.seqno <= *seqno && *seqno <= last_block.id.seqno
                        }
                    })
                    .collect();

                self.choose_from_vec(&mut idxs)
            },
            Route::Latest { chain } => {
                let groups = (0..cache.ready_len())
                    .filter_map(|i| cache
                        .get_ready_index(i)
                        .and_then(|(_, svc)| svc.load())
                        .map(|m| (i, m)))
                    .sorted_by_key(|(_, metrics)| match chain {
                        -1 => -metrics.last_block.0.id.seqno,
                        _ => -metrics.last_block.1.id.seqno
                    })
                    .group_by(|(_, metrics)| match chain {
                        -1 => metrics.last_block.0.id.seqno,
                        _ => metrics.last_block.1.id.seqno
                    });


                let mut idxs: Vec<(usize, Metrics)> = vec![];
                for (_, group) in &groups {
                    idxs = group.collect();

                    // we need at least 3 nodes in group
                    if idxs.len() > 2 {
                        break;
                    }
                }

                self.choose_from_vec(&mut idxs)
            }
        }
    }

    fn choose_from_vec(&self, idxs: &mut Vec<(usize, Metrics)>) -> Option<usize> {
        info!(route = ?self, len = idxs.len(), "choose from");

        return match idxs.len() {
            0 => None,
            1 => {
                let (aidx, _) = idxs.pop().unwrap();

                Some(aidx)
            },
            _ => {
                let (aidx, aload) = idxs.pop().unwrap();
                let (bidx, bload) = idxs.pop().unwrap();

                let chosen = if aload <= bload { aidx } else { bidx };

                trace!(
                    a.index = aidx,
                    a.load = ?aload,
                    b.index = bidx,
                    b.load = ?bload,
                    chosen = if chosen == aidx { "a" } else { "b" },
                    "any p2c"
                );

                Some(chosen)
            }
        }
    }
}


#[derive(new)]
pub struct BalanceRequest {
    pub route: Route,
    pub request: SessionRequest
}

impl<T> TryFrom<RequestableWrapper<T>> for BalanceRequest where T : Routable + Requestable {
    type Error = anyhow::Error;

    fn try_from(req: RequestableWrapper<T>) -> Result<Self, Self::Error> {
        let route = req.inner.route();

        req.inner.into_request()
            .map(SessionRequest::Atomic)
            .map(|r| {
                BalanceRequest::new(route, r)
            })
    }
}

impl From<SessionRequest> for BalanceRequest {
    fn from(request: SessionRequest) -> Self {
        let route = request.route();

        BalanceRequest::new(route, request)
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
    fn update_pending_from_discover(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<(), DiscoverError>>> {
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

        if self.services.ready_len() > 0 {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }

    fn call(&mut self, request: BalanceRequest) -> Self::Future {
        let (route, request) = (request.route, request.request);

        let index = route
            .choose(&self.services, &mut self.rng)
            .or_else(|| {
                info!(route = ?route, "fallback to any");

                Route::Any.choose(&self.services, &mut self.rng)
            })
            .expect("called before ready");

        self.services
            .call_ready_index(index, request)
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
