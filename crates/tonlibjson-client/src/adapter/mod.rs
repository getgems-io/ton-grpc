mod convert;

use crate::client::TonlibjsonClient;
use crate::tl::{
    AccountAddress, BlocksGetBlockHeader, BlocksGetMasterchainInfo, BlocksGetShards,
    BlocksGetTransactions, BlocksGetTransactionsExt, BlocksLookupBlock,
    GetShardAccountCell as TlGetShardAccountCell,
    GetShardAccountCellByTransaction as TlGetShardAccountCellByTransaction, InternalTransactionId,
    RawGetAccountState, RawGetAccountStateByTransaction, RawGetTransactionsV2, RawSendMessage,
    RawSendMessageReturnHash, SmcBoxedMethodId, SmcLoad, SmcRunGetMethod, Sync as TlSync,
    TonBlockId, TvmBoxedStackEntry,
};
use anyhow::anyhow;
use futures::future::BoxFuture;
use futures::{FutureExt, TryFutureExt, future};
use std::task::{Context, Poll};
use ton_tower::request::{
    GetAccountState, GetAccountStateByTransaction, GetAccountStateOnBlock, GetAccountTransactions,
    GetBlockHeader, GetMasterchainInfo, GetShardAccountCell, GetShardAccountCellByTransaction,
    GetShardAccountCellOnBlock, GetShards, GetTransactionIds, GetTransactions, LookUpBlockByLt,
    LookUpBlockBySeqno, RunGetMethod, SendMessage, SendMessageReturningHash, Sync,
};
use tower::{Service, ServiceExt};
pub mod make;

#[derive(Debug, Clone)]
pub struct TonlibjsonAdapter {
    inner: TonlibjsonClient,
}

impl TonlibjsonAdapter {
    pub fn new(inner: TonlibjsonClient) -> Self {
        Self { inner }
    }
}

impl Service<GetMasterchainInfo> for TonlibjsonAdapter {
    type Response = ton_tower::response::MasterchainInfo;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<BlocksGetMasterchainInfo>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, _: GetMasterchainInfo) -> Self::Future {
        self.inner
            .call(BlocksGetMasterchainInfo::default())
            .map_ok(Into::into)
            .boxed()
    }
}

impl Service<LookUpBlockBySeqno> for TonlibjsonAdapter {
    type Response = ton_tower::response::BlockIdExt;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<BlocksLookupBlock>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: LookUpBlockBySeqno) -> Self::Future {
        if req.seqno <= 0 {
            return future::err(anyhow!("seqno must be greater than 0")).boxed();
        }
        self.inner
            .call(BlocksLookupBlock::seqno(TonBlockId::new(
                req.chain, req.shard, req.seqno,
            )))
            .map_ok(Into::into)
            .boxed()
    }
}

impl Service<LookUpBlockByLt> for TonlibjsonAdapter {
    type Response = ton_tower::response::BlockIdExt;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<BlocksLookupBlock>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: LookUpBlockByLt) -> Self::Future {
        if req.lt <= 0 {
            return future::err(anyhow!("lt must be greater than 0")).boxed();
        }
        self.inner
            .call(BlocksLookupBlock::logical_time(
                TonBlockId::new(req.chain, req.shard, 0),
                req.lt,
            ))
            .map_ok(Into::into)
            .boxed()
    }
}

impl Service<GetShards> for TonlibjsonAdapter {
    type Response = Vec<ton_tower::response::BlockIdExt>;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<BlocksGetShards>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: GetShards) -> Self::Future {
        if req.block_id.workchain != -1 {
            return future::err(anyhow!("workchain must be -1")).boxed();
        }
        self.inner
            .call(BlocksGetShards::new(req.block_id.into()))
            .map_ok(|r| r.shards.into_iter().map(Into::into).collect())
            .boxed()
    }
}

impl Service<GetBlockHeader> for TonlibjsonAdapter {
    type Response = ton_tower::response::BlockHeader;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<BlocksGetBlockHeader>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: GetBlockHeader) -> Self::Future {
        self.inner
            .call(BlocksGetBlockHeader::new(req.id.into()))
            .map_ok(Into::into)
            .boxed()
    }
}

impl Service<GetTransactionIds> for TonlibjsonAdapter {
    type Response = ton_tower::response::BlockTransactions;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<BlocksGetTransactions>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: GetTransactionIds) -> Self::Future {
        self.inner
            .call(BlocksGetTransactions::unverified(
                req.block.into(),
                req.after.map(Into::into),
                req.reverse,
                req.count,
            ))
            .map(|r| r?.try_into())
            .boxed()
    }
}

impl Service<GetTransactions> for TonlibjsonAdapter {
    type Response = ton_tower::response::BlockTransactionsExt;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<BlocksGetTransactionsExt>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: GetTransactions) -> Self::Future {
        self.inner
            .call(BlocksGetTransactionsExt::unverified(
                req.block.into(),
                req.after.map(Into::into),
                req.reverse,
                req.count,
            ))
            .map(|r| r?.try_into())
            .boxed()
    }
}

impl Service<Sync> for TonlibjsonAdapter {
    type Response = ton_tower::response::BlockIdExt;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<TlSync>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, _: Sync) -> Self::Future {
        self.inner
            .call(TlSync::default())
            .map_ok(Into::into)
            .boxed()
    }
}

impl Service<SendMessage> for TonlibjsonAdapter {
    type Response = ();
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<RawSendMessage>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: SendMessage) -> Self::Future {
        self.inner
            .call(RawSendMessage::new(req.body))
            .map_ok(|_| ())
            .boxed()
    }
}

impl Service<SendMessageReturningHash> for TonlibjsonAdapter {
    type Response = String;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<RawSendMessageReturnHash>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: SendMessageReturningHash) -> Self::Future {
        self.inner
            .call(RawSendMessageReturnHash::new(req.body))
            .map_ok(|r| r.hash)
            .boxed()
    }
}

impl Service<GetAccountState> for TonlibjsonAdapter {
    type Response = ton_tower::response::AccountState;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<RawGetAccountState>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: GetAccountState) -> Self::Future {
        self.inner
            .call(RawGetAccountState::new(AccountAddress::new(&req.address)))
            .map_ok(Into::into)
            .boxed()
    }
}

impl Service<GetAccountStateOnBlock> for TonlibjsonAdapter {
    type Response = ton_tower::response::AccountState;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<crate::tl::WithBlock<RawGetAccountState>>>::poll_ready(
            &mut self.inner,
            cx,
        )
    }

    fn call(&mut self, req: GetAccountStateOnBlock) -> Self::Future {
        self.inner
            .call(crate::tl::WithBlock::new(
                req.block_id.into(),
                RawGetAccountState::new(AccountAddress::new(&req.address)),
            ))
            .map_ok(Into::into)
            .boxed()
    }
}

impl Service<GetAccountStateByTransaction> for TonlibjsonAdapter {
    type Response = ton_tower::response::AccountState;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<RawGetAccountStateByTransaction>>::poll_ready(
            &mut self.inner,
            cx,
        )
    }

    fn call(&mut self, req: GetAccountStateByTransaction) -> Self::Future {
        self.inner
            .call(RawGetAccountStateByTransaction::new(
                AccountAddress::new(&req.address),
                req.transaction_id.into(),
            ))
            .map_ok(Into::into)
            .boxed()
    }
}

impl Service<GetAccountTransactions> for TonlibjsonAdapter {
    type Response = ton_tower::response::Transactions;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<RawGetTransactionsV2>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: GetAccountTransactions) -> Self::Future {
        let from = InternalTransactionId {
            lt: req.from.lt,
            hash: req.from.hash,
        };
        self.inner
            .call(RawGetTransactionsV2::new(
                AccountAddress::new(&req.address),
                from,
                16,
                false,
            ))
            .map(|r| r?.try_into())
            .boxed()
    }
}

impl Service<GetShardAccountCell> for TonlibjsonAdapter {
    type Response = ton_tower::response::Cell;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<TlGetShardAccountCell>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: GetShardAccountCell) -> Self::Future {
        self.inner
            .call(TlGetShardAccountCell::new(AccountAddress::new(
                &req.address,
            )))
            .map_ok(Into::into)
            .boxed()
    }
}

impl Service<GetShardAccountCellOnBlock> for TonlibjsonAdapter {
    type Response = ton_tower::response::Cell;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<crate::tl::WithBlock<TlGetShardAccountCell>>>::poll_ready(
            &mut self.inner,
            cx,
        )
    }

    fn call(&mut self, req: GetShardAccountCellOnBlock) -> Self::Future {
        self.inner
            .call(crate::tl::WithBlock::new(
                req.block_id.into(),
                TlGetShardAccountCell::new(AccountAddress::new(&req.address)),
            ))
            .map_ok(Into::into)
            .boxed()
    }
}

impl Service<GetShardAccountCellByTransaction> for TonlibjsonAdapter {
    type Response = ton_tower::response::Cell;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<TlGetShardAccountCellByTransaction>>::poll_ready(
            &mut self.inner,
            cx,
        )
    }

    fn call(&mut self, req: GetShardAccountCellByTransaction) -> Self::Future {
        self.inner
            .call(TlGetShardAccountCellByTransaction::new(
                AccountAddress::new(&req.address),
                req.transaction_id.into(),
            ))
            .map_ok(Into::into)
            .boxed()
    }
}

impl Service<RunGetMethod> for TonlibjsonAdapter {
    type Response = ton_tower::response::SmcRunResult;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <TonlibjsonClient as Service<SmcLoad>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: RunGetMethod) -> Self::Future {
        let method = SmcBoxedMethodId::by_name(&req.method);
        let stack: Vec<TvmBoxedStackEntry> = req.stack.into_iter().map(Into::into).collect();

        let load = self
            .inner
            .call(SmcLoad::new(AccountAddress::new(&req.address)));
        let inner = self.inner.clone();
        async move {
            let info = load.await?;
            let resp = inner
                .oneshot(SmcRunGetMethod::new(info.id, method, stack))
                .await?;

            Ok(resp.into())
        }
        .boxed()
    }
}
