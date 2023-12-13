use std::str::FromStr;
use anyhow::anyhow;
use tonlibjson_client::address::{AccountAddressData};
use tonlibjson_client::block;
use tonlibjson_client::block::{MsgBoxedData, MsgDataDecryptedText, MsgDataEncryptedText, MsgDataRaw, MsgDataText};
use crate::ton::get_account_state_response::AccountState;
use crate::ton::message::MsgData;

tonic::include_proto!("ton");

pub(crate) const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("ton_descriptor");

impl From<block::TonBlockIdExt> for BlockIdExt {
    fn from(value: block::TonBlockIdExt) -> Self {
        Self {
            workchain: value.workchain,
            shard: value.shard,
            seqno: value.seqno,
            root_hash: value.root_hash,
            file_hash: value.file_hash,
        }
    }
}

impl From<BlockIdExt> for block::TonBlockIdExt {
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

impl From<(i32, block::BlocksShortTxId)> for TransactionId {
    fn from((chain_id, value): (i32, block::BlocksShortTxId)) -> Self {
        let address = value.clone().into_internal_string(chain_id);

        Self {
            account_address: address,
            lt: value.lt,
            hash: value.hash
        }
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
        if !value.code.is_empty() {
            AccountState::Active(ActiveAccountState {
                code: value.code,
                data: value.data
            })
        } else if !value.frozen_hash.is_empty() {
            AccountState::Frozen(FrozenAccountState {
                frozen_hash: value.frozen_hash
            })
        } else {
            AccountState::Uninitialized(UninitializedAccountState {})
        }
    }
}

impl From<block::TvmCell> for TvmCell {
    fn from(value: block::TvmCell) -> Self {
        Self {
            bytes: value.bytes
        }
    }
}

impl From<MsgBoxedData> for MsgData {
    fn from(value: MsgBoxedData) -> Self {

        match value {
            MsgBoxedData::MsgDataRaw(MsgDataRaw { body, init_state }) => { Self::Raw(MessageDataRaw { body, init_state })}
            MsgBoxedData::MsgDataText(MsgDataText { text }) => { Self::Text(MessageDataText { text })}
            MsgBoxedData::MsgDataDecryptedText(MsgDataDecryptedText { text }) => { Self::DecryptedText(MessageDataDecryptedText { text }) }
            MsgBoxedData::MsgDataEncryptedText(MsgDataEncryptedText { text }) => { Self::EncryptedText(MessageDataEncryptedText { text }) }
        }
    }
}

impl From<block::RawMessage> for Message {
    fn from(value: block::RawMessage) -> Self {
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
            in_msg: value.in_msg.map(|m| m.into()),
            out_msgs: value.out_msgs.into_iter().map(Into::into).collect(),
        }
    }
}

impl TryFrom<(i32, block::RawTransaction)> for Transaction {
    type Error = anyhow::Error;

    fn try_from((chain_id, value): (i32, block::RawTransaction)) -> Result<Self, Self::Error> {
        let address = value.address.account_address
            .as_ref()
            .ok_or(anyhow!("empty address"))
            .and_then(|f| AccountAddressData::from_str(f))?
            .with_chain_id(chain_id);

        Ok(Self {
            id: Some((&address, value.transaction_id).into()),
            utime: value.utime,
            data: value.data.clone(),
            fee: value.fee,
            storage_fee: value.storage_fee,
            other_fee: value.other_fee,
            in_msg: value.in_msg.map(|m| m.into()),
            out_msgs: value.out_msgs.into_iter().map(Into::into).collect(),
        })
    }
}
