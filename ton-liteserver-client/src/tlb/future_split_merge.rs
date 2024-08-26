use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};

/// ```tlb
/// fsm_none$0 = FutureSplitMerge;
/// fsm_split$10 split_utime:uint32 interval:uint32 = FutureSplitMerge;
/// fsm_merge$11 merge_utime:uint32 interval:uint32 = FutureSplitMerge;
/// ```
#[derive(Debug, Clone)]
pub enum FutureSplitMerge {
    None,                                      // fsm_none$0
    Split { split_utime: u32, interval: u32 }, // fsm_split$10
    Merge { merge_utime: u32, interval: u32 }, // fsm_merge$11
}

impl BitUnpack for FutureSplitMerge {
    fn unpack<R>(mut reader: R) -> Result<Self, R::Error>
    where
        R: BitReader,
    {
        if !reader.unpack::<bool>()? {
            return Ok(FutureSplitMerge::None);
        }

        if !reader.unpack::<bool>()? {
            let split_utime = reader.unpack()?;
            let interval = reader.unpack()?;

            Ok(FutureSplitMerge::Split {
                split_utime,
                interval,
            })
        } else {
            let merge_utime = reader.unpack()?;
            let interval = reader.unpack()?;

            Ok(FutureSplitMerge::Merge {
                merge_utime,
                interval,
            })
        }
    }
}
