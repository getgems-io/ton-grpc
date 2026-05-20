use crate::tlb::account::Account;
use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::hash_update::HashUpdate;
use crate::tlb::transaction::Transaction;
use toner::tlb::hashmap::Hashmap;
use toner::tlb::{Data, Ref, Same};
use toner_tlb_macros::CellDeserialize;

/// ```tlb
/// acc_trans#5 account_addr:bits256
///             transactions:(HashmapAug 64 ^Transaction CurrencyCollection)
///             state_update:^(HASH_UPDATE Account)
///           = AccountBlock;
/// ```
#[derive(Debug, Clone, CellDeserialize)]
#[tlb(tag = "#5")]
pub struct AccountBlock {
    #[tlb(bits)]
    pub account_addr: [u8; 32],
    #[tlb(cell, as = "Hashmap<Ref, Same>", args = "(64, (), ())")]
    pub transaction: Hashmap<Transaction, CurrencyCollection>,
    #[tlb(cell, as = "Ref<Data>")]
    pub state_update: HashUpdate<Account>,
}
