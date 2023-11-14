use std::error::Error;
use std::fmt::{Display, Formatter};
use std::time::Duration;
use std::str::FromStr;
use derive_new::new;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use crate::address::{AccountAddressData, ShardContextAccountAddress};
use crate::deserialize::{deserialize_number_from_string, deserialize_default_as_none, deserialize_ton_account_balance, deserialize_empty_as_none, serialize_none_as_empty};
use crate::router::{BlockCriteria, Route};
use crate::request::{Requestable, Routable};

#[derive(Debug, Serialize, Default, Clone)]
#[serde(tag = "@type", rename = "sync")]
pub struct Sync {}

impl Requestable for Sync {
    type Response = BlockIdExt;

    fn timeout(&self) -> Duration {
        Duration::from_secs(5 * 60)
    }
}

#[derive(Debug, Serialize, Clone, Hash, PartialEq, Eq)]
#[serde(tag = "@type", rename = "blocks.getBlockHeader")]
pub struct BlocksGetBlockHeader {
    pub id: BlockIdExt
}

impl Requestable for BlocksGetBlockHeader {
    type Response = BlockHeader;
}

impl Routable for BlocksGetBlockHeader {
    fn route(&self) -> Route {
        Route::Block { chain: self.id.workchain, criteria: BlockCriteria::Seqno { shard: self.id.shard, seqno: self.id.seqno } }
    }
}

impl BlocksGetBlockHeader {
    pub fn new(id: BlockIdExt) -> Self {
        Self {
            id
        }
    }
}

#[derive(Debug, Hash, Serialize, Deserialize, Clone, Eq, PartialEq, new)]
#[serde(tag = "@type", rename = "ton.blockIdExt")]
pub struct BlockIdExt {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub workchain: i32,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub shard: i64,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub seqno: i32,
    pub root_hash: String,
    pub file_hash: String,
}

#[derive(Debug, Hash, Serialize, Deserialize, Clone, Eq, PartialEq, new)]
#[serde(tag = "@type", rename = "ton.blockId")]
pub struct BlockId {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub workchain: i32,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub shard: i64,
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
#[serde(tag = "@type", rename = "blocks.header")]
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

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "@type", rename = "blocks.shortTxId")]
pub struct ShortTxId {
    pub account: ShardContextAccountAddress,
    pub hash: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub lt: i64,
    pub mode: u8,
}

impl PartialEq for ShortTxId {
    fn eq(&self, other: &Self) -> bool {
        self.account == other.account
        && self.hash == other.hash
        && self.lt == other.lt
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(tag = "@type", rename = "blocks.masterchainInfo")]
pub struct MasterchainInfo {
    pub init: BlockIdExt,
    pub last: BlockIdExt,
    pub state_root_hash: String,
}

#[derive(new, Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "@type", rename = "internal.transactionId")]
pub struct InternalTransactionId {
    pub hash: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub lt: i64,
}

impl Default for InternalTransactionId {
    fn default() -> Self {
        Self {
            hash: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned(),
            lt: 0
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "@type", rename = "accountAddress")]
pub struct AccountAddress {
    #[serde(deserialize_with = "deserialize_empty_as_none", serialize_with = "serialize_none_as_empty")]
    pub account_address: Option<AccountAddressData>,
}

impl AccountAddress {
    pub fn new(account_address: &str) -> anyhow::Result<Self> {
        Ok(Self {
            account_address: Some(AccountAddressData::from_str(account_address)?)
        })
    }

    pub fn chain_id(&self) -> i32 {
        // TODO[akostylev0]
        self.account_address.as_ref().map(|d| d.chain_id).unwrap_or(-1)
    }
}

#[derive(new, Debug, Serialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "getShardAccountCell")]
pub struct GetShardAccountCell {
    pub account_address: AccountAddress
}

impl Requestable for GetShardAccountCell {
    type Response = Cell;
}

impl Routable for GetShardAccountCell {
    fn route(&self) -> Route { Route::Latest }
}

#[derive(new, Debug, Serialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "getShardAccountCellByTransaction")]
pub struct GetShardAccountCellByTransaction {
    pub account_address: AccountAddress,
    pub transaction_id: InternalTransactionId
}

impl Requestable for GetShardAccountCellByTransaction {
    type Response = Cell;
}

impl Routable for GetShardAccountCellByTransaction {
    fn route(&self) -> Route {
        Route::Block { chain: self.account_address.chain_id(), criteria: BlockCriteria::LogicalTime(self.transaction_id.lt) }
    }
}

#[derive(new, Debug, Serialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "raw.getAccountState")]
pub struct RawGetAccountState {
    account_address: AccountAddress
}

#[derive(Deserialize, Debug)]
#[serde(tag = "@type", rename = "raw.FullAccountState")]
pub struct RawFullAccountState {
    #[serde(deserialize_with = "deserialize_ton_account_balance")]
    pub balance: Option<i64>,
    #[serde(deserialize_with = "deserialize_default_as_none")]
    pub code: Option<String>,
    #[serde(deserialize_with = "deserialize_default_as_none")]
    pub data: Option<String>,
    #[serde(deserialize_with = "deserialize_default_as_none")]
    pub last_transaction_id: Option<InternalTransactionId>,
    pub block_id: BlockIdExt,
    #[serde(deserialize_with = "deserialize_default_as_none")]
    pub frozen_hash: Option<String>,
    pub sync_utime: i64
}

impl Requestable for RawGetAccountState {
    type Response = RawFullAccountState;
}

impl Routable for RawGetAccountState {
    fn route(&self) -> Route {
        Route::Latest
    }
}

#[derive(new, Debug, Serialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "raw.getAccountStateByTransaction")]
pub struct RawGetAccountStateByTransaction {
    account_address: AccountAddress,
    transaction_id: InternalTransactionId
}

impl Requestable for RawGetAccountStateByTransaction {
    type Response = RawFullAccountState;
}

impl Routable for RawGetAccountStateByTransaction {
    fn route(&self) -> Route {
        Route::Block { chain: self.account_address.chain_id(), criteria: BlockCriteria::LogicalTime(self.transaction_id.lt)  }
    }
}

#[derive(new, Debug, Serialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "getAccountState")]
pub struct GetAccountState {
    account_address: AccountAddress
}

impl Requestable for GetAccountState {
    type Response = Value;
}

impl Routable for GetAccountState {
    fn route(&self) -> Route { Route::Latest }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "@type")]
pub enum MessageData {
    #[serde(rename = "msg.dataRaw")]
    Raw { body: String, init_state: String },
    #[serde(rename = "msg.dataText")]
    Text { text: String },
    #[serde(rename = "msg.dataDecryptedText")]
    DecryptedText { text: String },
    #[serde(rename = "msg.dataEncryptedText")]
    EncryptedText { text: String }
}

#[derive(Deserialize, Debug)]
#[serde(tag = "@type", rename = "raw.message")]
pub struct RawMessage {
    pub source: AccountAddress,
    pub destination: AccountAddress,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub value: i64,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub fwd_fee: i64,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub ihr_fee: i64,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub created_lt: i64,
    pub body_hash: String,
    pub msg_data: MessageData
}

#[derive(Deserialize, Debug)]
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

    pub in_msg: RawMessage,
    pub out_msgs: Vec<RawMessage>,
}

#[derive(Deserialize, Debug)]
pub struct RawTransactions {
    pub transactions: Vec<RawTransaction>,
    #[serde(deserialize_with = "deserialize_default_as_none")]
    pub previous_transaction_id: Option<InternalTransactionId>
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(tag = "@type", rename = "blocks.getMasterchainInfo")]
pub struct GetMasterchainInfo {}

impl Requestable for GetMasterchainInfo {
    type Response = MasterchainInfo;
}

impl Routable for GetMasterchainInfo {
    fn route(&self) -> Route {
        Route::Latest
    }
}

#[derive(Debug, Serialize, Clone, Hash, Eq, PartialEq)]
#[serde(tag = "@type", rename = "blocks.lookupBlock")]
pub struct BlocksLookupBlock {
    pub mode: i32,
    pub id: BlockId,
    pub lt: i64,
    pub utime: i32
}

impl Requestable for BlocksLookupBlock {
    type Response = BlockIdExt;
}

impl Routable for BlocksLookupBlock {
    fn route(&self) -> Route {
        let criteria = match self.mode {
            1 => BlockCriteria::Seqno { shard: self.id.shard, seqno: self.id.seqno },
            2 => BlockCriteria::LogicalTime(self.lt),
            _ => BlockCriteria::Seqno { shard: self.id.shard, seqno: self.id.seqno }
        };

        Route::Block { chain: self.id.workchain, criteria }
    }
}

impl BlocksLookupBlock {
    pub fn seqno(id: BlockId) -> Self {
        let mode = 1;

        Self {
            mode,
            id,
            lt: 0,
            utime: 0
        }
    }

    pub fn logical_time(id: BlockId, lt: i64) -> Self {
        let mode = 2;

        Self {
            mode,
            id,
            lt,
            utime: 0
        }
    }
}

#[derive(new, Debug, Serialize, Clone)]
#[serde(tag = "@type", rename = "blocks.getShards")]
pub struct BlocksGetShards {
    pub id: BlockIdExt
}

impl Requestable for BlocksGetShards {
    type Response = BlocksShards;
}

impl Routable for BlocksGetShards {
    fn route(&self) -> Route {
        Route::Block { chain: self.id.workchain, criteria: BlockCriteria::Seqno { shard: self.id.shard, seqno: self.id.seqno } }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "blocks.shards")]
pub struct BlocksShards {
    pub shards: Vec<BlockIdExt>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "blocks.getTransactions")]
pub struct BlocksGetTransactions {
    id: BlockIdExt,
    mode: i32,
    count: i32,
    after: AccountTransactionId
}

impl BlocksGetTransactions {
    pub fn unverified(block_id: BlockIdExt, after: Option<AccountTransactionId>, reverse: bool, count: i32) -> Self {
        let count = if count > 256 { 256 } else { count };
        let mode = 1 + 2 + 4
            + if after.is_some() { 128 } else { 0 }
            + if reverse { 64 } else { 0 };

        Self {
            id: block_id,
            mode,
            count,
            after: after.unwrap_or_default(),
        }
    }

    pub fn verified(block_id: BlockIdExt, after: Option<AccountTransactionId>, reverse: bool, count: i32) -> Self {
        let count = if count > 256 { 256 } else { count };
        let mode = 32 + 1 + 2 + 4
            + if after.is_some() { 128 } else { 0 }
            + if reverse { 64 } else { 0 };

        Self {
            id: block_id,
            mode,
            count,
            after: after.unwrap_or_default(),
        }
    }
}

impl Requestable for BlocksGetTransactions {
    type Response = BlocksTransactions;
}

impl Routable for BlocksGetTransactions {
    fn route(&self) -> Route {
        Route::Block { chain: self.id.workchain, criteria: BlockCriteria::Seqno { shard: self.id.shard, seqno: self.id.seqno } }
    }
}

#[derive(Debug, Deserialize)]
pub struct BlocksTransactions {
    pub id: BlockIdExt,
    pub incomplete: bool,
    pub req_count: u32,
    pub transactions: Vec<ShortTxId>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "blocks.accountTransactionId")]
pub struct AccountTransactionId {
    pub account: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub lt: i64,
}

impl Default for AccountTransactionId {
    fn default() -> Self {
        Self {
            account: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string(),
            lt: 0,
        }
    }
}

impl From<&ShortTxId> for AccountTransactionId {
    fn from(v: &ShortTxId) -> Self {
        AccountTransactionId {
            account: v.account.to_string(),
            lt: v.lt,
        }
    }
}

#[derive(new, Debug, Serialize, Deserialize)]
#[serde(tag = "@type", rename = "raw.sendMessage")]
pub struct RawSendMessage {
    pub body: String,
}

impl Requestable for RawSendMessage {
    // TODO[akostylev0]
    type Response = Value;
}

impl Routable for RawSendMessage {
    fn route(&self) -> Route {
        Route::Latest
    }
}

#[derive(new, Debug, Serialize, Deserialize)]
#[serde(tag = "@type", rename = "raw.sendMessageReturnHash")]
pub struct RawSendMessageReturnHash {
    pub body: String,
}

impl Requestable for RawSendMessageReturnHash {
    type Response = RawExtMessageInfo;
}

impl Routable for RawSendMessageReturnHash {
    fn route(&self) -> Route { Route::Latest }
}

#[derive(Deserialize)]
pub struct RawExtMessageInfo {
    pub hash: String
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "@type", rename = "smc.load")]
pub struct SmcLoad {
    pub account_address: AccountAddress
}

impl Requestable for SmcLoad {
    type Response = SmcInfo;
}

impl Routable for SmcLoad {
    fn route(&self) -> Route { Route::Latest }
}

impl SmcLoad {
    pub fn new(address: AccountAddress) -> Self {
        Self {
            account_address: address
        }
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "@type", rename = "smc.runGetMethod")]
pub struct SmcRunGetMethod {
    id: i64,
    method: SmcMethodId,
    stack: SmcStack
}

impl Requestable for SmcRunGetMethod {
    type Response = Value;
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

#[derive(new, Debug, Serialize, Clone)]
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

#[derive(new, Debug, Serialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "raw.getTransactionsV2")]
pub struct RawGetTransactionsV2 {
    pub account_address: AccountAddress,
    from_transaction_id: InternalTransactionId,
    #[new(value = "16")]
    count: i8,
    #[new(value = "false")]
    try_decode_messages: bool
}

impl Requestable for RawGetTransactionsV2 {
    type Response = RawTransactions;
}

impl Routable for RawGetTransactionsV2 {
    fn route(&self) -> Route {
        Route::Block {
            chain: self.account_address.chain_id(),
            criteria: BlockCriteria::LogicalTime(self.from_transaction_id.lt)
        }
    }
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

#[derive(new, Serialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "withBlock")]
pub struct WithBlock<T> {
    pub id: BlockIdExt,
    pub function: T
}

impl<T> Requestable for WithBlock<T> where T : Requestable {
    type Response = T::Response;
}

impl<T> Routable for WithBlock<T> {
    fn route(&self) -> Route {
        Route::Block {
            chain: self.id.workchain,
            criteria: BlockCriteria::Seqno { shard: self.id.shard, seqno: self.id.seqno }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::block::{Cell, List, Number, Slice, StackEntry, Tuple, SmcMethodId, AccountAddress};
    use serde_json::json;
    use tracing_test::traced_test;

    #[test]
    fn deserialize_account_address_empty() {
        let json = json!({"account_address": ""});

        let address = serde_json::from_value::<AccountAddress>(json).unwrap();

        assert!(address.account_address.is_none())
    }

    #[test]
    fn serialize_account_address_empty() {
        let address = AccountAddress { account_address: None };

        let json = serde_json::to_string(&address).unwrap();

        assert_eq!(json, "{\"@type\":\"accountAddress\",\"account_address\":\"\"}");
    }

    #[test]
    #[traced_test]
    fn account_address_workchain_id() {
        let tx_id = AccountAddress::new("EQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GEMS").unwrap();
        assert_eq!(0, tx_id.chain_id());

        let tx_id = AccountAddress::new("-1:a3935861f79daf59a13d6d182e1640210c02f98e3df18fda74b8f5ab141abf18").unwrap();
        assert_eq!(-1, tx_id.chain_id());

        let tx_id = AccountAddress::new("0:a3935861f79daf59a13d6d182e1640210c02f98e3df18fda74b8f5ab141abf18").unwrap();
        assert_eq!(0, tx_id.chain_id());

        assert!(AccountAddress::new("-1:0:a3935861f79daf59a13d6d182e1640210c02f98e3df18fda74b8f5ab141abf18").is_err());
    }

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

        assert_eq!(serde_json::to_value(number).unwrap(), json!({
            "@type": "smc.methodIdNumber",
            "number": 123
        }));
        assert_eq!(serde_json::to_value(name).unwrap(), json!({
            "@type": "smc.methodIdName",
            "name": "getOwner"
        }));
    }
}
