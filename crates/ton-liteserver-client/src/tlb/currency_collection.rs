use crate::tlb::extra_currency_collection::ExtraCurrencyCollection;
use num_bigint::BigUint;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::ton::currency::Grams;

/// ```tlb
/// currencies$_ grams:Grams other:ExtraCurrencyCollection
///            = CurrencyCollection;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CurrencyCollection {
    pub grams: BigUint,
    pub other: ExtraCurrencyCollection,
}

impl<'de> CellDeserialize<'de> for CurrencyCollection {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        Ok(Self {
            grams: parser.unpack_as::<_, Grams>(())?,
            other: parser.parse(())?,
        })
    }
}
