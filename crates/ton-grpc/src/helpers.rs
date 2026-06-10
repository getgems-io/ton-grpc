use crate::ton;
use crate::ton::get_account_transactions_request::bound::Bound::{BlockId, TransactionId};
use crate::ton::get_account_transactions_request::bound::Type;
use anyhow::{Result, anyhow};
use std::ops::Bound;
use std::ops::Bound::{Excluded, Included};
use ton_address::SmartContractAddress;
use ton_client::{Client, TonService};

#[tracing::instrument(skip_all, err)]
pub async fn extend_block_id(
    client: &mut Client<impl TonService>,
    block_id: &ton::BlockId,
) -> Result<ton_tower::response::BlockIdExt> {
    if let (Some(root_hash), Some(file_hash)) = (&block_id.root_hash, &block_id.file_hash) {
        Ok(ton_tower::response::BlockIdExt {
            workchain: block_id.workchain,
            shard: block_id.shard,
            seqno: block_id.seqno,
            root_hash: root_hash.clone(),
            file_hash: file_hash.clone(),
        })
    } else {
        client
            .look_up_block_by_seqno(block_id.workchain, block_id.shard, block_id.seqno)
            .await
    }
}

#[tracing::instrument(skip_all, err)]
pub async fn extend_get_block_header(
    client: &mut Client<impl TonService>,
    block_id: &ton::BlockId,
) -> Result<ton_tower::response::BlockHeader> {
    let block_id = extend_block_id(client, block_id).await?;
    client.get_block_header(block_id).await
}

#[tracing::instrument(skip_all, err)]
pub async fn prev_block_id(
    client: &mut Client<impl TonService>,
    block_id: &ton::BlockId,
) -> Result<ton_tower::response::BlockIdExt> {
    client
        .look_up_block_by_seqno(block_id.workchain, block_id.shard, block_id.seqno - 1)
        .await
}

#[tracing::instrument(skip_all, err)]
pub async fn extend_from_tx_id(
    client: &mut Client<impl TonService>,
    address: &SmartContractAddress,
    from: Option<ton::get_account_transactions_request::Bound>,
) -> Result<Bound<ton_tower::response::TransactionId>> {
    Ok(match from {
        None => Bound::Unbounded,
        Some(b) => {
            let typ = b.r#type();
            match b.bound {
                None => Bound::Unbounded,
                Some(BlockId(block_id)) => match typ {
                    Type::Included => {
                        let block_id = extend_block_id(client, &block_id).await?;
                        let state = client.get_account_state_on_block(address, block_id).await?;

                        Included(
                            state
                                .last_transaction_id
                                .ok_or(anyhow!("to_tx not found"))?,
                        )
                    }
                    Type::Excluded => {
                        let block_id = prev_block_id(client, &block_id).await?;
                        let state = client.get_account_state_on_block(address, block_id).await?;

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
    client: &mut Client<impl TonService>,
    address: &SmartContractAddress,
    to: Option<ton::get_account_transactions_request::Bound>,
) -> Result<Bound<ton_tower::response::TransactionId>> {
    Ok(match to {
        None => Bound::Unbounded,
        Some(b) => {
            let typ = b.r#type();
            match b.bound {
                None => Bound::Unbounded,
                Some(BlockId(block_id)) => match typ {
                    Type::Included => {
                        let block_id = prev_block_id(client, &block_id).await?;
                        let state = client.get_account_state_on_block(address, block_id).await?;

                        Excluded(
                            state
                                .last_transaction_id
                                .ok_or(anyhow!("to_tx not found"))?,
                        )
                    }
                    Type::Excluded => {
                        let block_id = extend_block_id(client, &block_id).await?;
                        let state = client.get_account_state_on_block(address, block_id).await?;

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
