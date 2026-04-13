use crate::tlb::account_block::AccountBlock;
use crate::tlb::currency_collection::CurrencyCollection;
use toner::tlb::hashmap::aug::HashmapAugE;
use toner::tlb::Same;
use toner_tlb_macros::CellDeserialize;

/// ```tlb
/// _ (HashmapAugE 256 AccountBlock CurrencyCollection) = ShardAccountBlocks;
/// ```
#[derive(Debug, Clone, CellDeserialize)]
pub struct ShardAccountBlocks(
    #[tlb(parse_as = "HashmapAugE<Same, Same>", args = "(256, (), ())")]
    pub  HashmapAugE<AccountBlock, CurrencyCollection>,
);
