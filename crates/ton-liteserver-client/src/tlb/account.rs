use crate::tlb::msg_address_int::MsgAddressInt;

/// ```tlb
/// account_none$0 = Account;
/// account$1 addr:MsgAddressInt storage_stat:StorageInfo
///           storage:AccountStorage = Account;
/// ```
enum Account {
    None,
    Account {
        addr: MsgAddressInt,
        // storage_stat: StorageInfo,
        // storage: AccountStorage,
    },
}
