use crate::tl;
use crate::tl::{
    BlocksTransactions, BlocksTransactionsExt, RawMessage, RawTransaction, RawTransactions,
};
use anyhow::{Context, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as base64;
use std::str::FromStr;
use ton_address::SmartContractAddress;
use ton_tower::response::ShortTxId;

impl From<tl::TonBlockIdExt> for ton_tower::response::BlockIdExt {
    fn from(v: tl::TonBlockIdExt) -> Self {
        Self {
            workchain: v.workchain,
            shard: v.shard,
            seqno: v.seqno,
            root_hash: v.root_hash,
            file_hash: v.file_hash,
        }
    }
}

impl From<ton_tower::response::BlockIdExt> for tl::TonBlockIdExt {
    fn from(v: ton_tower::response::BlockIdExt) -> Self {
        Self {
            workchain: v.workchain,
            shard: v.shard,
            seqno: v.seqno,
            root_hash: v.root_hash,
            file_hash: v.file_hash,
        }
    }
}

impl From<tl::InternalTransactionId> for ton_tower::response::TransactionId {
    fn from(v: tl::InternalTransactionId) -> Self {
        Self {
            lt: v.lt,
            hash: v.hash,
        }
    }
}

impl From<ton_tower::response::TransactionId> for tl::InternalTransactionId {
    fn from(v: ton_tower::response::TransactionId) -> Self {
        Self {
            lt: v.lt,
            hash: v.hash,
        }
    }
}

impl From<tl::BlocksMasterchainInfo> for ton_tower::response::MasterchainInfo {
    fn from(v: tl::BlocksMasterchainInfo) -> Self {
        Self {
            last: v.last.into(),
            state_root_hash: v.state_root_hash,
            init: v.init.into(),
        }
    }
}

impl From<tl::BlocksShards> for ton_tower::response::Shards {
    fn from(v: tl::BlocksShards) -> Self {
        Self {
            shards: v.shards.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<tl::BlocksHeader> for ton_tower::response::BlockHeader {
    fn from(v: tl::BlocksHeader) -> Self {
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

impl From<ton_tower::response::ShortTxId> for tl::BlocksAccountTransactionId {
    fn from(value: ShortTxId) -> Self {
        Self {
            account: base64.encode(value.account.to_internal()),
            lt: value.lt,
        }
    }
}

impl TryFrom<tl::BlocksTransactions> for ton_tower::response::BlockTransactions {
    type Error = anyhow::Error;

    fn try_from(v: BlocksTransactions) -> Result<Self, Self::Error> {
        Ok(Self {
            incomplete: v.incomplete,
            transactions: v
                .transactions
                .into_iter()
                .map(|tx| {
                    Ok(ShortTxId {
                        account: SmartContractAddress::raw(
                            v.id.workchain,
                            base64
                                .decode(&tx.account)
                                .context(format!("invalid base64 address: {}", tx.account))?
                                .try_into()
                                .map_err(|_| anyhow!("invalid address: {}", tx.account))?,
                        ),
                        lt: tx.lt,
                        hash: tx.hash,
                    })
                })
                .collect::<Result<Vec<ShortTxId>, anyhow::Error>>()?,
        })
    }
}

impl TryFrom<tl::BlocksTransactionsExt> for ton_tower::response::BlockTransactionsExt {
    type Error = anyhow::Error;

    fn try_from(v: BlocksTransactionsExt) -> Result<Self, Self::Error> {
        Ok(Self {
            incomplete: v.incomplete,
            transactions: v
                .transactions
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl From<tl::RawFullAccountState> for ton_tower::response::AccountState {
    fn from(v: tl::RawFullAccountState) -> Self {
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

impl TryFrom<tl::RawTransaction> for ton_tower::response::Transaction {
    type Error = anyhow::Error;

    fn try_from(v: RawTransaction) -> Result<Self, Self::Error> {
        let address = v.address.account_address.ok_or_else(|| {
            anyhow::anyhow!(
                "transaction address is not set, transaction id: {:?}",
                v.transaction_id
            )
        })?;

        Ok(Self {
            address: SmartContractAddress::from_str(&address)?,
            utime: v.utime,
            data: v.data,
            transaction_id: v.transaction_id.into(),
            fee: v.fee,
            storage_fee: v.storage_fee,
            other_fee: v.other_fee,
            in_msg: v.in_msg.map(TryInto::try_into).transpose()?,
            out_msgs: v
                .out_msgs
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl TryFrom<tl::RawTransactions> for ton_tower::response::Transactions {
    type Error = anyhow::Error;

    fn try_from(v: RawTransactions) -> Result<Self, Self::Error> {
        let transactions = v
            .transactions
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<ton_tower::response::Transaction>, _>>()?;

        Ok(Self {
            transactions,
            previous_transaction_id: v.previous_transaction_id.map(Into::into),
        })
    }
}

impl TryFrom<tl::RawMessage> for ton_tower::response::Message {
    type Error = anyhow::Error;

    fn try_from(v: RawMessage) -> Result<Self, Self::Error> {
        let source: Option<SmartContractAddress> = v
            .source
            .account_address
            .map(|s| SmartContractAddress::from_str(&s))
            .transpose()?;
        let destination: Option<SmartContractAddress> = v
            .destination
            .account_address
            .map(|s| SmartContractAddress::from_str(&s))
            .transpose()?;
        Ok(Self {
            hash: v.hash,
            source,
            destination,
            value: v.value,
            fwd_fee: v.fwd_fee,
            ihr_fee: v.ihr_fee,
            created_lt: v.created_lt,
            body_hash: v.body_hash,
            msg_data: v.msg_data.into(),
        })
    }
}

impl From<tl::MsgBoxedData> for ton_tower::response::MessageData {
    fn from(v: tl::MsgBoxedData) -> Self {
        match v {
            tl::MsgBoxedData::MsgDataRaw(d) => ton_tower::response::MessageData::Raw {
                body: d.body,
                init_state: d.init_state,
            },
            tl::MsgBoxedData::MsgDataText(d) => {
                ton_tower::response::MessageData::Text { text: d.text }
            }
            tl::MsgBoxedData::MsgDataDecryptedText(d) => {
                ton_tower::response::MessageData::DecryptedText { text: d.text }
            }
            tl::MsgBoxedData::MsgDataEncryptedText(d) => {
                ton_tower::response::MessageData::EncryptedText { text: d.text }
            }
        }
    }
}

impl From<tl::TvmCell> for ton_tower::response::Cell {
    fn from(v: tl::TvmCell) -> Self {
        Self { bytes: v.bytes }
    }
}

impl From<tl::SmcRunResult> for ton_tower::response::SmcRunResult {
    fn from(v: tl::SmcRunResult) -> Self {
        Self {
            gas_used: v.gas_used,
            exit_code: v.exit_code,
            stack: v.stack.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<tl::TvmBoxedStackEntry> for ton_tower::response::StackEntry {
    fn from(v: tl::TvmBoxedStackEntry) -> Self {
        match v {
            tl::TvmBoxedStackEntry::TvmStackEntrySlice(s) => {
                ton_tower::response::StackEntry::Slice {
                    bytes: s.slice.bytes,
                }
            }
            tl::TvmBoxedStackEntry::TvmStackEntryCell(c) => ton_tower::response::StackEntry::Cell {
                bytes: c.cell.bytes,
            },
            tl::TvmBoxedStackEntry::TvmStackEntryNumber(n) => {
                ton_tower::response::StackEntry::Number {
                    number: n.number.number,
                }
            }
            tl::TvmBoxedStackEntry::TvmStackEntryTuple(t) => {
                ton_tower::response::StackEntry::Tuple {
                    elements: t.tuple.elements.into_iter().map(Into::into).collect(),
                }
            }
            tl::TvmBoxedStackEntry::TvmStackEntryList(l) => ton_tower::response::StackEntry::List {
                elements: l.list.elements.into_iter().map(Into::into).collect(),
            },
            tl::TvmBoxedStackEntry::TvmStackEntryUnsupported(_) => {
                ton_tower::response::StackEntry::Unsupported
            }
        }
    }
}

impl From<ton_tower::response::StackEntry> for tl::TvmBoxedStackEntry {
    fn from(v: ton_tower::response::StackEntry) -> Self {
        match v {
            ton_tower::response::StackEntry::Slice { bytes } => {
                tl::TvmBoxedStackEntry::TvmStackEntrySlice(tl::TvmStackEntrySlice {
                    slice: tl::TvmSlice { bytes },
                })
            }
            ton_tower::response::StackEntry::Cell { bytes } => {
                tl::TvmBoxedStackEntry::TvmStackEntryCell(tl::TvmStackEntryCell {
                    cell: tl::TvmCell { bytes },
                })
            }
            ton_tower::response::StackEntry::Number { number } => {
                tl::TvmBoxedStackEntry::TvmStackEntryNumber(tl::TvmStackEntryNumber {
                    number: tl::TvmNumberDecimal { number },
                })
            }
            ton_tower::response::StackEntry::Tuple { elements } => {
                tl::TvmBoxedStackEntry::TvmStackEntryTuple(tl::TvmStackEntryTuple {
                    tuple: tl::TvmTuple {
                        elements: elements.into_iter().map(Into::into).collect(),
                    },
                })
            }
            ton_tower::response::StackEntry::List { elements } => {
                tl::TvmBoxedStackEntry::TvmStackEntryList(tl::TvmStackEntryList {
                    list: tl::TvmList {
                        elements: elements.into_iter().map(Into::into).collect(),
                    },
                })
            }
            ton_tower::response::StackEntry::Unsupported => {
                tl::TvmBoxedStackEntry::TvmStackEntryUnsupported(tl::TvmStackEntryUnsupported {})
            }
        }
    }
}

impl From<tl::RawExtMessageInfo> for ton_tower::response::ExtMessageInfo {
    fn from(v: tl::RawExtMessageInfo) -> Self {
        Self { hash: v.hash }
    }
}
