use serde::{Serialize, Deserialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "ton.blockIdExt")]
pub struct BlockIdExt {
    pub workchain: i64,
    pub shard: String,
    pub seqno: u64,
    pub root_hash: String,
    pub file_hash: String,
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

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "@type", rename = "internal.transactionId")]
pub struct InternalTransactionId {
    pub hash: String,
    pub lt: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "@type", rename = "accountAddress")]
pub struct AccountAddress {
    account_address: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "@type", rename = "raw.message")]
pub struct RawMessage {
    source: AccountAddress,
    destination: AccountAddress,
    value: String,
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
    pub utime: i64,
    pub data: String,
    pub transaction_id: InternalTransactionId,
    pub fee: String,
    pub storage_fee: String,
    pub other_fee: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_msg: Option<RawMessage>,
    pub out_msgs: Vec<RawMessage>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct RawTransactions {
    pub transactions: Vec<RawTransaction>,
    pub previous_transaction_id: InternalTransactionId,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "@type", rename = "blocks.getMasterchainInfo")]
pub struct GetMasterchainInfo {}

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

#[derive(Debug, Serialize)]
#[serde(tag = "@type", rename = "smc.runGetMethod")]
pub struct SmcRunGetMethod {
    id: i64,
    method: SmcMethodId,
    stack: Vec<StackEntry>
}

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
