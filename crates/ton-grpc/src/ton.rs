use crate::ton::get_account_state_response::AccountState;
use crate::ton::message::MsgData;

tonic::include_proto!("ton");

pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("ton_descriptor");

impl From<ton_client::BlockIdExt> for BlockIdExt {
    fn from(value: ton_client::BlockIdExt) -> Self {
        Self {
            workchain: value.workchain,
            shard: value.shard,
            seqno: value.seqno,
            root_hash: value.root_hash,
            file_hash: value.file_hash,
        }
    }
}

impl From<BlockIdExt> for ton_client::BlockIdExt {
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

impl From<ton_client::ShortTxId> for TransactionId {
    fn from(value: ton_client::ShortTxId) -> Self {
        Self {
            account_address: value.account.to_raw().to_string(),
            lt: value.lt,
            hash: value.hash,
        }
    }
}

impl From<ton_client::BlockHeader> for BlocksHeader {
    fn from(value: ton_client::BlockHeader) -> Self {
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
            prev_blocks: value.prev_blocks.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<TransactionId> for ton_client::TransactionId {
    fn from(value: TransactionId) -> Self {
        Self {
            hash: value.hash,
            lt: value.lt,
        }
    }
}

impl From<PartialTransactionId> for ton_client::TransactionId {
    fn from(value: PartialTransactionId) -> Self {
        Self {
            hash: value.hash,
            lt: value.lt,
        }
    }
}

impl From<ton_client::AccountState> for AccountState {
    fn from(value: ton_client::AccountState) -> Self {
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

impl From<ton_client::Cell> for TvmCell {
    fn from(value: ton_client::Cell) -> Self {
        Self { bytes: value.bytes }
    }
}

impl From<ton_client::MessageData> for MsgData {
    fn from(value: ton_client::MessageData) -> Self {
        match value {
            ton_client::MessageData::Raw { body, init_state } => {
                Self::Raw(MessageDataRaw { body, init_state })
            }
            ton_client::MessageData::Text { text } => Self::Text(MessageDataText { text }),
            ton_client::MessageData::DecryptedText { text } => {
                Self::DecryptedText(MessageDataDecryptedText { text })
            }
            ton_client::MessageData::EncryptedText { text } => {
                Self::EncryptedText(MessageDataEncryptedText { text })
            }
        }
    }
}

impl From<ton_client::Message> for Message {
    fn from(value: ton_client::Message) -> Self {
        Self {
            source: value.source.map(|a| a.to_raw().to_string()),
            destination: value.destination.map(|a| a.to_raw().to_string()),
            value: value.value,
            fwd_fee: value.fwd_fee,
            ihr_fee: value.ihr_fee,
            created_lt: value.created_lt,
            body_hash: value.body_hash,
            msg_data: Some(value.msg_data.into()),
        }
    }
}

impl From<ton_client::Transaction> for Transaction {
    fn from(value: ton_client::Transaction) -> Self {
        Self {
            id: Some(TransactionId {
                account_address: value.address.to_raw().to_string(),
                lt: value.transaction_id.lt,
                hash: value.transaction_id.hash,
            }),
            utime: value.utime,
            data: value.data,
            fee: value.fee,
            storage_fee: value.storage_fee,
            other_fee: value.other_fee,
            in_msg: value.in_msg.map(Into::into),
            out_msgs: value.out_msgs.into_iter().map(Into::into).collect(),
        }
    }
}
