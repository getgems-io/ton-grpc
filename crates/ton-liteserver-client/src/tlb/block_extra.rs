use crate::tlb::in_msg_descr::InMsgDescr;
use crate::tlb::out_msg_descr::OutMsgDescr;
use crate::tlb::shard_account_blocks::ShardAccountBlocks;
use toner::tlb::{Cell, ParseFully, Ref};
use toner_tlb_macros::CellDeserialize;

/// ```tlb
/// block_extra in_msg_descr:^InMsgDescr
///   out_msg_descr:^OutMsgDescr
///   account_blocks:^ShardAccountBlocks
///   rand_seed:bits256
///   created_by:bits256
///   custom:(Maybe ^McBlockExtra) = BlockExtra;
/// ```
#[derive(Debug, Clone, CellDeserialize)]
#[tlb(tag = "0x4a33f6fd")]
#[tlb(ensure_empty)]
pub struct BlockExtra {
    #[tlb(parse_as = "Ref<ParseFully>")]
    pub in_msg_descr: InMsgDescr,
    #[tlb(parse_as = "Ref<ParseFully>")]
    pub out_msg_descr: OutMsgDescr,
    #[tlb(parse_as = "Ref<ParseFully>")]
    pub account_blocks: ShardAccountBlocks,
    #[tlb(unpack)]
    pub rand_seed: [u8; 32],
    #[tlb(unpack)]
    pub created_by: [u8; 32],
    #[tlb(parse_as = "Option<Ref<ParseFully>>")]
    pub custom: Option<Cell>, // TODO[akostylev0]
}
