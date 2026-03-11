use crate::tlb::block_info::BlockInfo;
use crate::tlb::pruned_branch::PrunedBranch;
use toner::tlb::bits::{de::BitReaderExt, NBits};
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::{ParseFully, Ref};

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
    pub value_flow: PrunedBranch,
    pub state_update: PrunedBranch,
    pub extra: PrunedBranch,
}

impl<'de> CellDeserialize<'de> for BlockHeader {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u32 = parser.unpack_as::<_, NBits<32>>(())?;
        if tag != 0x11ef55aa {
            unimplemented!("actual tag is {:x}", tag)
        };

        let global_id = parser.unpack(())?;
        let info = parser.parse_as::<BlockInfo, Ref<ParseFully>>(())?;
        let value_flow = parser.parse_as::<PrunedBranch, Ref<ParseFully>>(())?;
        let state_update = parser.parse_as::<PrunedBranch, Ref<ParseFully>>(())?;
        let extra = parser.parse_as::<PrunedBranch, Ref<ParseFully>>(())?;
        parser.ensure_empty()?;

        Ok(Self {
            global_id,
            info,
            value_flow,
            state_update,
            extra,
        })
    }
}
