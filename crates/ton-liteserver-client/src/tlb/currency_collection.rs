use crate::tlb::extra_currency_collection::ExtraCurrencyCollection;
use num_bigint::BigUint;
use toner::ton::currency::Grams;
use toner_tlb_macros::CellDeserialize;

/// ```tlb
/// currencies$_ grams:Grams other:ExtraCurrencyCollection
///            = CurrencyCollection;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, CellDeserialize)]
pub struct CurrencyCollection {
    #[tlb(unpack_as = "Grams")]
    pub grams: BigUint,
    pub other: ExtraCurrencyCollection,
}
