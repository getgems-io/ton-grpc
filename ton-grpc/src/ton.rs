use anyhow::anyhow;
use serde::Deserialize;
use tonlibjson_client::block;
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

impl TryFrom<AccountAddress> for block::AccountAddress {
    type Error = anyhow::Error;

    fn try_from(value: AccountAddress) -> Result<Self, Self::Error> {
        Self::new(&value.address)
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

impl From<block::InternalTransactionId> for TransactionId {
    fn from(value: block::InternalTransactionId) -> Self {
        Self {
            lt: value.lt,
            hash: value.hash
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

impl From<block::AccountAddress> for AccountAddress {
    fn from(value: block::AccountAddress) -> Self {
        Self {
            address: value.account_address,
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
    fn from(value: block::RawMessage) -> Self {
        Self {
            source: Some(value.source.into()),
            destination: Some(value.destination.into()),
            value: value.value,
            fwd_fee: value.fwd_fee,
            ihr_fee: value.ihr_fee,
            created_lt: value.created_lt,
            body_hash: value.body_hash.clone(),
            msg_data: Some(value.msg_data.into()),
        }
    }
}

impl From<&block::RawMessage> for Message {
    fn from(value: &block::RawMessage) -> Self {
        value.to_owned().into()
    }
}

impl From<block::RawTransaction> for Transaction {
    fn from(value: block::RawTransaction) -> Self {
        Self {
            id: Some(value.transaction_id.into()),
            utime: value.utime,
            data: value.data.clone(),
            fee: value.fee,
            storage_fee: value.storage_fee,
            other_fee: value.other_fee,
            in_msg: value.in_msg.map(Into::into),
            out_msgs: value.out_msgs.into_iter().map(Into::into).collect(),
        }
    }
}
