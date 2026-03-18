use std::collections::HashMap;
use toner::ton::currency::CurrencyCollection;
use toner::ton::message::Message;
use adnl_tcp::types::Int256;
use crate::tlb::account_status::AccountStatus;
use crate::tlb::hash_update::HashUpdate;
use crate::tlb::transaction_descr::TransactionDescr;

/// TODO[akostylev0]
struct Account;

/// ```tlb
/// transaction$0111 account_addr:bits256 lt:uint64
///   prev_trans_hash:bits256 prev_trans_lt:uint64 now:uint32
///   outmsg_cnt:uint15
///   orig_status:AccountStatus end_status:AccountStatus
///   ^[ in_msg:(Maybe ^(Message Any)) out_msgs:(HashmapE 15 ^(Message Any)) ]
///   total_fees:CurrencyCollection state_update:^(HASH_UPDATE Account)
///   description:^TransactionDescr = Transaction;
/// ```
struct Transaction {
    account_addr: Int256,
    lt: u64,
    prev_trans_hash: Int256,
    prev_trans_lt: u64,
    now: u32,
    outmsg_cnt: u16,
    orig_status: AccountStatus,
    end_status: AccountStatus,
    in_msg: Option<Message>,
    out_msgs: HashMap<u16, Message>,
    total_fees: CurrencyCollection,
    state_update: HashUpdate<Account>,
    description: TransactionDescr,
}
