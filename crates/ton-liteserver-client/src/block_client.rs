use crate::client::LiteServerClient;
use crate::convert::{
    block_header_to_ton_client, block_transactions_to_ton_client, transaction_to_ton_client,
};
use crate::tl::{
    LiteServerGetAllShardsInfo, LiteServerGetBlock, LiteServerGetBlockHeader,
    LiteServerGetMasterchainInfo, LiteServerListBlockTransactions, LiteServerLookupBlock,
    TonNodeBlockId, TonNodeBlockIdExt, True,
};
use crate::tlb::block::Block;
use crate::tlb::block_header::BlockHeader;
use crate::tlb::merkle_proof::MerkleProof;
use crate::tlb::shard_descr::ShardDescr;
use crate::tlb::shard_hashes::ShardHashes;
use crate::tlb::transaction::Transaction;
use anyhow::anyhow;
use std::sync::Arc;
use ton_client::{BlockTransactions, BlockTransactionsExt, MasterchainInfo, ShortTxId};
use toner::tlb::BoC;
use toner::tlb::bits::de::{unpack_bytes, unpack_bytes_fully};
use tower::ServiceExt;

#[async_trait::async_trait]
impl ton_client::BlockClient for LiteServerClient {
    async fn get_masterchain_info(&self) -> anyhow::Result<MasterchainInfo> {
        self.clone()
            .oneshot(LiteServerGetMasterchainInfo::default())
            .await
            .map(Into::into)
            .map_err(|e| anyhow!(e))
    }

    async fn look_up_block_by_seqno(
        &self,
        chain: i32,
        shard: i64,
        seqno: i32,
    ) -> anyhow::Result<ton_client::BlockIdExt> {
        if seqno <= 0 {
            return Err(anyhow!("seqno must be greater than 0"));
        }

        let header = self
            .clone()
            .oneshot(LiteServerLookupBlock::seqno(TonNodeBlockId::new(
                chain, shard, seqno,
            )))
            .await
            .map_err(|e| anyhow!(e))?;

        Ok(ton_client::BlockIdExt::from(header.id))
    }

    async fn look_up_block_by_lt(
        &self,
        chain: i32,
        shard: i64,
        lt: i64,
    ) -> anyhow::Result<ton_client::BlockIdExt> {
        if lt <= 0 {
            return Err(anyhow!("lt must be greater than 0"));
        }

        let header = self
            .clone()
            .oneshot(LiteServerLookupBlock {
                mode: 0,
                id: TonNodeBlockId::new(chain, shard, 0),
                lt: Some(lt),
                utime: None,
            })
            .await
            .map_err(|e| anyhow!(e))?;

        Ok(ton_client::BlockIdExt::from(header.id))
    }

    async fn get_shards_by_block_id(
        &self,
        block_id: ton_client::BlockIdExt,
    ) -> anyhow::Result<Vec<ton_client::BlockIdExt>> {
        let id: TonNodeBlockIdExt = block_id.into();
        if id.workchain != -1 {
            return Err(anyhow!("workchain must be -1"));
        }

        let response = self
            .clone()
            .oneshot(LiteServerGetAllShardsInfo::new(id))
            .await
            .map_err(|e| anyhow!(e))?;

        let boc: BoC = unpack_bytes(&response.data, ())?;
        let root = boc
            .single_root()
            .ok_or_else(|| anyhow!("single root expected"))?;
        let shard_hashes: ShardHashes = root.parse_fully(())?;

        let block_ids = shard_hashes
            .iter()
            .flat_map(|(workchain_id, shards)| {
                shards
                    .iter()
                    .map(move |shard: &ShardDescr| ton_client::BlockIdExt {
                        workchain: *workchain_id as i32,
                        shard: shard.next_validator_shard as i64,
                        seqno: shard.seq_no as i32,
                        root_hash: hex::encode(shard.root_hash),
                        file_hash: hex::encode(shard.file_hash),
                    })
            })
            .collect();

        Ok(block_ids)
    }

    async fn get_block_header(
        &self,
        id: ton_client::BlockIdExt,
    ) -> anyhow::Result<ton_client::BlockHeader> {
        let block_id: TonNodeBlockIdExt = id.clone().into();

        let response = self
            .clone()
            .oneshot(LiteServerGetBlockHeader::new(block_id))
            .await
            .map_err(|e| anyhow!(e))?;

        let boc: BoC = unpack_bytes_fully(&response.header_proof, ())?;
        let header: BlockHeader = boc
            .single_root()
            .ok_or_else(|| anyhow!("single root expected"))?
            .parse_fully_as::<_, MerkleProof<_>>(())?;

        Ok(block_header_to_ton_client(id, header))
    }

    async fn blocks_get_transactions(
        &self,
        block: &ton_client::BlockIdExt,
        after: Option<ShortTxId>,
        reverse: bool,
        count: i32,
    ) -> anyhow::Result<BlockTransactions> {
        let id: TonNodeBlockIdExt = block.clone().into();

        let response = self
            .clone()
            .oneshot(LiteServerListBlockTransactions {
                id,
                mode: 0,
                count,
                after: after.map(Into::into),
                reverse_order: if reverse { Some(True {}) } else { None },
                want_proof: None,
            })
            .await
            .map_err(|e| anyhow!(e))?;

        block_transactions_to_ton_client(response)
    }

    async fn blocks_get_transactions_ext(
        &self,
        block: &ton_client::BlockIdExt,
        _after: Option<ShortTxId>,
        _reverse: bool,
        _count: i32,
    ) -> anyhow::Result<BlockTransactionsExt> {
        let id: TonNodeBlockIdExt = block.clone().into();

        let response = self
            .clone()
            .oneshot(LiteServerGetBlock::new(id))
            .await
            .map_err(|e| anyhow!(e))?;

        let boc: BoC = unpack_bytes(&response.data, ())?;
        let root = boc
            .single_root()
            .ok_or_else(|| anyhow!("single root expected"))?;
        let parsed: Block = root.parse_fully(())?;
        let workchain = parsed.info.shard.workchain_id;

        let mut transactions = Vec::new();
        for (_account_key, account_block) in &parsed.extra.account_blocks.0 {
            for (_tx_key, tx_cell) in &account_block.transactions {
                let tx_cell = Arc::new(tx_cell.clone());
                let tx: Transaction = tx_cell.parse_fully(())?;
                transactions.push(transaction_to_ton_client(workchain, &tx_cell, tx)?);
            }
        }

        Ok(BlockTransactionsExt {
            incomplete: false,
            transactions,
        })
    }
}
