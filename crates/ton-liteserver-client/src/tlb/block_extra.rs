use crate::tlb::in_msg_descr::InMsgDescr;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::de::{CellParser, CellParserError};
use toner::tlb::{Cell, Context, Error, ParseFully, Ref};
use toner::ton::de::CellDeserialize;

/// ```tlb
/// block_extra in_msg_descr:^InMsgDescr
///   out_msg_descr:^OutMsgDescr
///   account_blocks:^ShardAccountBlocks
///   rand_seed:bits256
///   created_by:bits256
///   custom:(Maybe ^McBlockExtra) = BlockExtra;
/// ```
#[derive(Debug, Clone)]
pub struct BlockExtra {
    pub in_msg_descr: InMsgDescr,
    pub out_msg_descr: Cell,  // TODO[akostylev0]
    pub account_blocks: Cell, // TODO[akostylev0]
    pub rand_seed: [u8; 32],
    pub created_by: [u8; 32],
    pub custom: Option<Cell>, // TODO[akostylev0]
}

impl<'de> CellDeserialize<'de> for BlockExtra {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u32 = parser.unpack(())?;
        if tag != 0x4a33f6fd {
            return Err(Error::custom(format!(
                "invalid BlockExtra tag: 0x{:08x}",
                tag
            )));
        }

        let in_msg_descr = parser
            .parse_as::<_, Ref<ParseFully>>(())
            .context("in_msg_descr")?;
        let out_msg_descr = parser
            .parse_as::<_, Ref<ParseFully>>(())
            .context("out_msg_descr")?;
        let account_blocks = parser
            .parse_as::<_, Ref<ParseFully>>(())
            .context("account_blocks")?;
        let rand_seed = parser.unpack(()).context("rand_seed")?;
        let created_by = parser.unpack(()).context("created_by")?;
        let custom = parser
            .parse_as::<_, Option<Ref<ParseFully>>>(())
            .context("custom")?;

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
