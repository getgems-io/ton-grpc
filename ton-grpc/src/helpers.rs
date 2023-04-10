use anyhow::Result;
use tonlibjson_client::block;
use tonlibjson_client::ton::TonClient;
use crate::ton;

#[tracing::instrument(skip_all, err)]
pub async fn extend_block_id(client: &TonClient, block_id: &ton::BlockId) -> Result<block::BlockIdExt> {
    if let (Some(root_hash), Some(file_hash)) = (&block_id.root_hash, &block_id.file_hash) {
        Ok(block::BlockIdExt::new(
            block_id.workchain,
            block_id.shard,
            block_id.seqno,
            root_hash.clone(),
            file_hash.clone()
        ))
    } else {
        client.look_up_block_by_seqno(block_id.workchain, block_id.shard, block_id.seqno).await
    }
}
