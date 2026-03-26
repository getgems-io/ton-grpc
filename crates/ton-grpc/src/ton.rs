use crate::ton::get_account_state_response::AccountState;
use crate::ton::message::MsgData;
use anyhow::anyhow;
use std::str::FromStr;
use ton_client::types::{
    BlockIdExt as TonClientBlockIdExt, BlocksHeader as TonClientBlocksHeader,
    InternalTransactionId as TonClientInternalTransactionId, MsgData as TonClientMsgData,
    RawFullAccountState as TonClientRawFullAccountState, RawMessage as TonClientRawMessage,
    RawTransaction as TonClientRawTransaction, ShortTxId as TonClientShortTxId,
    TvmCell as TonClientTvmCell,
};
use tonlibjson_client::address::AccountAddressData;

tonic::include_proto!("ton");

pub(crate) const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("ton_descriptor");

impl From<TonClientBlockIdExt> for BlockIdExt {
    fn from(value: TonClientBlockIdExt) -> Self {
        Self {
            workchain: value.workchain,
            shard: value.shard,
            seqno: value.seqno,
            root_hash: value.root_hash,
            file_hash: value.file_hash,
        }
    }
}

impl From<BlockIdExt> for TonClientBlockIdExt {
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

impl From<(i32, TonClientShortTxId)> for TransactionId {
    fn from((chain_id, value): (i32, TonClientShortTxId)) -> Self {
        let address =
            tonlibjson_client::address::ShardContextAccountAddress::from_str(&value.account)
                .expect("invalid shard context account address")
                .into_internal(chain_id)
                .to_string();

        Self {
            account_address: address,
            lt: value.lt,
            hash: value.hash,
        }
    }
}

impl From<TonClientBlocksHeader> for BlocksHeader {
    fn from(value: TonClientBlocksHeader) -> Self {
        Self {
            id: Some(value.id.into()),
            global_id: value.global_id,
            version: value.version,
            flags: value.flags,
            after_merge: value.after_merge,
            after_split: value.after_split,
            before_split: value.before_split,
            want_merge: value.want_merge,
            want_split: value.want_split,
            validator_list_hash_short: value.validator_list_hash_short,
            catchain_seqno: value.catchain_seqno,
            min_ref_mc_seqno: value.min_ref_mc_seqno,
            is_key_block: value.is_key_block,
            prev_key_block_seqno: value.prev_key_block_seqno,
            start_lt: value.start_lt,
            end_lt: value.end_lt,
            gen_utime: value.gen_utime,
            vert_seqno: value.vert_seqno,
            prev_blocks: value.prev_blocks.iter().cloned().map(Into::into).collect(),
        }
    }
}

impl From<(&AccountAddressData, TonClientInternalTransactionId)> for TransactionId {
    fn from(
        (account_address, tx_id): (&AccountAddressData, TonClientInternalTransactionId),
    ) -> Self {
        Self {
            account_address: account_address.to_raw_string(),
            lt: tx_id.lt,
            hash: tx_id.hash,
        }
    }
}

impl From<TransactionId> for TonClientInternalTransactionId {
    fn from(value: TransactionId) -> Self {
        Self {
            hash: value.hash,
            lt: value.lt,
        }
    }
}

impl From<PartialTransactionId> for TonClientInternalTransactionId {
    fn from(value: PartialTransactionId) -> Self {
        Self {
            hash: value.hash,
            lt: value.lt,
        }
    }
}

impl From<TonClientRawFullAccountState> for AccountState {
    fn from(value: TonClientRawFullAccountState) -> Self {
        if !value.code.is_empty() {
            AccountState::Active(ActiveAccountState {
                code: value.code,
                data: value.data,
            })
        } else if !value.frozen_hash.is_empty() {
            AccountState::Frozen(FrozenAccountState {
                frozen_hash: value.frozen_hash,
            })
        } else {
            AccountState::Uninitialized(UninitializedAccountState {})
        }
    }
}

impl From<TonClientTvmCell> for TvmCell {
    fn from(value: TonClientTvmCell) -> Self {
        Self { bytes: value.bytes }
    }
}

impl From<TonClientMsgData> for MsgData {
    fn from(value: TonClientMsgData) -> Self {
        match value {
            TonClientMsgData::Raw { body, init_state } => {
                Self::Raw(MessageDataRaw { body, init_state })
            }
            TonClientMsgData::Text { text } => Self::Text(MessageDataText { text }),
            TonClientMsgData::DecryptedText { text } => {
                Self::DecryptedText(MessageDataDecryptedText { text })
            }
            TonClientMsgData::EncryptedText { text } => {
                Self::EncryptedText(MessageDataEncryptedText { text })
            }
        }
    }
}

impl From<TonClientRawMessage> for Message {
    fn from(value: TonClientRawMessage) -> Self {
        Self {
            source: value.source.account_address,
            destination: value.destination.account_address,
            value: value.value,
            fwd_fee: value.fwd_fee,
            ihr_fee: value.ihr_fee,
            created_lt: value.created_lt,
            body_hash: value.body_hash.clone(),
            msg_data: Some(value.msg_data.into()),
        }
    }
}

impl From<(&AccountAddressData, TonClientRawTransaction)> for Transaction {
    fn from((address, value): (&AccountAddressData, TonClientRawTransaction)) -> Self {
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

impl TryFrom<(i32, TonClientRawTransaction)> for Transaction {
    type Error = anyhow::Error;

    fn try_from((chain_id, value): (i32, TonClientRawTransaction)) -> Result<Self, Self::Error> {
        let address = value
            .address
            .account_address
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
