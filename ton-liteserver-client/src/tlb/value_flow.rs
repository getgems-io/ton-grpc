use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::bits::NBits;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::{Error, Ref};
use toner::ton::currency::CurrencyCollection;

/// ```tlb
/// value_flow#b8e48dfb ^[ from_prev_blk:CurrencyCollection
///   to_next_blk:CurrencyCollection
///   imported:CurrencyCollection
///   exported:CurrencyCollection ]
///   fees_collected:CurrencyCollection
///   ^[
///   fees_imported:CurrencyCollection
///   recovered:CurrencyCollection
///   created:CurrencyCollection
///   minted:CurrencyCollection
///   ] = ValueFlow;
///
/// value_flow_v2#3ebf98b7 ^[ from_prev_blk:CurrencyCollection
///   to_next_blk:CurrencyCollection
///   imported:CurrencyCollection
///   exported:CurrencyCollection ]
///   fees_collected:CurrencyCollection
///   burned:CurrencyCollection
///   ^[
///   fees_imported:CurrencyCollection
///   recovered:CurrencyCollection
///   created:CurrencyCollection
///   minted:CurrencyCollection
///   ] = ValueFlow;
/// ```
#[derive(Debug, Clone)]
pub struct ValueFlow {
    pub from_prev_blk: CurrencyCollection,
    pub to_next_blk: CurrencyCollection,
    pub imported: CurrencyCollection,
    pub exported: CurrencyCollection,
    pub fees_collected: CurrencyCollection,
    pub burned: Option<CurrencyCollection>,
    pub fees_imported: CurrencyCollection,
    pub recovered: CurrencyCollection,
    pub created: CurrencyCollection,
    pub minted: CurrencyCollection,
}

impl<'de> CellDeserialize<'de> for ValueFlow {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u32 = parser.unpack_as::<_, NBits<32>>(())?;

        let is_v2 = match tag {
            0xb8e48dfb => false,
            0x3ebf98b7 => true,
            _ => {
                return Err(Error::custom(format!(
                    "invalid ValueFlow tag: 0x{:08x}",
                    tag
                )));
            }
        };

        let (from_prev_blk, to_next_blk, imported, exported): (
            CurrencyCollection,
            CurrencyCollection,
            CurrencyCollection,
            CurrencyCollection,
        ) = parser.parse_as::<_, Ref>(((), (), (), ()))?;

        let fees_collected = parser.parse(())?;

        let burned = if is_v2 { Some(parser.parse(())?) } else { None };

        let (fees_imported, recovered, created, minted): (
            CurrencyCollection,
            CurrencyCollection,
            CurrencyCollection,
            CurrencyCollection,
        ) = parser.parse_as::<_, Ref>(((), (), (), ()))?;

        Ok(Self {
            from_prev_blk,
            to_next_blk,
            imported,
            exported,
            fees_collected,
            burned,
            fees_imported,
            recovered,
            created,
            minted,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::tlb::block::Block;
    use crate::tlb::merkle_update::tests::BLOCK_HEX;
    use toner::tlb::bits::de::unpack_bytes;
    use toner::tlb::BoC;

    #[test]
    fn test_value_flow_parse_ok() {
        let data = hex::decode(BLOCK_HEX).unwrap();
        let boc: BoC = unpack_bytes(&data, ()).unwrap();
        let root = boc.single_root().unwrap();

        let block: Block = root.parse_fully(()).unwrap();

        assert!(block.value_flow.burned.is_some());
    }
}
