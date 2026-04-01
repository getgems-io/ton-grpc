use ton_address::SmartContractAddress;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockId {
    pub workchain: i32,
    pub shard: i64,
    pub seqno: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockIdExt {
    pub workchain: i32,
    pub shard: i64,
    pub seqno: i32,
    pub root_hash: String,
    pub file_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionId {
    pub lt: i64,
    pub hash: String,
}

#[derive(Debug, Clone)]
pub struct MasterchainInfo {
    pub last: BlockIdExt,
    pub state_root_hash: String,
    pub init: BlockIdExt,
}

#[derive(Debug, Clone)]
pub struct Shards {
    pub shards: Vec<BlockIdExt>,
}

#[derive(Debug, Clone)]
pub struct BlockHeader {
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

#[derive(Debug, Clone, PartialEq)]
pub struct ShortTxId {
    pub account: SmartContractAddress,
    pub lt: i64,
    pub hash: String,
}

#[derive(Debug, Clone)]
pub struct AccountState {
    pub balance: Option<i64>,
    pub code: String,
    pub data: String,
    pub frozen_hash: String,
    pub last_transaction_id: Option<TransactionId>,
    pub block_id: BlockIdExt,
    pub sync_utime: i64,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub address: SmartContractAddress,
    pub utime: i64,
    pub data: String,
    pub transaction_id: TransactionId,
    pub fee: i64,
    pub storage_fee: i64,
    pub other_fee: i64,
    pub in_msg: Option<Message>,
    pub out_msgs: Vec<Message>,
}

#[derive(Debug, Clone)]
pub struct Transactions {
    pub transactions: Vec<Transaction>,
    pub previous_transaction_id: Option<TransactionId>,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub hash: String,
    pub source: SmartContractAddress,
    pub destination: SmartContractAddress,
    pub value: i64,
    pub fwd_fee: i64,
    pub ihr_fee: i64,
    pub created_lt: i64,
    pub body_hash: String,
    pub msg_data: MessageData,
}

#[derive(Debug, Clone)]
pub enum MessageData {
    Raw { body: String, init_state: String },
    Text { text: String },
    DecryptedText { text: String },
    EncryptedText { text: String },
}

#[derive(Debug, Clone)]
pub struct Cell {
    pub bytes: String,
}

#[derive(Debug, Clone)]
pub struct SmcRunResult {
    pub gas_used: i64,
    pub exit_code: i32,
    pub stack: Vec<StackEntry>,
}

#[derive(Debug, Clone)]
pub enum StackEntry {
    Slice { bytes: String },
    Cell { bytes: String },
    Number { number: String },
    Tuple { elements: Vec<StackEntry> },
    List { elements: Vec<StackEntry> },
    Unsupported,
}

#[derive(Debug, Clone)]
pub struct BlockTransactions {
    pub incomplete: bool,
    pub transactions: Vec<ShortTxId>,
}

#[derive(Debug, Clone)]
pub struct BlockTransactionsExt {
    pub incomplete: bool,
    pub transactions: Vec<Transaction>,
}

#[derive(Debug, Clone)]
pub struct ExtMessageInfo {
    pub hash: String,
}
