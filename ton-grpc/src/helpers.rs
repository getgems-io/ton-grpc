use std::ops::Bound;
use std::ops::Bound::Included;
use anyhow::{anyhow, Result};
use tonlibjson_client::block;
use tonlibjson_client::block::InternalTransactionId;
use tonlibjson_client::ton::TonClient;
use crate::ton;
use crate::ton::get_account_transactions_request::From::{FromTransactionId, FromBlockId};
use crate::ton::get_account_transactions_request::To::{ToTransactionId, ToBlockId};

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
pub async fn prev_block_id(client: &TonClient, block_id: &ton::BlockId) -> Result<block::BlockIdExt> {
    client.look_up_block_by_seqno(block_id.workchain, block_id.shard, block_id.seqno - 1).await
}

#[tracing::instrument(skip_all, err)]
pub async fn extend_from_tx_id(client: &TonClient, address: &str, from: Option<ton::get_account_transactions_request::From>) -> Result<Bound<InternalTransactionId>> {
    Ok(match from {
        None => Bound::Unbounded,
        Some(FromBlockId(block_id)) => {
            let block_id = extend_block_id(client, &block_id).await?;
            let state = client.raw_get_account_state_on_block(address, block_id).await?;

            Included(state.last_transaction_id.ok_or(anyhow!("to_tx not found"))?)
        },
        Some(FromTransactionId(tx_id)) => {
            let state = client.raw_get_account_state_by_transaction(address, tx_id.into()).await?;

            Included(state.last_transaction_id.ok_or(anyhow!("to_tx not found"))?)
        }
    })
}

#[tracing::instrument(skip_all, err)]
pub async fn extend_to_tx_id(client: &TonClient, address: &str, to: Option<ton::get_account_transactions_request::To>) -> Result<Bound<InternalTransactionId>> {
    Ok(match to {
        None => Bound::Unbounded,
        Some(ToBlockId(block_id)) => {
            let prev_block_id = prev_block_id(client, &block_id).await?;
            let state = client.raw_get_account_state_on_block(address, prev_block_id).await?;

            Bound::Excluded(state.last_transaction_id.ok_or(anyhow!("to_tx not found"))?)
        },
        Some(ToTransactionId(tx_id)) => {
            // check tx exists
            let _state = client.raw_get_account_state_by_transaction(address, tx_id.clone().into()).await?;

            Bound::Included(tx_id.into())
        }
    })
}
