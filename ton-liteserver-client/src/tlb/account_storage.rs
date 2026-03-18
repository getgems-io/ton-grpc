use toner::ton::currency::CurrencyCollection;
use crate::tlb::account_state::AccountState;

/// ```tlb
/// account_storage$_ last_trans_lt:uint64
///     balance:CurrencyCollection state:AccountState
///   = AccountStorage;
/// ```
pub struct AccountStorage {
    pub last_trans_lt: u64,
    pub balance: CurrencyCollection,
    pub state: AccountState,
}
