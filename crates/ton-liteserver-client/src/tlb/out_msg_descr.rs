use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::out_msg::OutMsg;
use toner::tlb::Same;
use toner::tlb::hashmap::aug::HashmapAugE;
use toner_tlb_macros::CellDeserialize;

/// ```tlb
/// _ (HashmapAugE 256 OutMsg CurrencyCollection) = OutMsgDescr;
/// ```
/// #TODO[akostylev0]: use std Hashmap
#[derive(Debug, Clone, CellDeserialize)]
pub struct OutMsgDescr(
    #[tlb(cell, as = "HashmapAugE<Same, Same>", args = "(256, (), ())")]
    pub  HashmapAugE<OutMsg, CurrencyCollection>,
);
