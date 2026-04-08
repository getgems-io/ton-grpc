use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::in_msg::InMsg;
use num_bigint::BigUint;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::hashmap::aug::HashmapAugE;
use toner::tlb::{Context, Same};
use toner::ton::currency::Grams;

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportFees {
    fees_collected: BigUint,
    value_imported: CurrencyCollection,
}

impl<'de> CellDeserialize<'de> for ImportFees {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let fees_collected = parser.unpack_as::<_, Grams>(()).context("fees_collected")?;
        let value_imported = parser.parse(()).context("value_imported")?;

        Ok(ImportFees {
            fees_collected,
            value_imported,
        })
    }
}
