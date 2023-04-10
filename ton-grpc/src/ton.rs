use anyhow::anyhow;
use serde::Deserialize;
use tonlibjson_client::block;
use crate::ton::get_account_state_response::AccountState;

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
            root_hash: value.root_hash.clone(),
            file_hash: value.file_hash.clone(),
        }
    }
}

impl From<BlockIdExt> for block::BlockIdExt {
    fn from(value: BlockIdExt) -> Self {
        Self {
            workchain: value.workchain,
            shard: value.shard,
            seqno: value.seqno,
            root_hash: value.root_hash.clone(),
            file_hash: value.file_hash.clone(),
        }
    }
}

impl From<block::InternalTransactionId> for TransactionId {
    fn from(value: block::InternalTransactionId) -> Self {
        Self {
            lt: value.lt,
            hash: value.hash.clone()
        }
    }
}

impl From<TransactionId> for block::InternalTransactionId {
    fn from(value: TransactionId) -> Self {
        Self {
            hash: value.hash.clone(),
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
            bytes: value.bytes.clone()
        }
    }
}
