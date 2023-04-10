use anyhow::Result;
use tonlibjson_client::block;
use tonlibjson_client::block::InternalTransactionId;
use tonlibjson_client::ton::TonClient;
use crate::ton;
use crate::ton::get_account_transactions_request::From::{FromBlockId, FromTransactionId};

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

#[tracing::instrument(skip_all, err)]
pub async fn extend_from_tx_id(client: &TonClient, address: &str, from: Option<ton::get_account_transactions_request::From>) -> Result<Option<InternalTransactionId>> {
    Ok(match from {
        None => {
            let state = client.raw_get_account_state(address).await?;
            state.last_transaction_id
        },
        Some(FromBlockId(block_id)) => {
            let block_id = extend_block_id(client, &block_id).await?;
            let state = client.raw_get_account_state_on_block(address, block_id).await?;

            state.last_transaction_id
        },
        Some(FromTransactionId(tx_id)) => {
            let state = client.raw_get_account_state_by_transaction(address, tx_id.into()).await?;

            state.last_transaction_id
        }
    })
}
