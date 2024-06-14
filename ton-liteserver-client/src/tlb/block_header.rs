use crate::tlb::block_info::BlockInfo;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::bits::r#as::NBits;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::r#as::{ParseFully, Ref};
use toner::tlb::Cell;

/// ```tlb
/// block#11ef55aa
/// global_id:int32
/// info:^BlockInfo
/// value_flow:^ValueFlow - PRUNED
/// state_update:^(MERKLE_UPDATE ShardState) - PRUNED
/// extra:^BlockExtra - PRUNED
/// = Block;
/// ```
#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub global_id: i32,
    pub info: BlockInfo,
}

impl<'de> CellDeserialize<'de> for BlockHeader {
    fn parse(parser: &mut CellParser<'de>) -> Result<Self, CellParserError<'de>> {
        let tag: u32 = parser.unpack_as::<_, NBits<32>>()?;
        if tag != 0x11ef55aa {
            unimplemented!()
        };

        let global_id = parser.unpack()?;
        let info = parser.parse_as::<BlockInfo, Ref<ParseFully>>()?;

        // Pruned Branches
        let _: [Cell; 3] = parser.parse()?;

        Ok(Self { global_id, info })
    }
}
