use std::error::Error;
use std::fmt::{Display, Formatter};
use serde::{Serialize, Deserialize};
use serde_json::Value;
use serde_aux::prelude::*;

#[derive(Debug, Serialize, Default)]
#[serde(tag = "@type", rename = "sync")]
pub struct Sync {}

#[derive(Debug, Serialize)]
#[serde(tag = "@type", rename = "blocks.getBlockHeader")]
pub struct GetBlockHeader {
    id: BlockIdExt
}

impl GetBlockHeader {
    pub fn new(id: BlockIdExt) -> Self {
        Self {
            id
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "ton.blockIdExt")]
pub struct BlockIdExt {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub workchain: i64,
    pub shard: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub seqno: i32,
    pub root_hash: String,
    pub file_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "ton.blockId")]
pub struct BlockId {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub workchain: i64,
    pub shard: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub seqno: i32
}

impl From<BlockIdExt> for BlockId {
    fn from(block: BlockIdExt) -> Self {
        BlockId {
            workchain: block.workchain,
            shard: block.shard,
            seqno: block.seqno
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct BlockHeader {
    pub id: BlockIdExt,
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
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub start_lt: i64,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub end_lt: i64,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub gen_utime: i64,
    pub prev_blocks: Vec<BlockIdExt>
}

impl From<BlockHeader> for BlockId {
    fn from(header: BlockHeader) -> Self {
        BlockId {
            workchain: header.id.workchain,
            shard: header.id.shard,
            seqno: header.id.seqno
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "blocks.shortTxId")]
pub struct ShortTxId {
    pub account: String,
    pub hash: String,
    pub lt: String,
    pub mode: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "blocks.masterchainInfo")]
pub struct MasterchainInfo {
    pub init: BlockIdExt,
    pub last: BlockIdExt,
    pub state_root_hash: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "@type", rename = "internal.transactionId")]
pub struct InternalTransactionId {
    pub hash: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub lt: i64,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "@type", rename = "accountAddress")]
pub struct AccountAddress {
    account_address: String,
}

impl AccountAddress {
    pub fn new(account_address: String) -> Self {
        Self {
            account_address
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "@type", rename = "raw.message")]
pub struct RawMessage {
    source: AccountAddress,
    destination: AccountAddress,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub value: i64,
    fwd_fee: String,
    ihr_fee: String,
    created_lt: String,
    body_hash: String,
    msg_data: Value, // @todo maybe only msg.dataRaw
    // @todo deserialize boc
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "@type", rename = "raw.transaction")]
pub struct RawTransaction {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub utime: i64,
    pub data: String,
    pub transaction_id: InternalTransactionId,

    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub fee: i64,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub storage_fee: i64,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub other_fee: i64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_msg: Option<RawMessage>,
    pub out_msgs: Vec<RawMessage>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct RawTransactions {
    pub transactions: Vec<RawTransaction>,
    pub previous_transaction_id: InternalTransactionId,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(tag = "@type", rename = "blocks.getMasterchainInfo")]
pub struct GetMasterchainInfo {}

#[derive(Debug, Serialize)]
#[serde(tag = "@type", rename = "blocks.lookupBlock")]
pub struct BlocksLookupBlock {
    pub mode: i32,
    pub id: BlockId,
    pub lt: i64,
    pub utime: i32
}

impl BlocksLookupBlock {
    pub fn new(id: BlockId, lt: i64, utime: i32) -> Self {
        let mut mode: i32 = 0;
        if id.seqno > 0 {
            mode += 1
        }
        if lt > 0 {
            mode += 2
        }

        Self {
            mode,
            id,
            lt,
            utime
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShardsResponse {
    pub shards: Vec<BlockIdExt>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionsResponse {
    pub id: BlockIdExt,
    pub incomplete: bool,
    pub req_count: u32,
    pub transactions: Vec<ShortTxId>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "blocks.accountTransactionId")]
pub struct AccountTransactionId {
    pub account: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub lt: String,
}

impl Default for AccountTransactionId {
    fn default() -> Self {
        Self {
            account: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string(),
            lt: "".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "raw.sendMessage")]
pub struct RawSendMessage {
    pub body: String,
}

impl From<&ShortTxId> for AccountTransactionId {
    fn from(v: &ShortTxId) -> Self {
        AccountTransactionId {
            account: v.account.clone(),
            lt: v.lt.clone(),
        }
    }
}


#[derive(Debug, Serialize)]
#[serde(tag = "@type", rename = "smc.load")]
pub struct SmcLoad {
    pub account_address: AccountAddress
}

impl SmcLoad {
    pub fn new(address: String) -> Self {
        Self {
            account_address: AccountAddress::new(address)
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "@type", rename = "smc.runGetMethod")]
pub struct SmcRunGetMethod {
    id: i64,
    method: SmcMethodId,
    stack: SmcStack
}

impl SmcRunGetMethod {
    pub fn new(contract_id: i64, method: SmcMethodId, stack: SmcStack) -> Self {
        Self {
            id: contract_id,
            method,
            stack
        }
    }
}

pub type SmcStack = Vec<StackEntry>;

#[derive(Debug, Serialize)]
#[serde(tag = "@type")]
pub enum SmcMethodId {
    #[serde(rename = "smc.methodIdNumber")]
    Number { number: i32 },
    #[serde(rename = "smc.methodIdName")]
    Name { name: String }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "tvm.slice")]
pub struct Slice {
    pub bytes: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "tvm.cell")]
pub struct Cell {
    pub bytes: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "tvm.numberDecimal")]
pub struct Number {
    pub number: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "tvm.tuple")]
pub struct Tuple {
    pub elements: Vec<StackEntry>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "tvm.list")]
pub struct List {
    pub elements: Vec<StackEntry>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type")]
pub enum StackEntry {
    #[serde(rename = "tvm.stackEntrySlice")]
    Slice { slice: Slice },
    #[serde(rename = "tvm.stackEntryCell")]
    Cell { cell: Cell },
    #[serde(rename = "tvm.stackEntryNumber")]
    Number { number: Number },
    #[serde(rename = "tvm.stackEntryTuple")]
    Tuple { tuple: Tuple },
    #[serde(rename = "tvm.stackEntryList")]
    List { list: List },

    #[serde(rename = "tvm.stackEntryUnsupported")]
    Unsupported
}

#[derive(Debug, Deserialize)]
#[serde(tag = "smc.info")]
pub struct SmcInfo {
    pub id: i64
}

#[derive(Debug, Deserialize)]
pub struct TonError {
    code: i32,
    message: String,
}

impl Display for TonError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Ton error occurred with code {}, message {}",
            self.code, self.message
        )
    }
}

impl Error for TonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::block::{Cell, List, Number, Slice, StackEntry, Tuple, SmcMethodId};
    use serde_json::json;

    #[test]
    fn slice_correct_json() {
        let slice = Slice { bytes: "test".to_string() };

        assert_eq!(serde_json::to_string(&slice).unwrap(), "{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}")
    }

    #[test]
    fn cell_correct_json() {
        let cell = Cell { bytes: "test".to_string() };

        assert_eq!(serde_json::to_string(&cell).unwrap(), "{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}")
    }

    #[test]
    fn number_correct_json() {
        let number = Number { number: "100.2".to_string() };

        assert_eq!(serde_json::to_string(&number).unwrap(), "{\"@type\":\"tvm.numberDecimal\",\"number\":\"100.2\"}")
    }

    #[test]
    fn stack_entry_correct_json() {
        let slice = StackEntry::Slice { slice: Slice { bytes: "test".to_string() }};
        let cell = StackEntry::Cell { cell: Cell { bytes: "test".to_string() }};
        let number = StackEntry::Number { number: Number { number: "123".to_string() }};
        let tuple = StackEntry::Tuple { tuple: Tuple { elements: vec![slice.clone(), cell.clone()]  }};
        let list = StackEntry::List { list: List { elements: vec![slice.clone(), tuple.clone()]  }};

        assert_eq!(serde_json::to_string(&slice).unwrap(), "{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}}");
        assert_eq!(serde_json::to_string(&cell).unwrap(), "{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}");
        assert_eq!(serde_json::to_string(&number).unwrap(), "{\"@type\":\"tvm.stackEntryNumber\",\"number\":{\"@type\":\"tvm.numberDecimal\",\"number\":\"123\"}}");
        assert_eq!(serde_json::to_string(&tuple).unwrap(), "{\"@type\":\"tvm.stackEntryTuple\",\"tuple\":{\"@type\":\"tvm.tuple\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}]}}");
        assert_eq!(serde_json::to_string(&list).unwrap(), "{\"@type\":\"tvm.stackEntryList\",\"list\":{\"@type\":\"tvm.list\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryTuple\",\"tuple\":{\"@type\":\"tvm.tuple\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}]}}]}}");
    }

    #[test]
    fn smc_method_id() {
        let number = SmcMethodId::Number { number: 123 };
        let name = SmcMethodId::Name { name: "getOwner".to_owned() };

        assert_eq!(serde_json::to_value(&number).unwrap(), json!({
            "@type": "smc.methodIdNumber",
            "number": 123
        }));
        assert_eq!(serde_json::to_value(&name).unwrap(), json!({
            "@type": "smc.methodIdName",
            "name": "getOwner"
        }));
    }
}
