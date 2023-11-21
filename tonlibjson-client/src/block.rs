use std::cmp::Ordering;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::time::Duration;
use std::str::FromStr;
use derive_new::new;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use crate::address::{AccountAddressData, ShardContextAccountAddress};
use crate::block::tl::SmcMethodIdName;
use crate::deserialize::{deserialize_number_from_string, deserialize_default_as_none, deserialize_ton_account_balance, deserialize_empty_as_none, serialize_none_as_empty};
use crate::router::{BlockCriteria, Route, Routable};
use crate::request::Requestable;

pub mod tl {
    use derive_new::new;
    use serde::{Serialize, Deserialize};
    use crate::deserialize::deserialize_number_from_string;

    /**
    double ? = Double;
    string ? = String;

    int32 = Int32;
    int53 = Int53;
    int64 = Int64;
    int256 8*[ int32 ] = Int256;
    bytes = Bytes;
    secureString = SecureString;
    secureBytes = SecureBytes;

    object ? = Object;
    function ? = Function;

    boolFalse = Bool;
    boolTrue = Bool;

    vector {t:Type} # [ t ] = Vector t;

     **/

    type Double = f64;
    // type String = String;

    type Int31 = i32; // "#" / nat type
    type Int32 = i32;
    type Int53 = i64;
    type Int64 = i64;

    /* enum BoxedBool {
        BoolFalse,
        BoolTrue
    } */

    type BoxedBool = bool;

    type Bytes = String;

    type Vector<T> = Vec<T>;

    include!(concat!(env!("OUT_DIR"), "/generated.rs"));

    // TODO[akostylev0]
    type TonBoxedBlockIdExt = TonBlockIdExt;
}

pub type Sync = tl::Sync;

impl Requestable for Sync {
    type Response = BlockIdExt;

    fn timeout(&self) -> Duration {
        Duration::from_secs(5 * 60)
    }
}

pub type BlocksGetBlockHeader = tl::BlocksGetBlockHeader;

impl Requestable for BlocksGetBlockHeader {
    type Response = BlockHeader;
}

impl Routable for BlocksGetBlockHeader {
    fn route(&self) -> Route {
        Route::Block { chain: self.id.workchain, criteria: BlockCriteria::Seqno { shard: self.id.shard, seqno: self.id.seqno } }
    }
}

pub type BlockIdExt = tl::TonBlockIdExt;

pub type BlockId = tl::TonBlockId;

impl From<BlockIdExt> for BlockId {
    fn from(block: BlockIdExt) -> Self {
        BlockId {
            workchain: block.workchain,
            shard: block.shard,
            seqno: block.seqno
        }
    }
}

pub type BlockHeader = tl::BlocksHeader;

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

impl PartialOrd for MasterchainInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MasterchainInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.last.seqno.cmp(&other.last.seqno)
    }
}

pub type InternalTransactionId = tl::InternalTransactionId;

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

pub type MessageData = tl::MsgBoxedData;

pub type RawMessage = tl::RawMessage;

pub type RawTransaction = tl::RawTransaction;

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

pub type BlocksShards = tl::BlocksShards;

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

// pub type BlocksTransactions = tl::BlocksTransactions;

#[derive(Debug, Deserialize)]
pub struct BlocksTransactions {
    pub id: BlockIdExt,
    pub incomplete: bool,
    pub req_count: u32,
    pub transactions: Vec<ShortTxId>,
}

pub type AccountTransactionId = tl::BlocksAccountTransactionId;

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

pub type RawExtMessageInfo = tl::RawExtMessageInfo;

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
pub type SmcMethodId = tl::SmcBoxedMethodId;

impl SmcMethodId {
    pub fn by_name(name: &str) -> Self { Self::SmcMethodIdName(SmcMethodIdName { name: name.to_owned() })}
}

pub type Slice = tl::TvmSlice;
pub type Cell = tl::TvmCell;
pub type Number = tl::TvmNumberDecimal;
pub type Tuple = tl::TvmTuple;
pub type List = tl::TvmList;
pub type StackEntry = tl::TvmBoxedStackEntry;
pub type SmcInfo = tl::SmcInfo;

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
    use crate::block::tl::{TvmBoxedList, TvmBoxedNumber, TvmBoxedTuple, TvmList, TvmNumberDecimal, TvmStackEntryCell, TvmStackEntryList, TvmStackEntryNumber, TvmStackEntrySlice, TvmStackEntryTuple, TvmTuple};

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
        let slice = StackEntry::TvmStackEntrySlice(TvmStackEntrySlice { slice: Slice { bytes: "test".to_string() } });
        let cell = StackEntry::TvmStackEntryCell(TvmStackEntryCell { cell: Cell { bytes: "test".to_string() } });
        let number = StackEntry::TvmStackEntryNumber(TvmStackEntryNumber { number: TvmBoxedNumber::TvmNumberDecimal(TvmNumberDecimal { number: "123".to_string() }) });
        let tuple = StackEntry::TvmStackEntryTuple(TvmStackEntryTuple { tuple: TvmBoxedTuple::TvmTuple(TvmTuple { elements: vec![slice.clone(), cell.clone()] })});
        let list = StackEntry::TvmStackEntryList(TvmStackEntryList { list: TvmBoxedList::TvmList(TvmList { elements: vec![slice.clone(), tuple.clone()] })});

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
