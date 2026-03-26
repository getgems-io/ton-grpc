#[derive(Debug, Clone, PartialEq)]
pub struct BlockIdExt {
    pub workchain: i32,
    pub shard: i64,
    pub seqno: i32,
    pub root_hash: String,
    pub file_hash: String,
}

impl BlockIdExt {
    pub fn new(
        workchain: i32,
        shard: i64,
        seqno: i32,
        root_hash: String,
        file_hash: String,
    ) -> Self {
        Self {
            workchain,
            shard,
            seqno,
            root_hash,
            file_hash,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InternalTransactionId {
    pub hash: String,
    pub lt: i64,
}

impl Default for InternalTransactionId {
    fn default() -> Self {
        Self {
            hash: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned(),
            lt: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MasterchainInfo {
    pub last: BlockIdExt,
}

#[derive(Debug, Clone)]
pub struct ShortTxId {
    pub account: String,
    pub lt: i64,
    pub hash: String,
}

#[derive(Debug, Clone)]
pub struct RawFullAccountState {
    pub balance: Option<i64>,
    pub code: String,
    pub data: String,
    pub frozen_hash: String,
    pub last_transaction_id: Option<InternalTransactionId>,
    pub block_id: BlockIdExt,
}

#[derive(Debug, Clone)]
pub struct BlocksHeader {
    pub id: BlockIdExt,
    pub global_id: i32,
    pub version: i32,
    pub flags: i32,
    pub after_merge: bool,
    pub after_split: bool,
    pub before_split: bool,
    pub want_merge: bool,
    pub want_split: bool,
    pub validator_list_hash_short: i32,
    pub catchain_seqno: i32,
    pub min_ref_mc_seqno: i32,
    pub is_key_block: bool,
    pub prev_key_block_seqno: i32,
    pub start_lt: i64,
    pub end_lt: i64,
    pub gen_utime: i64,
    pub vert_seqno: i32,
    pub prev_blocks: Vec<BlockIdExt>,
}

#[derive(Debug, Clone)]
pub struct TvmCell {
    pub bytes: String,
}

#[derive(Debug, Clone)]
pub struct AccountAddress {
    pub account_address: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RawMessage {
    pub source: AccountAddress,
    pub destination: AccountAddress,
    pub value: i64,
    pub fwd_fee: i64,
    pub ihr_fee: i64,
    pub created_lt: i64,
    pub body_hash: String,
    pub msg_data: MsgData,
}

#[derive(Debug, Clone)]
pub enum MsgData {
    Raw { body: String, init_state: String },
    Text { text: String },
    DecryptedText { text: String },
    EncryptedText { text: String },
}

#[derive(Debug, Clone)]
pub struct RawTransaction {
    pub address: AccountAddress,
    pub utime: i64,
    pub data: String,
    pub transaction_id: InternalTransactionId,
    pub fee: i64,
    pub storage_fee: i64,
    pub other_fee: i64,
    pub in_msg: Option<RawMessage>,
    pub out_msgs: Vec<RawMessage>,
}

#[derive(Debug, Clone)]
pub struct InternalAccountAddress {
    pub chain_id: i32,
    pub bytes: [u8; 32],
}

impl std::fmt::Display for InternalAccountAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.chain_id, hex::encode(self.bytes))
    }
}
