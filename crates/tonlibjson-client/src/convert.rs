use crate::address::AccountAddressData;
use crate::block;
use std::str::FromStr;

impl From<block::TonBlockIdExt> for ton_client::BlockIdExt {
    fn from(v: block::TonBlockIdExt) -> Self {
        Self {
            workchain: v.workchain,
            shard: v.shard,
            seqno: v.seqno,
            root_hash: v.root_hash,
            file_hash: v.file_hash,
        }
    }
}

impl From<ton_client::BlockIdExt> for block::TonBlockIdExt {
    fn from(v: ton_client::BlockIdExt) -> Self {
        Self {
            workchain: v.workchain,
            shard: v.shard,
            seqno: v.seqno,
            root_hash: v.root_hash,
            file_hash: v.file_hash,
        }
    }
}

impl From<block::InternalTransactionId> for ton_client::TransactionId {
    fn from(v: block::InternalTransactionId) -> Self {
        Self {
            lt: v.lt,
            hash: v.hash,
        }
    }
}

impl From<ton_client::TransactionId> for block::InternalTransactionId {
    fn from(v: ton_client::TransactionId) -> Self {
        Self {
            lt: v.lt,
            hash: v.hash,
        }
    }
}

impl From<block::BlocksMasterchainInfo> for ton_client::MasterchainInfo {
    fn from(v: block::BlocksMasterchainInfo) -> Self {
        Self {
            last: v.last.into(),
            state_root_hash: v.state_root_hash,
            init: v.init.into(),
        }
    }
}

impl From<block::BlocksShards> for ton_client::Shards {
    fn from(v: block::BlocksShards) -> Self {
        Self {
            shards: v.shards.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<block::BlocksHeader> for ton_client::BlockHeader {
    fn from(v: block::BlocksHeader) -> Self {
        Self {
            id: v.id.into(),
            global_id: v.global_id,
            version: v.version,
            flags: v.flags,
            after_merge: v.after_merge,
            after_split: v.after_split,
            before_split: v.before_split,
            want_merge: v.want_merge,
            want_split: v.want_split,
            validator_list_hash_short: v.validator_list_hash_short,
            catchain_seqno: v.catchain_seqno,
            min_ref_mc_seqno: v.min_ref_mc_seqno,
            is_key_block: v.is_key_block,
            prev_key_block_seqno: v.prev_key_block_seqno,
            start_lt: v.start_lt,
            end_lt: v.end_lt,
            gen_utime: v.gen_utime,
            vert_seqno: v.vert_seqno,
            prev_blocks: v.prev_blocks.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ton_client::ShortTxId> for block::BlocksAccountTransactionId {
    fn from(v: ton_client::ShortTxId) -> Self {
        Self {
            account: v.account,
            lt: v.lt,
        }
    }
}

impl From<block::BlocksTransactions> for ton_client::BlockTransactions {
    fn from(v: block::BlocksTransactions) -> Self {
        Self {
            incomplete: v.incomplete,
            transactions: v.transactions.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<block::BlocksTransactionsExt> for ton_client::BlockTransactionsExt {
    fn from(v: block::BlocksTransactionsExt) -> Self {
        Self {
            incomplete: v.incomplete,
            transactions: v.transactions.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<block::BlocksShortTxId> for ton_client::ShortTxId {
    fn from(v: block::BlocksShortTxId) -> Self {
        Self {
            mode: v.mode,
            account: v.account,
            lt: v.lt,
            hash: v.hash,
        }
    }
}

impl From<ton_client::ShortTxId> for block::BlocksShortTxId {
    fn from(v: ton_client::ShortTxId) -> Self {
        Self {
            mode: v.mode,
            account: v.account,
            lt: v.lt,
            hash: v.hash,
        }
    }
}

impl From<block::RawFullAccountState> for ton_client::AccountState {
    fn from(v: block::RawFullAccountState) -> Self {
        Self {
            balance: v.balance,
            code: v.code,
            data: v.data,
            frozen_hash: v.frozen_hash,
            last_transaction_id: v.last_transaction_id.map(Into::into),
            block_id: v.block_id.into(),
            sync_utime: v.sync_utime,
        }
    }
}

impl From<block::RawTransaction> for ton_client::Transaction {
    fn from(v: block::RawTransaction) -> Self {
        Self {
            address: v
                .address
                .account_address
                .as_deref()
                .and_then(|a| AccountAddressData::from_str(a).ok())
                .map(|a| a.to_raw_string())
                .unwrap_or_default(),
            utime: v.utime,
            data: v.data,
            transaction_id: v.transaction_id.into(),
            fee: v.fee,
            storage_fee: v.storage_fee,
            other_fee: v.other_fee,
            in_msg: v.in_msg.map(Into::into),
            out_msgs: v.out_msgs.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<block::RawTransactions> for ton_client::Transactions {
    fn from(v: block::RawTransactions) -> Self {
        Self {
            transactions: v.transactions.into_iter().map(Into::into).collect(),
            previous_transaction_id: v.previous_transaction_id.map(Into::into),
        }
    }
}

impl From<block::RawMessage> for ton_client::Message {
    fn from(v: block::RawMessage) -> Self {
        Self {
            hash: v.hash,
            source: v
                .source
                .account_address
                .as_deref()
                .and_then(|a| AccountAddressData::from_str(a).ok())
                .map(|a| a.to_raw_string())
                .unwrap_or_default(),
            destination: v
                .destination
                .account_address
                .as_deref()
                .and_then(|a| AccountAddressData::from_str(a).ok())
                .map(|a| a.to_raw_string())
                .unwrap_or_default(),
            value: v.value,
            fwd_fee: v.fwd_fee,
            ihr_fee: v.ihr_fee,
            created_lt: v.created_lt,
            body_hash: v.body_hash,
            msg_data: v.msg_data.into(),
        }
    }
}

impl From<block::MsgBoxedData> for ton_client::MessageData {
    fn from(v: block::MsgBoxedData) -> Self {
        match v {
            block::MsgBoxedData::MsgDataRaw(d) => ton_client::MessageData::Raw {
                body: d.body,
                init_state: d.init_state,
            },
            block::MsgBoxedData::MsgDataText(d) => ton_client::MessageData::Text { text: d.text },
            block::MsgBoxedData::MsgDataDecryptedText(d) => {
                ton_client::MessageData::DecryptedText { text: d.text }
            }
            block::MsgBoxedData::MsgDataEncryptedText(d) => {
                ton_client::MessageData::EncryptedText { text: d.text }
            }
        }
    }
}

impl From<block::TvmCell> for ton_client::Cell {
    fn from(v: block::TvmCell) -> Self {
        Self { bytes: v.bytes }
    }
}

impl From<block::SmcRunResult> for ton_client::SmcRunResult {
    fn from(v: block::SmcRunResult) -> Self {
        Self {
            gas_used: v.gas_used,
            exit_code: v.exit_code,
            stack: v.stack.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<block::TvmBoxedStackEntry> for ton_client::StackEntry {
    fn from(v: block::TvmBoxedStackEntry) -> Self {
        match v {
            block::TvmBoxedStackEntry::TvmStackEntrySlice(s) => ton_client::StackEntry::Slice {
                bytes: s.slice.bytes,
            },
            block::TvmBoxedStackEntry::TvmStackEntryCell(c) => ton_client::StackEntry::Cell {
                bytes: c.cell.bytes,
            },
            block::TvmBoxedStackEntry::TvmStackEntryNumber(n) => ton_client::StackEntry::Number {
                number: n.number.number,
            },
            block::TvmBoxedStackEntry::TvmStackEntryTuple(t) => ton_client::StackEntry::Tuple {
                elements: t.tuple.elements.into_iter().map(Into::into).collect(),
            },
            block::TvmBoxedStackEntry::TvmStackEntryList(l) => ton_client::StackEntry::List {
                elements: l.list.elements.into_iter().map(Into::into).collect(),
            },
            block::TvmBoxedStackEntry::TvmStackEntryUnsupported(_) => {
                ton_client::StackEntry::Unsupported
            }
        }
    }
}

impl From<ton_client::StackEntry> for block::TvmBoxedStackEntry {
    fn from(v: ton_client::StackEntry) -> Self {
        match v {
            ton_client::StackEntry::Slice { bytes } => {
                block::TvmBoxedStackEntry::TvmStackEntrySlice(block::TvmStackEntrySlice {
                    slice: block::TvmSlice { bytes },
                })
            }
            ton_client::StackEntry::Cell { bytes } => {
                block::TvmBoxedStackEntry::TvmStackEntryCell(block::TvmStackEntryCell {
                    cell: block::TvmCell { bytes },
                })
            }
            ton_client::StackEntry::Number { number } => {
                block::TvmBoxedStackEntry::TvmStackEntryNumber(block::TvmStackEntryNumber {
                    number: block::TvmNumberDecimal { number },
                })
            }
            ton_client::StackEntry::Tuple { elements } => {
                block::TvmBoxedStackEntry::TvmStackEntryTuple(block::TvmStackEntryTuple {
                    tuple: block::TvmTuple {
                        elements: elements.into_iter().map(Into::into).collect(),
                    },
                })
            }
            ton_client::StackEntry::List { elements } => {
                block::TvmBoxedStackEntry::TvmStackEntryList(block::TvmStackEntryList {
                    list: block::TvmList {
                        elements: elements.into_iter().map(Into::into).collect(),
                    },
                })
            }
            ton_client::StackEntry::Unsupported => {
                block::TvmBoxedStackEntry::TvmStackEntryUnsupported(
                    block::TvmStackEntryUnsupported {},
                )
            }
        }
    }
}

impl From<block::RawExtMessageInfo> for ton_client::ExtMessageInfo {
    fn from(v: block::RawExtMessageInfo) -> Self {
        Self { hash: v.hash }
    }
}
