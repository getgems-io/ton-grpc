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
#[derive(Debug, Clone, PartialEq, Eq, CellDeserialize)]
#[tlb(tag = "#5")]
pub struct AccountBlock {
    #[tlb(unpack)]
    account_addr: [u8; 32],
    #[tlb(parse_as = "Hashmap<Ref, Same>", args = "(64, (), ())")]
    transaction: Hashmap<Transaction, CurrencyCollection>,
    #[tlb(parse_as = "Ref<Data>")]
    state_update: HashUpdate<Account>,
}
