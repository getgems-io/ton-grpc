use derive_new::new;
use private::Functional;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use serde_json::Value;
use std::cmp::Ordering;
use std::error::Error as StdError;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use ton_address::SmartContractAddress;

mod private {
    pub trait Functional {
        type Result;
    }
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

pub trait Requestable
where
    Self: Serialize,
{
    type Response: DeserializeOwned;
}

impl Requestable for Value {
    type Response = Value;
}

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
    pub fn new(account_address: &SmartContractAddress) -> Self {
        Self {
            account_address: Some(account_address.to_raw().to_string()),
        }
    }

    pub fn to_data(&self) -> Option<SmartContractAddress> {
        self.account_address
            .as_ref()
            .and_then(|a| SmartContractAddress::from_str(a).ok())
    }

    // TODO[akostylev0]
    pub fn chain_id(&self) -> i32 {
        self.account_address
            .as_ref()
            .and_then(|a| SmartContractAddress::from_str(a).ok())
            .map(|d| d.workchain_id())
            .unwrap_or(-1)
    }
}

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

impl SmcBoxedMethodId {
    pub fn by_name(name: &str) -> Self {
        Self::SmcMethodIdName(SmcMethodIdName {
            name: name.to_owned(),
        })
    }
}

impl<T> Requestable for T
where
    T: Functional + Serialize,
    T::Result: DeserializeOwned,
{
    type Response = T::Result;
}

#[derive(Debug, Deserialize)]
pub struct TonError {
    code: i32,
    message: String,
}

impl TonError {
    pub fn code(&self) -> i32 {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
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

#[derive(new, Debug, Serialize, Clone)]
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

fn deserialize_number_from_string<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr + serde::Deserialize<'de>,
    <T as FromStr>::Err: Display,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrInt<T> {
        String(String),
        Number(T),
    }

    match StringOrInt::<T>::deserialize(deserializer)? {
        StringOrInt::String(s) => s.parse::<T>().map_err(serde::de::Error::custom),
        StringOrInt::Number(i) => Ok(i),
    }
}

fn deserialize_default_as_none<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Default + serde::Deserialize<'de> + PartialEq,
{
    let v = T::deserialize(deserializer)?;

    Ok(if v == T::default() { None } else { Some(v) })
}

fn deserialize_empty_as_none<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr + serde::Deserialize<'de>,
    <T as FromStr>::Err: Display,
{
    let v = String::deserialize(deserializer)?;

    if v.is_empty() {
        Ok(None)
    } else {
        Ok(Some(T::from_str(&v).map_err(de::Error::custom)?))
    }
}

fn deserialize_ton_account_balance<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let v: i64 = deserialize_number_from_string(deserializer)?;

    Ok(if v == -1 { None } else { Some(v) })
}

fn serialize_none_as_empty<S, T>(v: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    match v {
        None => serializer.serialize_str(""),
        Some(v) => v.serialize(serializer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

        assert_eq!(
            serde_json::to_string(&slice).unwrap(),
            "{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}}"
        );
        assert_eq!(
            serde_json::to_string(&cell).unwrap(),
            "{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}"
        );
        assert_eq!(
            serde_json::to_string(&number).unwrap(),
            "{\"@type\":\"tvm.stackEntryNumber\",\"number\":{\"@type\":\"tvm.numberDecimal\",\"number\":\"123\"}}"
        );
        assert_eq!(
            serde_json::to_string(&tuple).unwrap(),
            "{\"@type\":\"tvm.stackEntryTuple\",\"tuple\":{\"@type\":\"tvm.tuple\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}]}}"
        );
        assert_eq!(
            serde_json::to_string(&list).unwrap(),
            "{\"@type\":\"tvm.stackEntryList\",\"list\":{\"@type\":\"tvm.list\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryTuple\",\"tuple\":{\"@type\":\"tvm.tuple\",\"elements\":[{\"@type\":\"tvm.stackEntrySlice\",\"slice\":{\"@type\":\"tvm.slice\",\"bytes\":\"test\"}},{\"@type\":\"tvm.stackEntryCell\",\"cell\":{\"@type\":\"tvm.cell\",\"bytes\":\"test\"}}]}}]}}"
        );
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
