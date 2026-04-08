use crate::tlb::account::Account;
use crate::tlb::account_status::AccountStatus;
use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::hash_update::HashUpdate;
use std::collections::HashMap;
use toner::tlb::Cell;

/// ```tlb
/// transaction$0111 account_addr:bits256 lt:uint64
///   prev_trans_hash:bits256 prev_trans_lt:uint64 now:uint32
///   outmsg_cnt:uint15
///   orig_status:AccountStatus end_status:AccountStatus
///   ^[ in_msg:(Maybe ^(Message Any)) out_msgs:(HashmapE 15 ^(Message Any)) ]
///   total_fees:CurrencyCollection state_update:^(HASH_UPDATE Account)
///   description:^TransactionDescr = Transaction;
/// ```
pub struct Transaction {
    account_addr: [u8; 32],
    lt: u64,
    prev_trans_hash: [u8; 32],
    prev_trans_lt: u64,
    now: u32,
    outmsg_cnt: u16,
    orig_status: AccountStatus,
    end_status: AccountStatus,
    in_msg: Option<Cell>,         // TODO[akostylev0]: Message
    out_msgs: HashMap<u16, Cell>, // TODO[akostylev0]: Message
    total_fees: CurrencyCollection,
    state_update: HashUpdate<Account>,
    description: Cell, // TODO[akostylev0]: TransactionDescr
}
