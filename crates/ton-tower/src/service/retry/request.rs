use crate::request::*;
use crate::service::retry::Retryable;

macro_rules! impl_retryable {
    ($value:expr; $($ty:ty),+ $(,)?) => {
        $(
            impl Retryable for $ty {
                const IS_RETRYABLE: bool = $value;
            }
        )+
    };
}

impl_retryable!(true;
    GetMasterchainInfo,
    Sync,
    LookUpBlockBySeqno,
    LookUpBlockByLt,
    GetShards,
    GetBlockHeader,
    GetTransactionIds,
    GetTransactions,
    GetAccountState,
    GetAccountStateOnBlock,
    GetAccountStateByTransaction,
    GetAccountTransactions,
    GetShardAccountCell,
    GetShardAccountCellOnBlock,
    GetShardAccountCellByTransaction,
    RunGetMethod,
);

impl_retryable!(false;
    SendMessage,
    SendMessageReturningHash,
);
