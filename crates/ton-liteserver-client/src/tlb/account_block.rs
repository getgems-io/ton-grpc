use crate::tlb::account::Account;
use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::hash_update::HashUpdate;
use toner::tlb::hashmap::Hashmap;
use toner::tlb::{Cell, Data, Ref, Same};
use toner_tlb_macros::CellDeserialize;

/// ```tlb
/// acc_trans#5 account_addr:bits256
///             transactions:(HashmapAug 64 ^Transaction CurrencyCollection)
///             state_update:^(HASH_UPDATE Account)
///           = AccountBlock;
/// ```
// TODO: store parsed Transaction instead of raw Cell once CellSerialize is implemented for Transaction
#[derive(Debug, Clone, CellDeserialize)]
#[tlb(tag = "#5")]
pub struct AccountBlock {
    #[tlb(unpack)]
    pub account_addr: [u8; 32],
    #[tlb(parse_as = "Hashmap<Ref, Same>", args = "(64, (), ())")]
    pub transactions: Hashmap<Cell, CurrencyCollection>,
    #[tlb(parse_as = "Ref<Data>")]
    pub state_update: HashUpdate<Account>,
}
