use crate::tlb::account_storage::AccountStorage;
use crate::tlb::msg_address_int::MsgAddressInt;
use crate::tlb::storage_info::StorageInfo;
use toner_tlb_macros::CellDeserialize;

/// ```tlb
/// account_none$0 = Account;
/// account$1 addr:MsgAddressInt storage_stat:StorageInfo
///           storage:AccountStorage = Account;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, CellDeserialize)]
pub enum Account {
    #[tlb(tag = "$0")]
    None,
    #[tlb(tag = "$1")]
    Account {
        #[tlb(unpack)]
        addr: MsgAddressInt,
        #[tlb(unpack)]
        storage_stat: StorageInfo,
        storage: AccountStorage,
    },
}
