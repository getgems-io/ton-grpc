use std::future::Future;
use std::mem;
use std::ops::Range;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use tower::Service;
use anyhow::{anyhow, Result};
use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt, TryFutureExt};
use tower::limit::ConcurrencyLimit;
use tower::load::{CompleteOnResponse, PeakEwma};
use tracing::error;
use tracing::log::{log, warn};
use crate::block::{BlockHeader, BlockId, BlockIdExt};
use crate::client::Client;
use crate::session::{SessionClient, SessionRequest};

enum State {
    Init,
    Future(Pin<Box<dyn Future<Output=(Result<BlockHeader>, Result<BlockHeader>, ConcurrencyLimit<SessionClient>)> + Send>>),
    Ready
}

pub struct CursorClient {
    client: Option<ConcurrencyLimit<SessionClient>>,

    first_block: Option<BlockHeader>,
    last_block: Option<BlockHeader>,

    state: State,
}

impl CursorClient {
    pub fn new(client: ConcurrencyLimit<SessionClient>) -> Self {
        Self {
            client: Some(client),
            first_block: None,
            last_block: None,
            state: State::Init
        }
    }

    pub fn first_block(&self) -> Result<&BlockHeader> {
        self.first_block.as_ref()
            .ok_or(anyhow!("first block is unknown"))
    }

    pub fn last_block(&self) -> Result<&BlockHeader> {
        self.last_block.as_ref()
            .ok_or(anyhow!("last block is unknown"))
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
                    let low = client.get_mut().get_mut();

                    warn!("start sync");
                    let rhs = low.synchronize()
                        .await;

                    warn!("start searching");
                    let lhs = low.find_first_block()
                        .await;

                    (lhs, rhs, client)
                }.boxed())
            },
            State::Future(fut) => {
                let (first_block, last_block, client) = ready!(fut.poll_unpin(cx));
                self.client.replace(client);

                match (first_block, last_block) {
                    (Ok(f), Ok(l)) => {
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
