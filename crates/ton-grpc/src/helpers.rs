use crate::ton;
use crate::ton::get_account_transactions_request::bound::Bound::{BlockId, TransactionId};
use crate::ton::get_account_transactions_request::bound::Type;
use anyhow::{Result, anyhow};
use std::ops::Bound;
use std::ops::Bound::{Excluded, Included};
use ton_client::client::TonClient;
use ton_client::types::{
    BlockIdExt as TonClientBlockIdExt, BlocksHeader as TonClientBlocksHeader,
    InternalTransactionId as TonClientInternalTransactionId,
};

#[tracing::instrument(skip_all, err)]
pub async fn extend_block_id(
    client: &impl TonClient,
    block_id: &ton::BlockId,
) -> Result<TonClientBlockIdExt> {
    if let (Some(root_hash), Some(file_hash)) = (&block_id.root_hash, &block_id.file_hash) {
        Ok(TonClientBlockIdExt::new(
            block_id.workchain,
            block_id.shard,
            block_id.seqno,
            root_hash.clone(),
            file_hash.clone(),
        ))
    } else {
        client
            .look_up_block_by_seqno(block_id.workchain, block_id.shard, block_id.seqno)
            .await
            .map_err(|e| anyhow!("{e}"))
    }
}

#[tracing::instrument(skip_all, err)]
pub async fn extend_get_block_header(
    client: &impl TonClient,
    block_id: &ton::BlockId,
) -> Result<TonClientBlocksHeader> {
    client
        .get_block_header(
            block_id.workchain,
            block_id.shard,
            block_id.seqno,
            block_id.root_hash.clone().zip(block_id.file_hash.clone()),
        )
        .await
        .map_err(|e| anyhow!("{e}"))
}

#[tracing::instrument(skip_all, err)]
pub async fn prev_block_id(
    client: &impl TonClient,
    block_id: &ton::BlockId,
) -> Result<TonClientBlockIdExt> {
    client
        .look_up_block_by_seqno(block_id.workchain, block_id.shard, block_id.seqno - 1)
        .await
        .map_err(|e| anyhow!("{e}"))
}

#[tracing::instrument(skip_all, err)]
pub async fn extend_from_tx_id(
    client: &impl TonClient,
    address: &str,
    from: Option<ton::get_account_transactions_request::Bound>,
) -> Result<Bound<TonClientInternalTransactionId>> {
    Ok(match from {
        None => Bound::Unbounded,
        Some(b) => {
            let typ = b.r#type();
            match b.bound {
                None => Bound::Unbounded,
                Some(BlockId(block_id)) => match typ {
                    Type::Included => {
                        let block_id = extend_block_id(client, &block_id).await?;
                        let state = client
                            .raw_get_account_state_on_block(address, block_id)
                            .await
                            .map_err(|e| anyhow!("{e}"))?;

                        Included(
                            state
                                .last_transaction_id
                                .ok_or(anyhow!("to_tx not found"))?,
                        )
                    }
                    Type::Excluded => {
                        let block_id = prev_block_id(client, &block_id).await?;
                        let state = client
                            .raw_get_account_state_on_block(address, block_id)
                            .await
                            .map_err(|e| anyhow!("{e}"))?;

                        Included(
                            state
                                .last_transaction_id
                                .ok_or(anyhow!("to_tx not found"))?,
                        )
                    }
                },
                Some(TransactionId(tx_id)) => match typ {
                    Type::Included => Included(tx_id.into()),
                    Type::Excluded => Excluded(tx_id.into()),
                },
            }
        }
    })
}

#[tracing::instrument(skip_all, err)]
pub async fn extend_to_tx_id(
    client: &impl TonClient,
    address: &str,
    to: Option<ton::get_account_transactions_request::Bound>,
) -> Result<Bound<TonClientInternalTransactionId>> {
    Ok(match to {
        None => Bound::Unbounded,
        Some(b) => {
            let typ = b.r#type();
            match b.bound {
                None => Bound::Unbounded,
                Some(BlockId(block_id)) => match typ {
                    Type::Included => {
                        let block_id = prev_block_id(client, &block_id).await?;
                        let state = client
                            .raw_get_account_state_on_block(address, block_id)
                            .await
                            .map_err(|e| anyhow!("{e}"))?;

                        Excluded(
                            state
                                .last_transaction_id
                                .ok_or(anyhow!("to_tx not found"))?,
                        )
                    }
                    Type::Excluded => {
                        let block_id = extend_block_id(client, &block_id).await?;
                        let state = client
                            .raw_get_account_state_on_block(address, block_id)
                            .await
                            .map_err(|e| anyhow!("{e}"))?;

                        Excluded(
                            state
                                .last_transaction_id
                                .ok_or(anyhow!("to_tx not found"))?,
                        )
                    }
                },
                Some(TransactionId(tx_id)) => match typ {
                    Type::Included => Included(tx_id.into()),
                    Type::Excluded => Excluded(tx_id.into()),
                },
            }
        }
    })
}
