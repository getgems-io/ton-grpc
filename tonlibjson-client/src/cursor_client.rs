use std::ops::Range;
use std::task::{Context, Poll};
use tower::Service;
use anyhow::{anyhow, Result};
use crate::block::{BlockId, BlockIdExt};
use crate::session::{SessionClient, SessionRequest};

struct CursorClient {
    client: SessionClient,

    first_block: Option<BlockIdExt>,
    last_block: Option<BlockIdExt>
}

impl CursorClient {
    pub fn new(client: SessionClient) -> Self {
        Self {
            client,
            first_block: None,
            last_block: None
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

impl Service<SessionRequest> for CursorClient {
    type Response = <SessionClient as Service<SessionRequest>>::Response;
    type Error = <SessionClient as Service<SessionRequest>>::Error;
    type Future = <SessionClient as Service<SessionRequest>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.client.poll_ready(cx)
    }

    fn call(&mut self, req: SessionRequest) -> Self::Future {
        self.client.call(req)
    }
}
