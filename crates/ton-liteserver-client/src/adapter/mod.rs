mod account;
mod block;
mod convert;
pub mod make;
mod message;
mod smc;

use crate::adapter::convert::{
    block_header_to_ton_client, block_transactions_to_ton_client, shard_descr_to_block_id_ext,
    transaction_to_ton_client,
};
use crate::client::LiteServerClient;
use crate::tl::{
    BoxedBool, Int256, LiteServerAccountId, LiteServerGetAccountState, LiteServerGetAllShardsInfo,
    LiteServerGetBlockHeader, LiteServerGetMasterchainInfo,
    LiteServerGetTransactions as LiteServerGetTransactionsRequest, LiteServerListBlockTransactions,
    LiteServerListBlockTransactionsExt, LiteServerLookupBlock, LiteServerSendMessage,
    TonNodeBlockId, TonNodeBlockIdExt, True,
};
use crate::tlb::block_header::BlockHeader;
use crate::tlb::merkle_proof::MerkleProof;
use crate::tlb::shard_descr::ShardDescr;
use crate::tlb::shard_hashes::ShardHashes;
use crate::tlb::transaction::Transaction;
use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as base64_standard;
use futures::future::BoxFuture;
use futures::{FutureExt, TryFutureExt};
use std::task::{Context, Poll};
use ton_tower::request::*;
use ton_tower::response::TransactionId;
use toner::tlb::BoC;
use toner::tlb::bits::de::{unpack_bytes, unpack_bytes_fully};
use tower::Service;

#[derive(Clone)]
pub struct LiteServerAdapter {
    inner: LiteServerClient,
}

impl LiteServerAdapter {
    pub fn new(inner: LiteServerClient) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &LiteServerClient {
        &self.inner
    }

    pub fn into_inner(self) -> LiteServerClient {
        self.inner
    }
}

impl Service<GetMasterchainInfo> for LiteServerAdapter {
    type Response = ton_tower::response::MasterchainInfo;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerGetMasterchainInfo>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, _: GetMasterchainInfo) -> Self::Future {
        self.inner
            .call(LiteServerGetMasterchainInfo::default())
            .map_ok(Into::into)
            .map_err(Into::into)
            .boxed()
    }
}

impl Service<Sync> for LiteServerAdapter {
    type Response = ton_tower::response::BlockIdExt;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerGetMasterchainInfo>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, _: Sync) -> Self::Future {
        self.inner
            .call(LiteServerGetMasterchainInfo::default())
            .map_ok(|info| ton_tower::response::MasterchainInfo::from(info).last)
            .map_err(Into::into)
            .boxed()
    }
}

impl Service<LookUpBlockBySeqno> for LiteServerAdapter {
    type Response = ton_tower::response::BlockIdExt;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerLookupBlock>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: LookUpBlockBySeqno) -> Self::Future {
        if req.seqno <= 0 {
            return futures::future::err(anyhow!("seqno must be greater than 0")).boxed();
        }
        self.inner
            .call(LiteServerLookupBlock::seqno(TonNodeBlockId::new(
                req.chain, req.shard, req.seqno,
            )))
            .map_err(Into::into)
            .and_then(async |response| {
                block::verify_header_proof(&response.header_proof, &response.id.root_hash)?;

                Ok(ton_tower::response::BlockIdExt::from(response.id))
            })
            .boxed()
    }
}

impl Service<LookUpBlockByLt> for LiteServerAdapter {
    type Response = ton_tower::response::BlockIdExt;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerLookupBlock>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: LookUpBlockByLt) -> Self::Future {
        if req.lt <= 0 {
            return futures::future::err(anyhow!("lt must be greater than 0")).boxed();
        }
        self.inner
            .call(LiteServerLookupBlock {
                mode: 0,
                id: TonNodeBlockId::new(req.chain, req.shard, 0),
                lt: Some(req.lt),
                utime: None,
            })
            .map_err(Into::into)
            .and_then(async |response| {
                block::verify_header_proof(&response.header_proof, &response.id.root_hash)?;

                Ok(ton_tower::response::BlockIdExt::from(response.id))
            })
            .boxed()
    }
}

impl Service<GetShards> for LiteServerAdapter {
    type Response = Vec<ton_tower::response::BlockIdExt>;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerGetAllShardsInfo>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: GetShards) -> Self::Future {
        let id: TonNodeBlockIdExt = req.block_id.into();
        if id.workchain != -1 {
            return futures::future::err(anyhow!("workchain must be -1")).boxed();
        }
        let expected_root_hash = id.root_hash;

        self.inner
            .call(LiteServerGetAllShardsInfo::new(id))
            .map_err(Into::into)
            .and_then(async move |response| {
                // TODO verify data inclusion in proof via ShardState traversal (needs MaybePruned)
                block::verify_block_proof(&response.proof, &expected_root_hash)?;

                let boc: BoC = unpack_bytes(&response.data, ())?;
                let root = boc
                    .single_root()
                    .ok_or_else(|| anyhow!("single root expected"))?;
                let shard_hashes: ShardHashes = root.parse_fully(())?;

                let block_ids = shard_hashes
                    .iter()
                    .flat_map(|(workchain_id, shards)| {
                        shards.iter().map(move |shard: &ShardDescr| {
                            shard_descr_to_block_id_ext(*workchain_id as i32, shard)
                        })
                    })
                    .collect();

                Ok(block_ids)
            })
            .boxed()
    }
}

impl Service<GetBlockHeader> for LiteServerAdapter {
    type Response = ton_tower::response::BlockHeader;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerGetBlockHeader>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: GetBlockHeader) -> Self::Future {
        let block_id: TonNodeBlockIdExt = req.id.clone().into();
        let expected_root_hash = block_id.root_hash;

        self.inner
            .call(LiteServerGetBlockHeader::new(block_id))
            .map_err(Into::into)
            .and_then(async move |response| {
                let boc: BoC = unpack_bytes_fully(&response.header_proof, ())?;
                let root = boc
                    .single_root()
                    .ok_or_else(|| anyhow!("single root expected"))?;

                let proof: MerkleProof<BlockHeader> = root.parse_fully(())?;
                if proof.virtual_hash != expected_root_hash {
                    return Err(anyhow!(
                        "block header proof root hash mismatch: expected {}, got {}",
                        hex::encode(expected_root_hash),
                        hex::encode(proof.virtual_hash)
                    ));
                }

                Ok(block_header_to_ton_client(req.id, proof.virtual_root))
            })
            .boxed()
    }
}

impl Service<GetTransactionIds> for LiteServerAdapter {
    type Response = ton_tower::response::BlockTransactions;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerListBlockTransactions>>::poll_ready(
            &mut self.inner,
            cx,
        )
        .map_err(Into::into)
    }

    fn call(&mut self, req: GetTransactionIds) -> Self::Future {
        let id: TonNodeBlockIdExt = req.block.into();
        let expected_root_hash = id.root_hash;

        let mode = block::list_block_transactions_mode(req.after.is_some(), req.reverse, true);

        self.inner
            .call(LiteServerListBlockTransactions {
                id,
                mode,
                count: req.count,
                after: req.after.map(Into::into),
                reverse_order: if req.reverse { Some(True {}) } else { None },
                want_proof: Some(True {}),
            })
            .map_err(Into::into)
            .and_then(async move |response| {
                // TODO verify transaction ids against proof via ShardAccountBlocks dict (needs MaybePruned)
                block::verify_block_proof(&response.proof, &expected_root_hash)?;

                block_transactions_to_ton_client(response)
            })
            .boxed()
    }
}

impl Service<GetTransactions> for LiteServerAdapter {
    type Response = ton_tower::response::BlockTransactionsExt;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerListBlockTransactionsExt>>::poll_ready(
            &mut self.inner,
            cx,
        )
        .map_err(Into::into)
    }

    fn call(&mut self, req: GetTransactions) -> Self::Future {
        let id: TonNodeBlockIdExt = req.block.into();
        let expected_root_hash = id.root_hash;

        let mode = block::list_block_transactions_mode(req.after.is_some(), req.reverse, true);

        self.inner
            .call(LiteServerListBlockTransactionsExt {
                id,
                mode,
                count: req.count,
                after: req.after.map(Into::into),
                reverse_order: if req.reverse { Some(True {}) } else { None },
                want_proof: Some(True {}),
            })
            .map_err(Into::into)
            .and_then(async move |response| {
                let incomplete = matches!(response.incomplete, BoxedBool::BoolTrue(_));
                let workchain = response.id.workchain;

                block::verify_block_proof(&response.proof, &expected_root_hash)?;

                let mut transactions = Vec::new();
                if !response.transactions.is_empty() {
                    let boc: BoC = BoC::deserialize(&response.transactions)?;
                    transactions.reserve(boc.roots().len());

                    for root in boc.into_roots() {
                        let tx: Transaction = root.parse_fully(())?;
                        transactions.push(transaction_to_ton_client(workchain, &root, tx)?);
                    }
                }

                Ok(ton_tower::response::BlockTransactionsExt {
                    incomplete,
                    transactions,
                })
            })
            .boxed()
    }
}

impl Service<SendMessage> for LiteServerAdapter {
    type Response = ();
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerSendMessage>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: SendMessage) -> Self::Future {
        let body = match message::decode_message_body(&req.body) {
            Ok(body) => body,
            Err(e) => return futures::future::err(e).boxed(),
        };

        self.inner
            .call(LiteServerSendMessage { body })
            .map_err(Into::into)
            .and_then(async |response| {
                if response.status != message::SEND_MSG_STATUS_OK {
                    return Err(anyhow!(
                        "unexpected liteServer.sendMsgStatus: {}",
                        response.status
                    ));
                }

                Ok(())
            })
            .boxed()
    }
}

impl Service<SendMessageReturningHash> for LiteServerAdapter {
    type Response = String;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerSendMessage>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: SendMessageReturningHash) -> Self::Future {
        let (body, hash) = match message::decode_message_body(&req.body)
            .and_then(|body| message::compute_message_hash(&body).map(|hash| (body, hash)))
        {
            Ok(pair) => pair,
            Err(e) => return futures::future::err(e).boxed(),
        };

        self.inner
            .call(LiteServerSendMessage { body })
            .map_err(Into::into)
            .and_then(async move |response| {
                if response.status != message::SEND_MSG_STATUS_OK {
                    return Err(anyhow!(
                        "unexpected liteServer.sendMsgStatus: {}",
                        response.status
                    ));
                }

                Ok(hash)
            })
            .boxed()
    }
}

impl Service<GetAccountState> for LiteServerAdapter {
    type Response = ton_tower::response::AccountState;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerGetMasterchainInfo>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: GetAccountState) -> Self::Future {
        let client = self.inner.clone();

        self.inner
            .call(LiteServerGetMasterchainInfo::default())
            .map_err(Into::into)
            .and_then(async move |mc| {
                account::get_account_state_inner(client, req.address, mc.last).await
            })
            .boxed()
    }
}

impl Service<GetAccountStateOnBlock> for LiteServerAdapter {
    type Response = ton_tower::response::AccountState;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerGetAccountState>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: GetAccountStateOnBlock) -> Self::Future {
        let id: TonNodeBlockIdExt = req.block_id.into();
        let request = account::account_state_request(&req.address, id);

        self.inner
            .call(request)
            .map_err(Into::into)
            .and_then(async |response| account::account_state_from_response(response))
            .boxed()
    }
}

impl Service<GetAccountStateByTransaction> for LiteServerAdapter {
    type Response = ton_tower::response::AccountState;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerGetMasterchainInfo>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: GetAccountStateByTransaction) -> Self::Future {
        let client = self.inner.clone();

        self.inner
            .call(LiteServerGetMasterchainInfo::default())
            .map_err(Into::into)
            .and_then(async move |mc| {
                let address = req.address;
                let tx = req.transaction_id;
                let block_id =
                    account::lookup_block_by_transaction(&client, mc.last, &address, &tx).await?;
                account::get_account_state_inner(client, address, block_id).await
            })
            .boxed()
    }
}

impl Service<GetAccountTransactions> for LiteServerAdapter {
    type Response = ton_tower::response::Transactions;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerGetTransactionsRequest>>::poll_ready(
            &mut self.inner,
            cx,
        )
        .map_err(Into::into)
    }

    fn call(&mut self, req: GetAccountTransactions) -> Self::Future {
        let address = req.address;
        let from = req.from;

        let workchain = address.workchain_id();
        let hash: Int256 = match account::decode_tx_hash(&from.hash) {
            Ok(hash) => hash,
            Err(e) => return futures::future::err(e).boxed(),
        };
        let account = LiteServerAccountId {
            workchain,
            id: *address.to_internal(),
        };

        self.inner
            .call(LiteServerGetTransactionsRequest {
                count: account::DEFAULT_TX_BATCH,
                account,
                lt: from.lt,
                hash,
            })
            .map_err(Into::into)
            .and_then(async move |response| {
                let mut transactions: Vec<ton_tower::response::Transaction> = Vec::new();
                let mut previous_transaction_id: Option<TransactionId> = None;

                if !response.transactions.is_empty() {
                    let boc = BoC::deserialize(&response.transactions)?;
                    let roots = boc.into_roots();

                    for root in roots.iter() {
                        let tx: Transaction = root.parse_fully(())?;
                        let workchain_tx = workchain;
                        let raw_tx = transaction_to_ton_client(workchain_tx, root, tx)?;
                        transactions.push(raw_tx);
                    }

                    if let Some(last_root) = roots.last() {
                        let last_tx: Transaction = last_root.parse_fully(())?;
                        if last_tx.prev_trans_lt > 0 {
                            previous_transaction_id = Some(TransactionId {
                                lt: last_tx.prev_trans_lt as i64,
                                hash: base64_standard.encode(last_tx.prev_trans_hash),
                            });
                        }
                    }
                }

                Ok(ton_tower::response::Transactions {
                    transactions,
                    previous_transaction_id,
                })
            })
            .boxed()
    }
}

impl Service<GetShardAccountCell> for LiteServerAdapter {
    type Response = ton_tower::response::Cell;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerGetMasterchainInfo>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: GetShardAccountCell) -> Self::Future {
        let client = self.inner.clone();

        self.inner
            .call(LiteServerGetMasterchainInfo::default())
            .map_err(Into::into)
            .and_then(async move |mc| {
                account::get_shard_account_cell_inner(client, req.address, mc.last).await
            })
            .boxed()
    }
}

impl Service<GetShardAccountCellOnBlock> for LiteServerAdapter {
    type Response = ton_tower::response::Cell;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerGetAccountState>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: GetShardAccountCellOnBlock) -> Self::Future {
        let id: TonNodeBlockIdExt = req.block_id.into();
        let request = account::account_state_request(&req.address, id);

        self.inner
            .call(request)
            .map_err(Into::into)
            .and_then(async |response| account::shard_account_cell_from_response(response))
            .boxed()
    }
}

impl Service<GetShardAccountCellByTransaction> for LiteServerAdapter {
    type Response = ton_tower::response::Cell;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerGetMasterchainInfo>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: GetShardAccountCellByTransaction) -> Self::Future {
        let client = self.inner.clone();

        self.inner
            .call(LiteServerGetMasterchainInfo::default())
            .map_err(Into::into)
            .and_then(async move |mc| {
                let address = req.address;
                let tx = req.transaction_id;
                let block_id =
                    account::lookup_block_by_transaction(&client, mc.last, &address, &tx).await?;
                account::get_shard_account_cell_inner(client, address, block_id).await
            })
            .boxed()
    }
}

impl Service<RunGetMethod> for LiteServerAdapter {
    type Response = ton_tower::response::SmcRunResult;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <LiteServerClient as Service<LiteServerGetMasterchainInfo>>::poll_ready(&mut self.inner, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: RunGetMethod) -> Self::Future {
        let client = self.inner.clone();

        self.inner
            .call(LiteServerGetMasterchainInfo::default())
            .map_err(Into::into)
            .and_then(async move |mc| {
                let address = req.address;
                let method = req.method;
                let stack = req.stack;

                smc::run_get_method_inner(client, address, mc.last, &method, stack).await
            })
            .boxed()
    }
}
