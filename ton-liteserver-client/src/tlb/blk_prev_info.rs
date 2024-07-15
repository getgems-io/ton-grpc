use crate::tlb::ext_blk_ref::ExtBlkRef;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::de::args::CellDeserializeWithArgs;
use toner::tlb::de::{CellParser, CellParserError};
use toner::tlb::r#as::{Data, Ref, Same};

/// ```tlb
/// prev_blk_info$_ prev:ExtBlkRef = BlkPrevInfo 0;
/// prev_blks_info$_ prev1:^ExtBlkRef prev2:^ExtBlkRef = BlkPrevInfo 1;
/// ```
#[derive(Debug, Clone)]
pub enum BlkPrevInfo {
    Ref(ExtBlkRef),
    RefPair(ExtBlkRef, ExtBlkRef),
}

impl<'de> CellDeserializeWithArgs<'de> for BlkPrevInfo {
    type Args = bool;

    fn parse_with(
        parser: &mut CellParser<'de>,
        args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        match args {
            false => Ok(BlkPrevInfo::Ref(parser.unpack()?)),
            true => Ok(BlkPrevInfo::RefPair(
                parser.parse_as::<_, Ref<Data<Same>>>()?,
                parser.parse_as::<_, Ref<Data<Same>>>()?,
            )),
        }
    }
}
