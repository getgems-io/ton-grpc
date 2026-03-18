use crate::tlb::account_storage::AccountStorage;
use crate::tlb::msg_address_int::MsgAddressInt;
use crate::tlb::storage_info::StorageInfo;

/// ```tlb
/// account_none$0 = Account;
/// account$1 addr:MsgAddressInt storage_stat:StorageInfo
///           storage:AccountStorage = Account;
/// ```
enum Account {
    None,
    Som { addr: MsgAddressInt, storage_stat: StorageInfo, storage: AccountStorage }
}
