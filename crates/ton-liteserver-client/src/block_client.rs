use crate::client::LiteServerClient;
use crate::convert::{
    block_header_to_ton_client, block_transactions_to_ton_client, transaction_to_ton_client,
};
use crate::tl::{
    BoxedBool, LiteServerGetAllShardsInfo, LiteServerGetBlockHeader, LiteServerGetMasterchainInfo,
    LiteServerListBlockTransactions, LiteServerListBlockTransactionsExt, LiteServerLookupBlock,
    TonNodeBlockId, TonNodeBlockIdExt, True,
};
use crate::tlb::block_header::BlockHeader;
use crate::tlb::merkle_proof::MerkleProof;
use crate::tlb::shard_descr::ShardDescr;
use crate::tlb::shard_hashes::ShardHashes;
use crate::tlb::transaction::Transaction;
use anyhow::anyhow;
use ton_client::{BlockTransactions, BlockTransactionsExt, MasterchainInfo, ShortTxId};
use toner::tlb::bits::de::{unpack_bytes, unpack_bytes_fully};
use toner::tlb::{BoC, Cell};
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

        let response = self
            .clone()
            .oneshot(LiteServerLookupBlock::seqno(TonNodeBlockId::new(
                chain, shard, seqno,
            )))
            .await
            .map_err(|e| anyhow!(e))?;

        verify_header_proof(&response.header_proof, &response.id.root_hash)?;

        Ok(ton_client::BlockIdExt::from(response.id))
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

        let response = self
            .clone()
            .oneshot(LiteServerLookupBlock {
                mode: 0,
                id: TonNodeBlockId::new(chain, shard, 0),
                lt: Some(lt),
                utime: None,
            })
            .await
            .map_err(|e| anyhow!(e))?;

        verify_header_proof(&response.header_proof, &response.id.root_hash)?;

        Ok(ton_client::BlockIdExt::from(response.id))
    }

    async fn get_shards_by_block_id(
        &self,
        block_id: ton_client::BlockIdExt,
    ) -> anyhow::Result<Vec<ton_client::BlockIdExt>> {
        let id: TonNodeBlockIdExt = block_id.into();
        if id.workchain != -1 {
            return Err(anyhow!("workchain must be -1"));
        }
        let expected_root_hash = id.root_hash;

        let response = self
            .clone()
            .oneshot(LiteServerGetAllShardsInfo::new(id))
            .await
            .map_err(|e| anyhow!(e))?;

        // TODO verify data inclusion in proof via ShardState traversal (needs MaybePruned)
        verify_block_proof(&response.proof, &expected_root_hash)?;

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
        let expected_root_hash = block_id.root_hash;

        let response = self
            .clone()
            .oneshot(LiteServerGetBlockHeader::new(block_id))
            .await
            .map_err(|e| anyhow!(e))?;

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

        Ok(block_header_to_ton_client(id, proof.virtual_root))
    }

    async fn blocks_get_transactions(
        &self,
        block: &ton_client::BlockIdExt,
        after: Option<ShortTxId>,
        reverse: bool,
        count: i32,
    ) -> anyhow::Result<BlockTransactions> {
        let id: TonNodeBlockIdExt = block.clone().into();
        let expected_root_hash = id.root_hash;

        let response = self
            .clone()
            .oneshot(LiteServerListBlockTransactions {
                id,
                mode: 0,
                count,
                after: after.map(Into::into),
                reverse_order: if reverse { Some(True {}) } else { None },
                want_proof: Some(True {}),
            })
            .await
            .map_err(|e| anyhow!(e))?;

        // TODO verify transaction ids against proof via ShardAccountBlocks dict (needs MaybePruned)
        verify_block_proof(&response.proof, &expected_root_hash)?;

        block_transactions_to_ton_client(response)
    }

    async fn blocks_get_transactions_ext(
        &self,
        block: &ton_client::BlockIdExt,
        after: Option<ShortTxId>,
        reverse: bool,
        count: i32,
    ) -> anyhow::Result<BlockTransactionsExt> {
        let id: TonNodeBlockIdExt = block.clone().into();
        let expected_root_hash = id.root_hash;

        let response = self
            .clone()
            .oneshot(LiteServerListBlockTransactionsExt {
                id,
                mode: 0,
                count,
                after: after.map(Into::into),
                reverse_order: if reverse { Some(True {}) } else { None },
                want_proof: Some(True {}),
            })
            .await
            .map_err(|e| anyhow!(e))?;

        let incomplete = matches!(response.incomplete, BoxedBool::BoolTrue(_));
        let workchain = response.id.workchain;

        verify_block_proof(&response.proof, &expected_root_hash)?;

        let mut transactions = Vec::new();
        if !response.transactions.is_empty() {
            let boc: BoC = BoC::deserialize(&response.transactions)?;
            transactions.reserve(boc.roots().len());

            for root in boc.into_roots() {
                let tx: Transaction = root.parse_fully(())?;
                transactions.push(transaction_to_ton_client(workchain, &root, tx)?);
            }
        }

        Ok(BlockTransactionsExt {
            incomplete,
            transactions,
        })
    }
}

fn verify_header_proof(proof_bytes: &[u8], expected_root_hash: &[u8; 32]) -> anyhow::Result<()> {
    let boc: BoC = unpack_bytes_fully(proof_bytes, ())?;
    let root = boc
        .single_root()
        .ok_or_else(|| anyhow!("header proof: single root expected"))?;

    let proof: MerkleProof<Cell> = root.parse_fully(())?;
    if &proof.virtual_hash != expected_root_hash {
        return Err(anyhow!(
            "header proof root hash mismatch: expected {}, got {}",
            hex::encode(expected_root_hash),
            hex::encode(proof.virtual_hash)
        ));
    }

    Ok(())
}

// TODO verify individual transaction inclusion via ShardAccountBlocks dict traversal in proof
fn verify_block_proof(proof_bytes: &[u8], expected_root_hash: &[u8; 32]) -> anyhow::Result<()> {
    if proof_bytes.is_empty() {
        return Err(anyhow!("empty proof"));
    }

    let boc: BoC = BoC::deserialize(proof_bytes)?;
    let root = boc
        .single_root()
        .ok_or_else(|| anyhow!("proof: single root expected"))?;

    let proof: MerkleProof<Cell> = root.parse_fully(())?;

    if &proof.virtual_hash != expected_root_hash {
        return Err(anyhow!(
            "proof root hash mismatch: expected {}, got {}",
            hex::encode(expected_root_hash),
            hex::encode(proof.virtual_hash)
        ));
    }

    Ok(())
}
