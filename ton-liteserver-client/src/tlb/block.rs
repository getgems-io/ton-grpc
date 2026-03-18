use crate::tlb::block_info::BlockInfo;
use crate::tlb::merkle_update::MerkleUpdate;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::{Cell, Ref};

/// ```tlb
/// block#11ef55aa global_id:int32
///   info:^BlockInfo value_flow:^ValueFlow
///   state_update:^(MERKLE_UPDATE ShardState)
///   extra:^BlockExtra = Block;
/// ```
#[derive(Debug)]
pub struct Block {
    pub global_id: i32,
    pub info: BlockInfo,
    pub value_flow: Cell,
    pub state_update: MerkleUpdate<Cell>,
    pub extra: Cell,
}

impl<'de> CellDeserialize<'de> for Block {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u32 = parser.unpack(())?;
        if tag != 0x11ef55aa {
            return Err(toner::tlb::Error::custom(format!(
                "invalid Block tag: 0x{:08x}",
                tag
            )));
        }

        let global_id = parser.unpack(())?;
        let info = parser.parse_as::<_, Ref>(())?;
        let value_flow = parser.parse_as::<_, Ref>(())?;
        let state_update = parser.parse_as::<_, Ref>(())?;
        let extra = parser.parse_as::<_, Ref>(())?;

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
