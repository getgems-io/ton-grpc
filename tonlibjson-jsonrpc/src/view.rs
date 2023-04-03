use serde::Serialize;
use serde_json::Value;
use tonlibjson_client::block::{BlockHeader, BlockIdExt, BlocksShards, InternalTransactionId, MasterchainInfo, RawMessage, RawTransaction};

#[derive(Serialize)]
#[serde(tag = "@type", rename = "blocks.masterchainInfo")]
pub struct MasterchainInfoView {
    pub init: BlockIdExtView,
    pub last: BlockIdExtView,
    pub state_root_hash: String,
}

impl From<MasterchainInfo> for MasterchainInfoView {
    fn from(info: MasterchainInfo) -> Self {
        Self {
            init: info.init.into(),
            last: info.last.into(),
            state_root_hash: info.state_root_hash
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "@type", rename = "ton.blockIdExt")]
pub struct BlockIdExtView {
    pub workchain: i32,
    pub shard: String,
    pub seqno: i32,
    pub root_hash: String,
    pub file_hash: String,
}

impl From<BlockIdExt> for BlockIdExtView {
    fn from(id: BlockIdExt) -> Self {
        Self {
            workchain: id.workchain,
            shard: id.shard.to_string(),
            seqno: id.seqno,
            root_hash: id.root_hash,
            file_hash: id.file_hash
        }
    }
}

impl From<&BlockIdExt> for BlockIdExtView {
    fn from(id: &BlockIdExt) -> Self {
        id.clone().into()
    }
}

#[derive(Serialize)]
#[serde(tag = "@type", rename = "blocks.shards")]
pub struct ShardsView {
    pub shards: Vec<BlockIdExtView>
}

impl From<BlocksShards> for ShardsView {
    fn from(shards: BlocksShards) -> Self {
        Self {
            shards: shards
                .shards
                .into_iter()
                .map(Into::into)
                .collect()
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "@type", rename = "blocks.header")]
pub struct BlockHeaderView {
    pub id: BlockIdExtView,
    pub global_id: i32,
    pub version: i32,
    pub after_merge: bool,
    pub after_split: bool,
    pub before_split: bool,
    pub want_merge: bool,
    pub validator_list_hash_short: i32,
    pub catchain_seqno: i32,
    pub min_ref_mc_seqno: i32,
    pub is_key_block: bool,
    pub prev_key_block_seqno: i32,
    pub start_lt: String,
    pub end_lt: String,
    pub gen_utime: i64,
    pub prev_blocks: Vec<BlockIdExtView>
}

impl From<BlockHeader> for BlockHeaderView {
    fn from(header: BlockHeader) -> Self {
        Self {
            id: header.id.into(),
            global_id: header.global_id,
            version: header.version,
            after_merge: header.after_merge,
            after_split: header.after_split,
            before_split: header.before_split,
            want_merge: header.want_merge,
            validator_list_hash_short: header.validator_list_hash_short,
            catchain_seqno: header.catchain_seqno,
            min_ref_mc_seqno: header.min_ref_mc_seqno,
            is_key_block: header.is_key_block,
            prev_key_block_seqno: header.prev_key_block_seqno,
            start_lt: header.start_lt.to_string(),
            end_lt: header.end_lt.to_string(),
            gen_utime: header.gen_utime,
            prev_blocks: header.prev_blocks.iter().map(Into::into).collect()
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "@type", rename = "raw.message")]
pub struct MessageView {
    pub source: String,
    pub destination: String,
    pub value: String,
    pub fwd_fee: String,
    pub ihr_fee: String,
    pub created_lt: String,
    pub body_hash: String,
    pub msg_data: Value
}

impl From<&RawMessage> for MessageView {
    fn from(msg: &RawMessage) -> Self {
        Self {
            source: msg.source.account_address.as_base64_string(),
            destination: msg.destination.account_address.as_base64_string(),
            value: msg.value.to_string(),
            fwd_fee: msg.fwd_fee.clone(),
            ihr_fee: msg.ihr_fee.clone(),
            created_lt: msg.created_lt.clone(),
            body_hash: msg.body_hash.clone(),
            msg_data: msg.msg_data.clone()
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "@type", rename = "internal.transactionId")]
pub struct TransactionIdView {
    pub hash: String,
    pub lt: String
}

impl From<&InternalTransactionId> for TransactionIdView {
    fn from(id: &InternalTransactionId) -> Self {
        Self {
            hash: id.hash.clone(),
            lt: id.lt.to_string()
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "@type", rename = "raw.transaction")]
pub struct TransactionView {
    pub utime: i64,
    pub data: String,
    pub transaction_id: TransactionIdView,
    pub fee: String,
    pub storage_fee: String,
    pub other_fee: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_msg: Option<MessageView>,
    pub out_msgs: Vec<MessageView>
}

impl From<&RawTransaction> for TransactionView {
    fn from(tx: &RawTransaction) -> Self {
        Self {
            utime: tx.utime,
            data: tx.data.clone(),
            transaction_id: (&tx.transaction_id).into(),
            fee: tx.fee.to_string(),
            storage_fee: tx.storage_fee.to_string(),
            other_fee: tx.other_fee.to_string(),
            in_msg: tx.in_msg.as_ref().map(|msg| msg.into()),
            out_msgs: tx.out_msgs.iter().map(Into::into).collect(),
        }
    }
}
