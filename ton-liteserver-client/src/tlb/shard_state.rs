use crate::tlb::shard_ident::ShardIdent;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::bits::NBits;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::{Cell, Error, Ref};

/// ```tlb
/// shard_state#9023afe2 global_id:int32
///   shard_id:ShardIdent
///   seq_no:uint32 vert_seq_no:#
///   gen_utime:uint32 gen_lt:uint64
///   min_ref_mc_seqno:uint32
///   out_msg_queue_info:^OutMsgQueueInfo
///   before_split:(## 1)
///   accounts:^ShardAccounts
///   ^[ overload_history:uint64 underload_history:uint64
///   total_balance:CurrencyCollection
///   total_validator_fees:CurrencyCollection
///   libraries:(HashmapE 256 LibDescr)
///   master_ref:(Maybe BlkMasterInfo) ]
///   custom:(Maybe ^McStateExtra)
///   = ShardStateUnsplit;
/// ```
#[derive(Debug)]
pub struct ShardStateUnsplit {
    pub global_id: i32,
    pub shard_id: ShardIdent,
    pub seq_no: u32,
    pub vert_seq_no: u32,
    pub gen_utime: u32,
    pub gen_lt: u64,
    pub min_ref_mc_seqno: u32,
    pub out_msg_queue_info: Cell,
    pub before_split: bool,
    pub accounts: Cell,
    pub info: Cell,
    pub custom: Option<Cell>,
}

fn parse_shard_state_unsplit<'de>(
    parser: &mut CellParser<'de>,
) -> Result<ShardStateUnsplit, CellParserError<'de>> {
    let global_id: i32 = parser.unpack(())?;
    let shard_id: ShardIdent = parser.unpack(())?;
    let seq_no: u32 = parser.unpack(())?;
    let vert_seq_no: u32 = parser.unpack(())?;
    let gen_utime: u32 = parser.unpack(())?;
    let gen_lt: u64 = parser.unpack(())?;
    let min_ref_mc_seqno: u32 = parser.unpack(())?;
    let out_msg_queue_info = parser.parse_as::<Cell, Ref>(())?;
    let before_split: u8 = parser.unpack_as::<_, NBits<1>>(())?;
    let before_split = before_split != 0;
    let accounts = parser.parse_as::<Cell, Ref>(())?;
    let info = parser.parse_as::<Cell, Ref>(())?;
    let custom: Option<Cell> = parser.parse_as::<_, Option<Ref>>(())?;

    Ok(ShardStateUnsplit {
        global_id,
        shard_id,
        seq_no,
        vert_seq_no,
        gen_utime,
        gen_lt,
        min_ref_mc_seqno,
        out_msg_queue_info,
        before_split,
        accounts,
        info,
        custom,
    })
}

impl<'de> CellDeserialize<'de> for ShardStateUnsplit {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u32 = parser.unpack_as::<_, NBits<32>>(())?;
        if tag != 0x9023afe2 {
            return Err(Error::custom(format!(
                "invalid ShardStateUnsplit tag: 0x{:08x}",
                tag
            )));
        }

        parse_shard_state_unsplit(parser)
    }
}

/// ```tlb
/// _ ShardStateUnsplit = ShardState;
/// split_state#5f327da5 left:^ShardStateUnsplit right:^ShardStateUnsplit = ShardState;
/// ```
#[derive(Debug)]
pub enum ShardState {
    Unsplit(ShardStateUnsplit),
    Split {
        left: ShardStateUnsplit,
        right: ShardStateUnsplit,
    },
}

impl<'de> CellDeserialize<'de> for ShardState {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u32 = parser.unpack_as::<_, NBits<32>>(())?;

        match tag {
            0x9023afe2 => {
                let unsplit = parse_shard_state_unsplit(parser)?;

                Ok(Self::Unsplit(unsplit))
            }
            0x5f327da5 => {
                let left = parser.parse_as::<_, Ref>(())?;
                let right = parser.parse_as::<_, Ref>(())?;

                Ok(Self::Split { left, right })
            }
            _ => Err(Error::custom(format!(
                "invalid ShardState tag: 0x{:08x}",
                tag
            ))),
        }
    }
}
