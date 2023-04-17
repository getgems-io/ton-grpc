use std::error::Error;
use std::fmt::{Display, Formatter};
use std::time::Duration;
use anyhow::anyhow;
use base64::Engine;
use derive_new::new;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use crate::deserialize::{deserialize_number_from_string, deserialize_default_as_none, deserialize_ton_account_balance};
use crate::balance::{BlockCriteria, Route};
use crate::request::{Requestable, RequestBody, Routable};

#[derive(Debug, Serialize, Default, Clone)]
#[serde(tag = "@type", rename = "sync")]
pub struct Sync {}

impl Requestable for Sync {
    type Response = BlockIdExt;

    fn timeout(&self) -> Duration {
        Duration::from_secs(5 * 60)
    }

    fn into_request_body(self) -> RequestBody {
        RequestBody::Sync(self)
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "@type", rename = "blocks.getBlockHeader")]
pub struct BlocksGetBlockHeader {
    id: BlockIdExt
}

impl Requestable for BlocksGetBlockHeader {
    type Response = BlockHeader;

    fn into_request_body(self) -> RequestBody {
        RequestBody::BlocksGetBlockHeader(self)
    }
}

impl Routable for BlocksGetBlockHeader {
    fn route(&self) -> Route {
        Route::Block { chain: self.id.workchain, criteria: BlockCriteria::Seqno(self.id.seqno) }
    }
}

impl BlocksGetBlockHeader {
    pub fn new(id: BlockIdExt) -> Self {
        Self {
            id
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, new)]
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

#[derive(new, Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "blocks.shortTxId")]
pub struct ShortTxId {
    pub account: String,
    pub hash: String,
    pub lt: String,
    pub mode: u8,
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
    pub account_address: String,

    #[serde(skip)]
    _chain_id: i32
}

impl AccountAddress {
    pub fn new(account_address: &str) -> anyhow::Result<Self> {
        let _chain_id = Self::parse_chain_id(account_address)?;

        Ok(Self {
            account_address: account_address.to_owned(),
            _chain_id
        })
    }

    pub fn chain_id(&self) -> i32 {
        self._chain_id
    }

    fn parse_chain_id(address: &str) -> anyhow::Result<i32> {
        if let Some(pos) = address.find(':') {
            return Ok(address[0..pos].parse()?)
        } else if hex::decode(address).is_ok() {
            return Ok(-1)
        } else if let Ok(data) = base64::engine::general_purpose::URL_SAFE.decode(address) {
            let workchain = data[1];

            return Ok(if workchain == u8::MAX { -1 } else { workchain as i32 })
        } else if let Ok(data) = base64::engine::general_purpose::STANDARD.decode(address) {
            let workchain = data[1];

            return Ok(if workchain == u8::MAX { -1 } else { workchain as i32 })
        }

        Err(anyhow!("unknown address format: {}", address))
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

    fn into_request_body(self) -> RequestBody {
        RequestBody::GetShardAccountCell(self)
    }
}

impl Routable for GetShardAccountCell {
    fn route(&self) -> Route {
        Route::Latest { chain: self.account_address.chain_id() }
    }
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

    fn into_request_body(self) -> RequestBody {
        RequestBody::GetShardAccountCellByTransaction(self)
    }
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

    fn into_request_body(self) -> RequestBody {
        RequestBody::RawGetAccountState(self)
    }
}

impl Routable for RawGetAccountState {
    fn route(&self) -> Route {
        Route::Latest { chain: self.account_address.chain_id() }
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

    fn into_request_body(self) -> RequestBody {
        RequestBody::RawGetAccountStateByTransaction(self)
    }
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

    fn into_request_body(self) -> RequestBody {
        RequestBody::GetAccountState(self)
    }
}

impl Routable for GetAccountState {
    fn route(&self) -> Route {
        Route::Latest { chain: self.account_address.chain_id() }
    }
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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_msg: Option<RawMessage>,
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

    fn into_request_body(self) -> RequestBody {
        RequestBody::GetMasterchainInfo(self)
    }
}

impl Routable for GetMasterchainInfo {
    fn route(&self) -> Route {
        Route::Latest { chain: -1 }
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "@type", rename = "blocks.lookupBlock")]
pub struct BlocksLookupBlock {
    pub mode: i32,
    pub id: BlockId,
    pub lt: i64,
    pub utime: i32,

    #[serde(skip)]
    _criteria: BlockCriteria
}

impl Requestable for BlocksLookupBlock {
    type Response = BlockIdExt;

    fn into_request_body(self) -> RequestBody {
        RequestBody::BlocksLookupBlock(self)
    }
}

impl Routable for BlocksLookupBlock {
    fn route(&self) -> Route {
        Route::Block { chain: self.id.workchain, criteria: self._criteria }
    }
}

impl BlocksLookupBlock {
    pub fn seqno(id: BlockId) -> Self {
        let mode = 1;

        let seqno = id.seqno;

        Self {
            mode,
            id,
            lt: 0,
            utime: 0,

            _criteria: BlockCriteria::Seqno(seqno)
        }
    }

    pub fn logical_time(id: BlockId, lt: i64) -> Self {
        let mode = 2;

        Self {
            mode,
            id,
            lt,
            utime: 0,

            _criteria: BlockCriteria::LogicalTime(lt)
        }
    }
}

#[derive(new, Debug, Serialize, Clone)]
#[serde(tag = "@type", rename = "blocks.getShards")]
pub struct BlocksGetShards {
    id: BlockIdExt
}

impl Requestable for BlocksGetShards {
    type Response = BlocksShards;

    fn into_request_body(self) -> RequestBody {
        RequestBody::BlocksGetShards(self)
    }
}

impl Routable for BlocksGetShards {
    fn route(&self) -> Route {
        Route::Block { chain: self.id.workchain, criteria: BlockCriteria::Seqno(self.id.seqno) }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "@type", rename = "blocks.shards")]
pub struct BlocksShards {
    pub shards: Vec<BlockIdExt>,
}

#[derive(new, Debug, Serialize, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "blocks.getTransactions")]
pub struct BlocksGetTransactions {
    id: BlockIdExt,
    #[new(value = "135")]
    mode: i32,
    #[new(value = "30")]
    count: i32,
    after: AccountTransactionId
}

impl Requestable for BlocksGetTransactions {
    type Response = BlocksTransactions;

    fn into_request_body(self) -> RequestBody {
        RequestBody::BlocksGetTransactions(self)
    }
}

impl Routable for BlocksGetTransactions {
    fn route(&self) -> Route {
        Route::Block { chain: self.id.workchain, criteria: BlockCriteria::Seqno(self.id.seqno) }
    }
}

#[derive(Debug, Serialize, Deserialize)]
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

impl From<&ShortTxId> for AccountTransactionId {
    fn from(v: &ShortTxId) -> Self {
        AccountTransactionId {
            account: v.account.clone(),
            lt: v.lt.clone(),
        }
    }
}

#[derive(new, Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "raw.sendMessage")]
pub struct RawSendMessage {
    pub body: String,
}

impl Requestable for RawSendMessage {
    // TODO[akostylev0]
    type Response = Value;

    fn into_request_body(self) -> RequestBody {
        RequestBody::RawSendMessage(self)
    }
}

impl Routable for RawSendMessage {
    fn route(&self) -> Route {
        Route::Any
    }
}


#[derive(Debug, Serialize, Clone)]
#[serde(tag = "@type", rename = "smc.load")]
pub struct SmcLoad {
    pub account_address: AccountAddress
}

impl Requestable for SmcLoad {
    type Response = SmcInfo;

    fn into_request_body(self) -> RequestBody {
        RequestBody::SmcLoad(self)
    }
}

impl Routable for SmcLoad {
    fn route(&self) -> Route {
        Route::Latest { chain: self.account_address.chain_id() }
    }
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

    fn into_request_body(self) -> RequestBody {
        RequestBody::SmcRunGetMethod(self)
    }
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

    fn into_request_body(self) -> RequestBody {
        RequestBody::RawGetTransactionsV2(self)
    }
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

    fn into_request_body(self) -> RequestBody {
        RequestBody::Value(serde_json::to_value(self).expect("must be valid"))
    }
}

impl<T> Routable for WithBlock<T> {
    fn route(&self) -> Route {
        Route::Block {
            chain: self.id.workchain,
            criteria: BlockCriteria::Seqno(self.id.seqno)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::block::{Cell, List, Number, Slice, StackEntry, Tuple, SmcMethodId, AccountAddress};
    use serde_json::json;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn account_address_workchain_id() {
        let tx_id = AccountAddress::new("EQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GEMS").unwrap();
        assert_eq!(0, tx_id.chain_id());

        let tx_id = AccountAddress::new("-1:qweq").unwrap();
        assert_eq!(-1, tx_id.chain_id());

        let tx_id = AccountAddress::new("0:qweq").unwrap();
        assert_eq!(0, tx_id.chain_id())
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
