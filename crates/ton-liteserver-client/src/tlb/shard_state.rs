use crate::tlb::shard_ident::ShardIdent;
use toner::tlb::bits::de::BitReaderExt;
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
///      total_balance:CurrencyCollection
///      total_validator_fees:CurrencyCollection
///      libraries:(HashmapE 256 LibDescr)
///      master_ref:(Maybe BlkMasterInfo) ]
///   custom:(Maybe ^McStateExtra)
///   = ShardStateUnsplit;
/// ```
#[derive(Debug, Clone)]
pub struct ShardStateUnsplit {
    pub global_id: i32,
    pub shard_id: ShardIdent,
    pub seq_no: u32,
    pub vert_seq_no: u32,
    pub gen_utime: u32,
    pub gen_lt: u64,
    pub min_ref_mc_seqno: u32,
    // TODO[akostylev0]: typed struct for OutMsgQueueInfo
    pub out_msg_queue_info: Cell,
    pub before_split: bool,
    // TODO[akostylev0]: typed struct for ShardAccounts (HashmapAugE 256 ShardAccount DepthBalanceInfo)
    pub accounts: Cell,
    // TODO[akostylev0]: typed struct for the inline tuple
    //   (overload_history, underload_history, total_balance, total_validator_fees, libraries, master_ref)
    pub stats: Cell,
    // TODO[akostylev0]: typed struct for McStateExtra
    pub custom: Option<Cell>,
}

impl<'de> CellDeserialize<'de> for ShardStateUnsplit {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u32 = parser.unpack(())?;
        if tag != 0x9023afe2 {
            return Err(Error::custom(format!(
                "invalid ShardStateUnsplit tag: 0x{:08x}",
                tag
            )));
        }

        let global_id = parser.unpack(())?;
        let shard_id = parser.unpack(())?;
        let seq_no = parser.unpack(())?;
        let vert_seq_no = parser.unpack(())?;
        let gen_utime = parser.unpack(())?;
        let gen_lt = parser.unpack(())?;
        let min_ref_mc_seqno = parser.unpack(())?;
        let out_msg_queue_info = parser.parse_as::<Cell, Ref>(())?;
        let before_split = parser.unpack(())?;
        let accounts = parser.parse_as::<Cell, Ref>(())?;
        let stats = parser.parse_as::<Cell, Ref>(())?;
        let custom: Option<Cell> = parser.parse_as::<_, Option<Ref>>(())?;

        Ok(Self {
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
            stats,
            custom,
        })
    }
}
