use toner::tlb::bits::de::{BitReader, BitReaderExt};
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};

/// ```tlb
/// fsm_none$0 = FutureSplitMerge;
/// fsm_split$10 split_utime:uint32 interval:uint32 = FutureSplitMerge;
/// fsm_merge$11 merge_utime:uint32 interval:uint32 = FutureSplitMerge;
/// ```
pub enum FutureSplitMerge {
    None, // fsm_none$0
    Split { split_utime: u32, interval: u32 }, // fsm_split$10
    Merge { merge_utime: u32, interval: u32 }, // fsm_merge$11
}

impl<'de> CellDeserialize<'de> for FutureSplitMerge {
    fn parse(parser: &mut CellParser<'de>) -> Result<Self, CellParserError<'de>> {
        if !parser.read_bit()? {
            return Ok(FutureSplitMerge::None)
        }

        if !parser.read_bit()? {
            let split_utime = parser.unpack()?;
            let interval = parser.unpack()?;

            Ok(FutureSplitMerge::Split { split_utime, interval })
        } else {
            let merge_utime = parser.unpack()?;
            let interval = parser.unpack()?;

            Ok(FutureSplitMerge::Merge { merge_utime, interval })
        }
    }
}
