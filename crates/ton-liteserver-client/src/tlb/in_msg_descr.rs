use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::in_msg::InMsg;
use num_bigint::BigUint;
use toner::tlb::Same;
use toner::tlb::hashmap::aug::HashmapAugE;
use toner::ton::currency::Grams;
use toner_tlb_macros::CellDeserialize;

/// ```tlb
/// _ (HashmapAugE 256 InMsg ImportFees) = InMsgDescr;
/// ```
/// #TODO[akostylev0]: use std Hashmap
#[derive(Debug, Clone, CellDeserialize)]
#[tlb(ensure_empty)]
pub struct InMsgDescr(
    #[tlb(parse_as = "HashmapAugE<Same, Same>", args = "(256, (), ())")]
    pub  HashmapAugE<InMsg, ImportFees>,
);

/// ```tlb
/// import_fees$_ fees_collected:Grams
///   value_imported:CurrencyCollection = ImportFees;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, CellDeserialize)]
pub struct ImportFees {
    #[tlb(unpack_as = "Grams")]
    fees_collected: BigUint,
    value_imported: CurrencyCollection,
}
