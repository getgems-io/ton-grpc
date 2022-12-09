use std::cmp::Ordering;
use std::future::Future;
use std::mem;
use std::ops::Range;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use tower::{Service, ServiceExt};
use anyhow::{anyhow, Result};
use async_stream::stream;
use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt, TryFutureExt, TryStreamExt};
use serde_json::Value;
use tower::limit::ConcurrencyLimit;
use tower::load::{CompleteOnResponse, PeakEwma};
use tower::load::peak_ewma::Cost;
use tracing::error;
use tracing::log::{log, warn};
use crate::block::{BlockHeader, BlockId, BlockIdExt};
use crate::client::Client;
use crate::session::{SessionClient, SessionRequest};

enum State {
    Init,
    Future(Pin<Box<dyn Future<Output=(Result<Value>, Result<Value>, ConcurrencyLimit<PeakEwma<SessionClient>>)> + Send>>),
    Ready
}

pub struct CursorClient {
    client: Option<ConcurrencyLimit<PeakEwma<SessionClient>>>,

    first_block: Option<BlockHeader>,
    last_block: Option<BlockHeader>,

    state: State,
}

impl CursorClient {
    pub fn new(client: ConcurrencyLimit<PeakEwma<SessionClient>>) -> Self {
        Self {
            client: Some(client),
            first_block: None,
            last_block: None,
            state: State::Init
        }
    }
}

impl Service<SessionRequest> for CursorClient {
    type Response = <SessionClient as Service<SessionRequest>>::Response;
    type Error = <SessionClient as Service<SessionRequest>>::Error;
    type Future = <SessionClient as Service<SessionRequest>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.state = match &mut self.state {
            State::Init => {
                let mut client = self.client
                    .take()
                    .expect("client must be provided");

                State::Future(async move {
                    let mut low = client.get_mut();

                    let req = futures::stream::iter(vec![SessionRequest::FindFirsBlock {}, SessionRequest::Synchronize {}]);
                    let mut resp = low.call_all(req)
                        .map_err(|e| anyhow!(e));

                    let lhs = resp.next().await.unwrap();
                    let rhs = resp.next().await.unwrap();

                    (lhs, rhs, client)
                }.boxed())
            },
            State::Future(fut) => {
                let (first_block, last_block, client) = ready!(fut.poll_unpin(cx));
                self.client.replace(client);

                match (first_block, last_block) {
                    (Ok(f), Ok(l)) => {
                        let f = serde_json::from_value::<BlockHeader>(f).unwrap();
                        let l = serde_json::from_value::<BlockHeader>(l).unwrap();
                        self.first_block.replace(f);
                        self.last_block.replace(l);

                        State::Ready
                    },
                    (Err(e), _) | (_, Err(e)) => {
                        error!("error occurred during client initialization: {}", e);

                        State::Init
                    },
                }
            },
            State::Ready => return self.client.as_mut().unwrap().poll_ready(cx)
        };

        cx.waker().wake_by_ref();

        Poll::Pending
    }

    fn call(&mut self, req: SessionRequest) -> Self::Future {
        Box::pin(self.client.as_mut().expect("ready must be called").call(req))
    }
}


impl tower::load::Load for CursorClient {
    type Metric = Metrics;

    fn load(&self) -> Self::Metric {
        Metrics {
            first_block: self.first_block.clone(),
            last_block: self.last_block.clone(),
            ewma: self.client.as_ref().map(|c| c.get_ref().load())
        }
    }
}

#[derive(Debug)]
pub struct Metrics {
    pub first_block: Option<BlockHeader>,
    pub last_block: Option<BlockHeader>,
    pub ewma: Option<Cost>
}

impl PartialEq<Self> for Metrics {
    fn eq(&self, other: &Self) -> bool {
        self.ewma.eq(&other.ewma)
    }
}

impl PartialOrd<Self> for Metrics {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.ewma.partial_cmp(&other.ewma)
    }
}
