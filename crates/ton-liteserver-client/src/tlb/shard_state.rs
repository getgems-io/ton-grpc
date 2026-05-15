use crate::tlb::shard_ident::ShardIdent;
use toner::tlb::Cell;
use toner::tlb::Ref;
use toner_tlb_macros::CellDeserialize;

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
#[derive(Debug, Clone, CellDeserialize)]
#[tlb(tag = "0x9023afe2")]
pub struct ShardStateUnsplit {
    #[tlb(unpack)]
    pub global_id: i32,
    #[tlb(unpack)]
    pub shard_id: ShardIdent,
    #[tlb(unpack)]
    pub seq_no: u32,
    #[tlb(unpack)]
    pub vert_seq_no: u32,
    #[tlb(unpack)]
    pub gen_utime: u32,
    #[tlb(unpack)]
    pub gen_lt: u64,
    #[tlb(unpack)]
    pub min_ref_mc_seqno: u32,
    // TODO[akostylev0]: typed struct for OutMsgQueueInfo
    #[tlb(parse_as = "Ref")]
    pub out_msg_queue_info: Cell,
    #[tlb(unpack)]
    pub before_split: bool,
    // TODO[akostylev0]: typed struct for ShardAccounts (HashmapAugE 256 ShardAccount DepthBalanceInfo)
    #[tlb(parse_as = "Ref")]
    pub accounts: Cell,
    // TODO[akostylev0]: typed struct for the inline tuple
    //   (overload_history, underload_history, total_balance, total_validator_fees, libraries, master_ref)
    #[tlb(parse_as = "Ref")]
    pub stats: Cell,
    // TODO[akostylev0]: typed struct for McStateExtra
    #[tlb(parse_as = "Option<Ref>")]
    pub custom: Option<Cell>,
}
