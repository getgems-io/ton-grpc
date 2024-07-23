use crate::address::{AccountAddressData, InternalAccountAddress, ShardContextAccountAddress};
use crate::deserialize::{
    deserialize_default_as_none, deserialize_empty_as_none, deserialize_number_from_string,
    deserialize_ton_account_balance, serialize_none_as_empty,
};
use crate::request::Requestable;
use anyhow::anyhow;
use derive_new::new;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::error::Error as StdError;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::time::Duration;
use ton_client_util::router::route::{BlockCriteria, Route, ToRoute};
use ton_client_util::service::timeout::ToTimeout;

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

impl ToRoute for BlocksGetBlockHeader {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: self.id.shard,
                seqno: self.id.seqno,
            },
        }
    }
}

impl ToTimeout for BlocksGetBlockHeader {}

impl From<TonBlockIdExt> for TonBlockId {
    fn from(block: TonBlockIdExt) -> Self {
        TonBlockId {
            workchain: block.workchain,
            shard: block.shard,
            seqno: block.seqno,
        }
    }
}

impl From<BlocksHeader> for TonBlockId {
    fn from(header: BlocksHeader) -> Self {
        TonBlockId {
            workchain: header.id.workchain,
            shard: header.id.shard,
            seqno: header.id.seqno,
        }
    }
}

impl BlocksShortTxId {
    pub fn account(&self) -> &str {
        &self.account
    }

    pub fn into_internal(self, chain_id: i32) -> InternalAccountAddress {
        ShardContextAccountAddress::from_str(&self.account)
            .unwrap()
            .into_internal(chain_id)
    }

    pub fn into_internal_string(self, chain_id: i32) -> String {
        self.into_internal(chain_id).to_string()
    }
}

impl PartialEq for BlocksShortTxId {
    fn eq(&self, other: &Self) -> bool {
        self.account == other.account && self.hash == other.hash && self.lt == other.lt
    }
}

impl PartialOrd for BlocksMasterchainInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BlocksMasterchainInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.last.seqno.cmp(&other.last.seqno)
    }
}

impl Default for InternalTransactionId {
    fn default() -> Self {
        Self {
            hash: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned(),
            lt: 0,
        }
    }
}

impl AccountAddress {
    // TODO[akostylev0]
    pub fn new(account_address: &str) -> anyhow::Result<Self> {
        AccountAddressData::from_str(account_address)?; // validate

        Ok(Self {
            account_address: Some(account_address.to_owned()),
        })
    }

    pub fn to_data(&self) -> Option<AccountAddressData> {
        self.account_address
            .as_ref()
            .and_then(|a| AccountAddressData::from_str(a).ok())
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

impl ToRoute for GetShardAccountCell {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToTimeout for GetShardAccountCell {}

impl ToRoute for GetShardAccountCellByTransaction {
    fn to_route(&self) -> Route {
        let data = self
            .account_address
            .to_data()
            .expect("invalid account address");

        Route::Block {
            chain: data.chain_id,
            criteria: BlockCriteria::LogicalTime {
                address: data.bytes,
                lt: self.transaction_id.lt,
            },
        }
    }
}

impl ToTimeout for GetShardAccountCellByTransaction {}

impl ToRoute for RawGetAccountState {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToTimeout for RawGetAccountState {}

impl ToRoute for RawGetAccountStateByTransaction {
    fn to_route(&self) -> Route {
        let data = self
            .account_address
            .to_data()
            .expect("invalid account address");

        Route::Block {
            chain: data.chain_id,
            criteria: BlockCriteria::LogicalTime {
                address: data.bytes,
                lt: self.transaction_id.lt,
            },
        }
    }
}

impl ToTimeout for RawGetAccountStateByTransaction {}

impl ToRoute for GetAccountState {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToTimeout for GetAccountState {}

impl ToRoute for BlocksGetMasterchainInfo {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToTimeout for BlocksGetMasterchainInfo {}

impl ToRoute for BlocksLookupBlock {
    fn to_route(&self) -> Route {
        let criteria = match self.mode {
            2 => {
                let mut address = [0_u8; 32];
                address[0..8].copy_from_slice(&self.id.shard.to_be_bytes());

                BlockCriteria::LogicalTime {
                    address,
                    lt: self.lt,
                }
            }
            _ => BlockCriteria::Seqno {
                shard: self.id.shard,
                seqno: self.id.seqno,
            },
        };

        Route::Block {
            chain: self.id.workchain,
            criteria,
        }
    }
}

impl ToTimeout for BlocksLookupBlock {}

impl BlocksLookupBlock {
    pub fn seqno(id: TonBlockId) -> Self {
        Self {
            mode: 1,
            id,
            lt: 0,
            utime: 0,
        }
    }

    pub fn logical_time(id: TonBlockId, lt: i64) -> Self {
        Self {
            mode: 2,
            id,
            lt,
            utime: 0,
        }
    }
}

impl ToRoute for BlocksGetShards {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: self.id.shard,
                seqno: self.id.seqno,
            },
        }
    }
}

impl ToTimeout for BlocksGetShards {}

impl BlocksGetTransactionsExt {
    pub fn unverified(
        block_id: TonBlockIdExt,
        after: Option<BlocksAccountTransactionId>,
        reverse: bool,
        count: i32,
    ) -> Self {
        let count = if count > 256 { 256 } else { count };
        let mode = 1 + 2 + 4 + if after.is_some() { 128 } else { 0 } + if reverse { 64 } else { 0 };

        Self {
            id: block_id,
            mode,
            count,
            after: after.unwrap_or_default(),
        }
    }

    pub fn verified(
        block_id: TonBlockIdExt,
        after: Option<BlocksAccountTransactionId>,
        reverse: bool,
        count: i32,
    ) -> Self {
        let count = if count > 256 { 256 } else { count };
        let mode =
            32 + 1 + 2 + 4 + if after.is_some() { 128 } else { 0 } + if reverse { 64 } else { 0 };

        Self {
            id: block_id,
            mode,
            count,
            after: after.unwrap_or_default(),
        }
    }
}

impl ToRoute for BlocksGetTransactionsExt {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: self.id.shard,
                seqno: self.id.seqno,
            },
        }
    }
}

impl ToTimeout for BlocksGetTransactionsExt {}

impl BlocksGetTransactions {
    pub fn unverified(
        block_id: TonBlockIdExt,
        after: Option<BlocksAccountTransactionId>,
        reverse: bool,
        count: i32,
    ) -> Self {
        let count = if count > 256 { 256 } else { count };
        let mode = 1 + 2 + 4 + if after.is_some() { 128 } else { 0 } + if reverse { 64 } else { 0 };

        Self {
            id: block_id,
            mode,
            count,
            after: after.unwrap_or_default(),
        }
    }

    pub fn verified(
        block_id: TonBlockIdExt,
        after: Option<BlocksAccountTransactionId>,
        reverse: bool,
        count: i32,
    ) -> Self {
        let count = if count > 256 { 256 } else { count };
        let mode =
            32 + 1 + 2 + 4 + if after.is_some() { 128 } else { 0 } + if reverse { 64 } else { 0 };

        Self {
            id: block_id,
            mode,
            count,
            after: after.unwrap_or_default(),
        }
    }
}

impl ToRoute for BlocksGetTransactions {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: self.id.shard,
                seqno: self.id.seqno,
            },
        }
    }
}

impl ToTimeout for BlocksGetTransactions {}

impl Default for BlocksAccountTransactionId {
    fn default() -> Self {
        Self {
            account: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string(),
            lt: 0,
        }
    }
}

impl From<&BlocksShortTxId> for BlocksAccountTransactionId {
    fn from(v: &BlocksShortTxId) -> Self {
        Self {
            account: v.account.to_string(),
            lt: v.lt,
        }
    }
}

impl TryFrom<&RawTransaction> for BlocksAccountTransactionId {
    type Error = anyhow::Error;

    fn try_from(v: &RawTransaction) -> Result<Self, Self::Error> {
        let address_data = v
            .address
            .account_address
            .as_ref()
            .ok_or(anyhow!("empty address"))
            .and_then(|s| AccountAddressData::from_str(s))?;

        Ok(Self {
            account: address_data.into_shard_context().to_string(),
            lt: v.transaction_id.lt,
        })
    }
}

impl ToRoute for RawSendMessage {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToTimeout for RawSendMessage {}

impl ToRoute for RawSendMessageReturnHash {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToTimeout for RawSendMessageReturnHash {}

impl ToRoute for SmcLoad {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToTimeout for SmcLoad {}

impl SmcBoxedMethodId {
    pub fn by_name(name: &str) -> Self {
        Self::SmcMethodIdName(SmcMethodIdName {
            name: name.to_owned(),
        })
    }
}

impl ToTimeout for SmcBoxedMethodId {}

impl<T> Requestable for T
where
    T: Functional + Serialize,
    T::Result: DeserializeOwned,
{
    type Response = T::Result;
}

impl ToRoute for RawGetTransactionsV2 {
    fn to_route(&self) -> Route {
        let data = self
            .account_address
            .to_data()
            .expect("invalid account address");

        Route::Block {
            chain: data.chain_id,
            criteria: BlockCriteria::LogicalTime {
                address: data.bytes,
                lt: self.from_transaction_id.lt,
            },
        }
    }
}

impl ToTimeout for RawGetTransactionsV2 {}

impl ToTimeout for Sync {
    fn to_timeout(&self) -> Option<Duration> {
        Some(Duration::from_secs(5 * 60))
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

impl StdError for TonError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        None
    }
}

#[derive(new, Serialize, Clone)]
#[serde(tag = "@type", rename = "withBlock")]
pub struct WithBlock<T> {
    pub id: TonBlockIdExt,
    pub function: T,
}

impl<T: Functional> Requestable for WithBlock<T>
where
    T: Requestable,
{
    type Response = T::Response;
}

impl<T> ToTimeout for WithBlock<T>
where
    T: ToTimeout,
{
    fn to_timeout(&self) -> Option<Duration> {
        self.function.to_timeout()
    }
}

impl<T: Functional> ToRoute for WithBlock<T> {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: self.id.shard,
                seqno: self.id.seqno,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let address = AccountAddress {
            account_address: None,
        };

        let json = serde_json::to_string(&address).unwrap();

        assert_eq!(
            json,
            "{\"@type\":\"accountAddress\",\"account_address\":\"\"}"
        );
    }

    #[test]
    #[traced_test]
    fn account_address_workchain_id() {
        let tx_id =
            AccountAddress::new("EQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GEMS").unwrap();
        assert_eq!(0, tx_id.chain_id());

        let tx_id = AccountAddress::new(
            "-1:a3935861f79daf59a13d6d182e1640210c02f98e3df18fda74b8f5ab141abf18",
        )
        .unwrap();
        assert_eq!(-1, tx_id.chain_id());

        let tx_id = AccountAddress::new(
            "0:a3935861f79daf59a13d6d182e1640210c02f98e3df18fda74b8f5ab141abf18",
        )
        .unwrap();
        assert_eq!(0, tx_id.chain_id());

        assert!(AccountAddress::new(
            "-1:0:a3935861f79daf59a13d6d182e1640210c02f98e3df18fda74b8f5ab141abf18"
        )
        .is_err());
    }

    #[test]
    fn slice_correct_json() {
        let slice = TvmSlice {
            bytes: "test".to_string(),
        };

        assert_eq!(
            serde_json::to_string(&slice).unwrap(),
            "{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}"
        )
    }

    #[test]
    fn cell_correct_json() {
        let cell = TvmCell {
            bytes: "test".to_string(),
        };

        assert_eq!(
            serde_json::to_string(&cell).unwrap(),
            "{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}"
        )
    }

    #[test]
    fn number_correct_json() {
        let number = TvmNumberDecimal {
            number: "100.2".to_string(),
        };

        assert_eq!(
            serde_json::to_string(&number).unwrap(),
            "{\"@type\":\"tvm.numberDecimal\",\"number\":\"100.2\"}"
        )
    }

    #[test]
    fn stack_entry_correct_json() {
        let slice = TvmBoxedStackEntry::TvmStackEntrySlice(TvmStackEntrySlice {
            slice: TvmSlice {
                bytes: "test".to_string(),
            },
        });
        let cell = TvmBoxedStackEntry::TvmStackEntryCell(TvmStackEntryCell {
            cell: TvmCell {
                bytes: "test".to_string(),
            },
        });
        let number = TvmBoxedStackEntry::TvmStackEntryNumber(TvmStackEntryNumber {
            number: TvmNumberDecimal {
                number: "123".to_string(),
            },
        });
        let tuple = TvmBoxedStackEntry::TvmStackEntryTuple(TvmStackEntryTuple {
            tuple: TvmTuple {
                elements: vec![slice.clone(), cell.clone()],
            },
        });
        let list = TvmBoxedStackEntry::TvmStackEntryList(TvmStackEntryList {
            list: TvmList {
                elements: vec![slice.clone(), tuple.clone()],
            },
        });

        assert_eq!(serde_json::to_string(&slice).unwrap(), "{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}}");
        assert_eq!(serde_json::to_string(&cell).unwrap(), "{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}");
        assert_eq!(serde_json::to_string(&number).unwrap(), "{\"@type\":\"tvm.stackEntryNumber\",\"number\":{\"@type\":\"tvm.numberDecimal\",\"number\":\"123\"}}");
        assert_eq!(serde_json::to_string(&tuple).unwrap(), "{\"@type\":\"tvm.stackEntryTuple\",\"tuple\":{\"@type\":\"tvm.tuple\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}]}}");
        assert_eq!(serde_json::to_string(&list).unwrap(), "{\"@type\":\"tvm.stackEntryList\",\"list\":{\"@type\":\"tvm.list\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryTuple\",\"tuple\":{\"@type\":\"tvm.tuple\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}]}}]}}");
    }

    #[test]
    fn smc_method_id() {
        let number = SmcBoxedMethodId::SmcMethodIdNumber(SmcMethodIdNumber { number: 123 });
        let name = SmcBoxedMethodId::SmcMethodIdName(SmcMethodIdName {
            name: "getOwner".to_owned(),
        });

        assert_eq!(
            serde_json::to_value(number).unwrap(),
            json!({
                "@type": "smc.methodIdNumber",
                "number": 123
            })
        );
        assert_eq!(
            serde_json::to_value(name).unwrap(),
            json!({
                "@type": "smc.methodIdName",
                "name": "getOwner"
            })
        );
    }
}
