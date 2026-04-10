use crate::tlb::account_state::AccountState;
use crate::tlb::currency_collection::CurrencyCollection;
use toner_tlb_macros::CellDeserialize;

/// ```tlb
/// account_storage$_ last_trans_lt:uint64
///     balance:CurrencyCollection state:AccountState
///   = AccountStorage;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, CellDeserialize)]
pub struct AccountStorage {
    #[tlb(unpack)]
    last_trans_lt: u64,
    balance: CurrencyCollection,
    #[tlb(unpack)]
    state: AccountState,
}
