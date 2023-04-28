use anyhow::anyhow;
use serde::Deserialize;
use tonlibjson_client::address::AccountAddressData;
use tonlibjson_client::block;
use tonlibjson_client::block::{RawMessage, ShortTxId};
use crate::ton::get_account_state_response::AccountState;
use crate::ton::message::MsgData;

tonic::include_proto!("ton");

pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("ton_descriptor");

#[derive(Deserialize)]
pub struct TvmResult<T> {
    pub success: bool,
    pub error: Option<String>,
    #[serde(flatten)]
    pub data: Option<T>
}

impl<T> From<TvmResult<T>> for anyhow::Result<T> where T: Default {
    fn from(value: TvmResult<T>) -> Self {
        if value.success {
            Ok(value.data.unwrap_or_default())
        } else {
            Err(anyhow!(value.error.unwrap_or("ambiguous response".to_owned())))
        }
    }
}

impl From<block::BlockIdExt> for BlockIdExt {
    fn from(value: block::BlockIdExt) -> Self {
        Self {
            workchain: value.workchain,
            shard: value.shard,
            seqno: value.seqno,
            root_hash: value.root_hash,
            file_hash: value.file_hash,
        }
    }
}

impl From<BlockIdExt> for block::BlockIdExt {
    fn from(value: BlockIdExt) -> Self {
        Self {
            workchain: value.workchain,
            shard: value.shard,
            seqno: value.seqno,
            root_hash: value.root_hash,
            file_hash: value.file_hash,
        }
    }
}

impl TryFrom<(i32, block::ShortTxId)> for TransactionId {
    type Error = anyhow::Error;

    fn try_from((chain_id, value): (i32, ShortTxId)) -> Result<Self, Self::Error> {
        let address = format!("{}:{}", chain_id, value.get_account_address()?);

        Ok(Self {
            account_address: address,
            lt: value.lt,
            hash: value.hash
        })
    }
}

impl From<(&AccountAddressData, block::InternalTransactionId)> for TransactionId {
    fn from((account_address, tx_id): (&AccountAddressData, block::InternalTransactionId)) -> Self {
        Self {
            account_address: account_address.to_raw_string(),
            lt: tx_id.lt,
            hash: tx_id.hash
        }
    }
}

impl From<TransactionId> for block::InternalTransactionId {
    fn from(value: TransactionId) -> Self {
        Self {
            hash: value.hash,
            lt: value.lt
        }
    }
}

impl From<PartialTransactionId> for block::InternalTransactionId {
    fn from(value: PartialTransactionId) -> Self {
        Self {
            hash: value.hash,
            lt: value.lt
        }
    }
}

impl From<block::RawFullAccountState> for AccountState {
    fn from(value: block::RawFullAccountState) -> Self {
        if value.code.is_some() {
            AccountState::Active(ActiveAccountState {
                code: value.code.unwrap_or_default(),
                data: value.data.unwrap_or_default()
            })
        } else if value.frozen_hash.is_some() {
            AccountState::Frozen(FrozenAccountState {
                frozen_hash: value.frozen_hash.unwrap_or_default()
            })
        } else {
            AccountState::Uninitialized(UninitializedAccountState {})
        }
    }
}

impl From<block::Cell> for TvmCell {
    fn from(value: block::Cell) -> Self {
        Self {
            bytes: value.bytes
        }
    }
}

impl From<block::MessageData> for MsgData {
    fn from(value: block::MessageData) -> Self {
        match value {
            block::MessageData::Raw { body, init_state } => { Self::Raw(MessageDataRaw { body, init_state }) }
            block::MessageData::Text { text } => { Self::Text(MessageDataText { text }) }
            block::MessageData::DecryptedText { text } => { Self::DecryptedText(MessageDataDecryptedText { text }) }
            block::MessageData::EncryptedText { text } => { Self::EncryptedText(MessageDataEncryptedText { text }) }
        }
    }
}

impl From<block::RawMessage> for Message {
    fn from(value: RawMessage) -> Self {
        Self {
            source: value.source.account_address.map(|s| s.to_string()),
            destination: value.destination.account_address.map(|s| s.to_string()),
            value: value.value,
            fwd_fee: value.fwd_fee,
            ihr_fee: value.ihr_fee,
            created_lt: value.created_lt,
            body_hash: value.body_hash.clone(),
            msg_data: Some(value.msg_data.into()),
        }
    }
}

impl From<(&AccountAddressData, block::RawTransaction)> for Transaction {
    fn from((address, value): (&AccountAddressData, block::RawTransaction)) -> Self {
        Self {
            id: Some((address, value.transaction_id).into()),
            utime: value.utime,
            data: value.data.clone(),
            fee: value.fee,
            storage_fee: value.storage_fee,
            other_fee: value.other_fee,
            in_msg: Some(value.in_msg.into()),
            out_msgs: value.out_msgs.into_iter().map(Into::into).collect(),
        }
    }
}
