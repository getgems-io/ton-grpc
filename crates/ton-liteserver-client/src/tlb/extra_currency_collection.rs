use num_bigint::BigUint;
use std::collections::HashMap;
use toner::tlb::Data;
use toner::tlb::bits::VarInt;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::hashmap::HashmapE;

/// ```tlb
/// extra_currencies$_ dict:(HashmapE 32 (VarUInteger 32))
///                  = ExtraCurrencyCollection;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExtraCurrencyCollection(pub HashMap<u32, BigUint>);

impl<'de> CellDeserialize<'de> for ExtraCurrencyCollection {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let inner =
            parser.parse_as::<HashMap<_, BigUint>, HashmapE<Data<VarInt<5>>>>((32, (), ()))?;

        Ok(Self(inner))
    }
}
