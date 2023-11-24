use std::any::{TypeId};
use std::cmp::Ordering;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::time::Duration;
use std::str::FromStr;
use derive_new::new;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use crate::address::{AccountAddressData, InternalAccountAddress, ShardContextAccountAddress};
use crate::block::tl::{Functional, SmcMethodIdName};
use crate::router::{BlockCriteria, Route, Routable};
use crate::request::Requestable;

pub mod tl {
    use derive_new::new;
    use serde::{Serialize, Deserialize};
    use crate::deserialize::{deserialize_number_from_string, deserialize_default_as_none, deserialize_ton_account_balance, serialize_none_as_empty, deserialize_empty_as_none};
    pub trait Functional {
        type Result;
    }

    type Double = f64;
    type Int31 = i32; // "#" / nat type
    type Int32 = i32;
    type Int53 = i64;
    type Int64 = i64;
    type Int256 = String; // TODO[akostylev0] idk actually
    type BoxedBool = bool;
    type Bytes = String;
    type SecureString = String;
    type SecureBytes = String;
    type Vector<T> = Vec<T>;

    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

pub type Sync = tl::Sync;
pub type BlocksGetBlockHeader = tl::BlocksGetBlockHeader;

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

pub type ShortTxId = tl::BlocksShortTxId;

impl ShortTxId {
    pub fn account(&self) -> &str {
        &self.account
    }

    pub fn into_internal(self, chain_id: i32) -> InternalAccountAddress {
        ShardContextAccountAddress::from_str(&self.account).unwrap().into_internal(chain_id)
    }

    pub fn into_internal_string(self, chain_id: i32) -> String {
        self.into_internal(chain_id).to_string()
    }
}

impl PartialEq for ShortTxId {
    fn eq(&self, other: &Self) -> bool {
        self.account == other.account && self.hash == other.hash && self.lt == other.lt
    }
}

pub type MasterchainInfo = tl::BlocksMasterchainInfo;

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

pub type AccountAddress = tl::AccountAddress;

impl AccountAddress {
    // TODO[akostylev0]
    pub fn new(account_address: &str) -> anyhow::Result<Self> {
        AccountAddressData::from_str(account_address)?; // validate

        Ok(Self { account_address: Some(account_address.to_owned()) })
    }

    // TODO[akostylev0]
    pub fn chain_id(&self) -> i32 {
        self.account_address
            .as_ref()
            .and_then(|a| AccountAddressData::from_str(a).ok())
            .map(|d| d.chain_id)
            .unwrap_or(-1)
    }
}

pub type GetShardAccountCell = tl::GetShardAccountCell;

impl Routable for GetShardAccountCell {}

pub type GetShardAccountCellByTransaction = tl::GetShardAccountCellByTransaction;

impl Routable for GetShardAccountCellByTransaction {
    fn route(&self) -> Route {
        Route::Block { chain: self.account_address.chain_id(), criteria: BlockCriteria::LogicalTime(self.transaction_id.lt) }
    }
}

pub type RawFullAccountState = tl::RawFullAccountState;
pub type RawGetAccountState = tl::RawGetAccountState;

impl Routable for RawGetAccountState {}

pub type RawGetAccountStateByTransaction = tl::RawGetAccountStateByTransaction;

impl Routable for RawGetAccountStateByTransaction {
    fn route(&self) -> Route {
        Route::Block { chain: self.account_address.chain_id(), criteria: BlockCriteria::LogicalTime(self.transaction_id.lt)  }
    }
}

pub type GetAccountState = tl::GetAccountState;
impl Routable for GetAccountState {}

pub type MessageData = tl::MsgBoxedData;
pub type RawMessage = tl::RawMessage;
pub type RawTransaction = tl::RawTransaction;
pub type RawTransactions = tl::RawTransactions;
pub type GetMasterchainInfo = tl::BlocksGetMasterchainInfo;
impl Routable for GetMasterchainInfo {}

pub type BlocksLookupBlock = tl::BlocksLookupBlock;

impl Routable for BlocksLookupBlock {
    fn route(&self) -> Route {
        let criteria = match self.mode {
            2 => BlockCriteria::LogicalTime(self.lt),
            1 | _ => BlockCriteria::Seqno { shard: self.id.shard, seqno: self.id.seqno }
        };

        Route::Block { chain: self.id.workchain, criteria }
    }
}

impl BlocksLookupBlock {
    pub fn seqno(id: BlockId) -> Self {
        Self { mode: 1, id, lt: 0, utime: 0 }
    }

    pub fn logical_time(id: BlockId, lt: i64) -> Self {
        Self { mode: 2, id, lt, utime: 0 }
    }
}

pub type BlocksGetShards = tl::BlocksGetShards;

impl Routable for BlocksGetShards {
    fn route(&self) -> Route {
        Route::Block { chain: self.id.workchain, criteria: BlockCriteria::Seqno { shard: self.id.shard, seqno: self.id.seqno } }
    }
}

pub type BlocksShards = tl::BlocksShards;
pub type BlocksGetTransactions = tl::BlocksGetTransactions;

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

impl Routable for BlocksGetTransactions {
    fn route(&self) -> Route {
        Route::Block { chain: self.id.workchain, criteria: BlockCriteria::Seqno { shard: self.id.shard, seqno: self.id.seqno } }
    }
}

pub type BlocksTransactions = tl::BlocksTransactions;
pub type AccountTransactionId = tl::BlocksAccountTransactionId;

impl Default for AccountTransactionId {
    fn default() -> Self {
        Self { account: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string(), lt: 0, }
    }
}

impl From<&ShortTxId> for AccountTransactionId {
    fn from(v: &ShortTxId) -> Self {
        AccountTransactionId { account: v.account.to_string(), lt: v.lt }
    }
}

pub type RawSendMessage = tl::RawSendMessage;
impl Routable for RawSendMessage {}

pub type RawSendMessageReturnHash = tl::RawSendMessageReturnHash;
impl Routable for RawSendMessageReturnHash {}

pub type RawExtMessageInfo = tl::RawExtMessageInfo;
pub type SmcLoad = tl::SmcLoad;
impl Routable for SmcLoad {}

pub type SmcRunGetMethod = tl::SmcRunGetMethod;
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
pub type RawGetTransactionsV2 = tl::RawGetTransactionsV2;


// TODO[akostylev0]
impl<T> Requestable for T where T: Functional + Serialize + Send + std::marker::Sync + 'static,
        T::Result: DeserializeOwned + Send + std::marker::Sync + 'static {
    type Response = T::Result;
    fn timeout(&self) -> Duration {
        if TypeId::of::<T>() == TypeId::of::<Sync>() {
            Duration::from_secs(5 * 60)
        } else {
            Duration::from_secs(3)
        }
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
#[serde(tag = "@type", rename = "withBlock")]
pub struct WithBlock<T> where T : tl::Functional {
    pub id: BlockIdExt,
    pub function: T
}

impl<T: tl::Functional> Requestable for WithBlock<T> where T : Requestable {
    type Response = T::Response;
    fn timeout(&self) -> Duration { self.function.timeout() }
}

impl<T: tl::Functional> Routable for WithBlock<T> {
    fn route(&self) -> Route {
        Route::Block {
            chain: self.id.workchain,
            criteria: BlockCriteria::Seqno { shard: self.id.shard, seqno: self.id.seqno }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::block::{Cell, Number, Slice, StackEntry,SmcMethodId, AccountAddress};
    use serde_json::json;
    use tracing_test::traced_test;
    use crate::block::tl::{SmcMethodIdName, SmcMethodIdNumber, TvmList, TvmNumberDecimal, TvmStackEntryCell, TvmStackEntryList, TvmStackEntryNumber, TvmStackEntrySlice, TvmStackEntryTuple, TvmTuple};

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
        let number = StackEntry::TvmStackEntryNumber(TvmStackEntryNumber { number: TvmNumberDecimal { number: "123".to_string() } });
        let tuple = StackEntry::TvmStackEntryTuple(TvmStackEntryTuple { tuple: TvmTuple { elements: vec![slice.clone(), cell.clone()] } });
        let list = StackEntry::TvmStackEntryList(TvmStackEntryList { list: TvmList { elements: vec![slice.clone(), tuple.clone()] } });

        assert_eq!(serde_json::to_string(&slice).unwrap(), "{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}}");
        assert_eq!(serde_json::to_string(&cell).unwrap(), "{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}");
        assert_eq!(serde_json::to_string(&number).unwrap(), "{\"@type\":\"tvm.stackEntryNumber\",\"number\":{\"@type\":\"tvm.numberDecimal\",\"number\":\"123\"}}");
        assert_eq!(serde_json::to_string(&tuple).unwrap(), "{\"@type\":\"tvm.stackEntryTuple\",\"tuple\":{\"@type\":\"tvm.tuple\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}]}}");
        assert_eq!(serde_json::to_string(&list).unwrap(), "{\"@type\":\"tvm.stackEntryList\",\"list\":{\"@type\":\"tvm.list\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryTuple\",\"tuple\":{\"@type\":\"tvm.tuple\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}]}}]}}");
    }

    #[test]
    fn smc_method_id() {
        let number = SmcMethodId::SmcMethodIdNumber(SmcMethodIdNumber { number: 123 }) ;
        let name = SmcMethodId::SmcMethodIdName(SmcMethodIdName { name: "getOwner".to_owned() });

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
