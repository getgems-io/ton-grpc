use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::in_msg::InMsg;
use num_bigint::BigUint;
use toner::tlb::Same;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::hashmap::aug::HashmapAugE;
use toner::ton::currency::Grams;
use toner_tlb_macros::CellDeserialize;

/// ```tlb
/// _ (HashmapAugE 256 InMsg ImportFees) = InMsgDescr;
/// ```
/// #TODO[akostylev0]: use std Hashmap
#[derive(Debug, Clone)]
pub struct InMsgDescr(pub HashmapAugE<InMsg, ImportFees>);

impl<'de> CellDeserialize<'de> for InMsgDescr {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let inner = parser.parse_as::<_, HashmapAugE<Same, Same>>((256, (), ()))?;

        parser.ensure_empty()?;

        Ok(Self(inner))
    }
}

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
