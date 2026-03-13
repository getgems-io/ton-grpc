use crate::tlb::mc_block_extra::McBlockExtra;
use adnl_tcp::types::Int256;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::bits::NBits;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::{Cell, Error, Ref};

/// ```tlb
/// block_extra in_msg_descr:^InMsgDescr
///   out_msg_descr:^OutMsgDescr
///   account_blocks:^ShardAccountBlocks
///   rand_seed:bits256
///   created_by:bits256
///   custom:(Maybe ^McBlockExtra) = BlockExtra;
/// ```
#[derive(Debug)]
pub struct BlockExtra {
    pub in_msg_descr: Cell,
    pub out_msg_descr: Cell,
    pub account_blocks: Cell,
    pub rand_seed: Int256,
    pub created_by: Int256,
    pub custom: Option<McBlockExtra>,
}

impl<'de> CellDeserialize<'de> for BlockExtra {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u32 = parser.unpack_as::<_, NBits<32>>(())?;
        if tag != 0x4a33f6fd {
            return Err(Error::custom(format!(
                "invalid BlockExtra tag: 0x{:08x}",
                tag
            )));
        }

        let in_msg_descr = parser.parse_as::<Cell, Ref>(())?;
        let out_msg_descr = parser.parse_as::<Cell, Ref>(())?;
        let account_blocks = parser.parse_as::<Cell, Ref>(())?;
        let rand_seed = parser.unpack(())?;
        let created_by = parser.unpack(())?;
        let custom: Option<McBlockExtra> = parser.parse_as::<_, Option<Ref>>(())?;
        parser.ensure_empty()?;

        Ok(Self {
            in_msg_descr,
            out_msg_descr,
            account_blocks,
            rand_seed,
            created_by,
            custom,
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
    fn test_block_extra_parse_ok() {
        let data = hex::decode(BLOCK_HEX).unwrap();
        let boc: BoC = unpack_bytes(&data, ()).unwrap();
        let root = boc.single_root().unwrap();

        let block: Block = root.parse_fully(()).unwrap();

        assert!(block.extra.custom.is_some());
        assert!(!block.extra.custom.unwrap().shard_hashes.is_empty());
    }
}
