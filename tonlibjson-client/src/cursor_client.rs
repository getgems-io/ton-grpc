use std::ops::Range;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use tower::Service;
use anyhow::{anyhow, Result};
use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt};
use tower::limit::ConcurrencyLimit;
use tracing::error;
use tracing::log::{log, warn};
use crate::block::{BlockId, BlockIdExt};
use crate::cursor_client::State::Init;
use crate::session::{SessionClient, SessionRequest};

enum State<'a> {
    Init,
    Future(BoxFuture<'a, Result<(BlockIdExt, BlockIdExt)>>),
    Ready
}

pub struct CursorClient<'a> {
    client: ConcurrencyLimit<SessionClient>,

    pub first_block: Option<BlockIdExt>,
    pub last_block: Option<BlockIdExt>,

    state: State<'a>,
}

impl CursorClient<'_> {
    pub fn new(client: ConcurrencyLimit<SessionClient>) -> Self {
        Self {
            client,
            first_block: None,
            last_block: None,
            state: Init
        }
    }

    fn first_block(&self) -> Result<&BlockIdExt> {
        self.first_block.as_ref()
            .ok_or(anyhow!("first block is unknown"))
    }

    fn last_block(&self) -> Result<&BlockIdExt> {
        self.last_block.as_ref()
            .ok_or(anyhow!("last block is unknown"))
    }

    fn get_range(&self) -> Result<Range<&BlockIdExt>> {
        Ok(self.first_block()? .. self.last_block()?)
    }
}

impl Service<SessionRequest> for CursorClient<'_> {
    type Response = <SessionClient as Service<SessionRequest>>::Response;
    type Error = <SessionClient as Service<SessionRequest>>::Error;
    type Future = <SessionClient as Service<SessionRequest>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.state = match &mut self.state {
            State::Init => {
                let mut client = self.client.get_mut().get_mut().clone();

                State::Future(async move {
                    warn!("start sync");
                    let rhs = client.synchronize().await?;
                    warn!("start searching");
                    let lhs = client.find_first_block().await?;

                    Ok((lhs, rhs))
                }.boxed())
            },
            State::Future(fut) => {
                match ready!(fut.poll_unpin(cx)) {
                    Err(e) => {
                        error!("initialization error: {:?}", e);

                        State::Init
                    }
                    Ok((first_block, last_block)) => {
                        self.first_block = Some(first_block);
                        self.last_block = Some(last_block);

                        State::Ready
                    }
                }
            },
            State::Ready => return self.client.poll_ready(cx)
        };

        cx.waker().wake_by_ref();

        Poll::Pending
    }

    fn call(&mut self, req: SessionRequest) -> Self::Future {
        Box::pin(self.client.call(req))
    }
}
