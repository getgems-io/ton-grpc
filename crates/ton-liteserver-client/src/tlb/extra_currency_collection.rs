use num_bigint::BigUint;
use std::collections::HashMap;
use toner::tlb::Data;
use toner::tlb::bits::VarInt;
use toner::tlb::bits::bitvec::field::BitField;
use toner::tlb::bits::bitvec::order::Msb0;
use toner::tlb::bits::bitvec::vec::BitVec;
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
        // TODO[akostylev0]: parse as Key
        let hashmap = parser
            .parse_as::<HashMap<BitVec<u8, Msb0>, BigUint>, HashmapE<Data<VarInt<5>>>>((32, ()))?;

        let inner = hashmap
            .into_iter()
            .map(|(k, v): (BitVec<u8, Msb0>, BigUint)| (k.load_be::<u32>(), v))
            .collect();

        Ok(Self(inner))
    }
}
