use crate::tlb::blk_master_info::BlkMasterInfo;
use crate::tlb::blk_prev_info::BlkPrevInfo;
use crate::tlb::global_version::GlobalVersion;
use crate::tlb::shard_ident::ShardIdent;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::bits::r#as::NBits;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::r#as::{Data, Ref, Same};

/// ```tlb
///  block_info#9bc7a987
///  version:uint32
///  not_master:(## 1)
///  after_merge:(## 1) before_split:(## 1)
///  after_split:(## 1)
///  want_split:Bool want_merge:Bool
///  key_block:Bool vert_seqno_incr:(## 1)
///  flags:(## 8) { flags <= 1 }
///  seq_no:#
///  vert_seq_no:# { vert_seq_no >= vert_seqno_incr }
///  { prev_seq_no:# } { ~prev_seq_no + 1 = seq_no }
///  shard:ShardIdent gen_utime:uint32
///  start_lt:uint64 end_lt:uint64
///  gen_validator_list_hash_short:uint32
///  gen_catchain_seqno:uint32
///  min_ref_mc_seqno:uint32
///  prev_key_block_seqno:uint32
///  gen_software:flags.0?GlobalVersion
///  master_ref:not_master?^BlkMasterInfo
///  prev_ref:^(BlkPrevInfo after_merge)
///  prev_vert_ref:vert_seqno_incr?^(BlkPrevInfo 0)
///  = BlockInfo;
/// ```
#[derive(Debug, Clone)]
pub struct BlockInfo {
    pub version: u32,
    pub flags: u16,
    pub seq_no: u32,
    pub vert_seq_no: u32,
    pub shard: ShardIdent,
    pub gen_utime: u32,
    pub start_lt: u64,
    pub end_lt: u64,
    pub gen_validator_list_hash_short: u32,
    pub gen_catchain_seqno: u32,
    pub min_ref_mc_seqno: u32,
    pub prev_key_block_seqno: u32,
    pub gen_software: Option<GlobalVersion>,
    pub master_ref: Option<BlkMasterInfo>,
    pub prev_ref: BlkPrevInfo,
    pub prev_vert_ref: Option<BlkPrevInfo>,
}

impl<'de> CellDeserialize<'de> for BlockInfo {
    fn parse(parser: &mut CellParser<'de>) -> Result<Self, CellParserError<'de>> {
        let tag: u32 = parser.unpack_as::<_, NBits<32>>()?;
        if tag != 0x9bc7a987 {
            unreachable!()
        };

        let version = parser.unpack()?;
        let flags = parser.unpack()?;
        let seq_no = parser.unpack()?;
        let vert_seq_no = parser.unpack()?;
        let shard = parser.unpack()?;
        let gen_utime = parser.unpack()?;
        let start_lt = parser.unpack()?;
        let end_lt = parser.unpack()?;
        let gen_validator_list_hash_short = parser.unpack()?;
        let gen_catchain_seqno = parser.unpack()?;
        let min_ref_mc_seqno = parser.unpack()?;
        let prev_key_block_seqno = parser.unpack()?;

        let gen_software = if flags & (1 << 0) != 0 {
            Some(parser.unpack()?)
        } else {
            None
        };
        let master_ref = if flags & (1 << 15) != 0 {
            Some(parser.parse_as::<_, Ref<Data<Same>>>()?)
        } else {
            None
        };
        let prev_ref = parser.parse_as_with::<_, Ref<Same>>(flags & (1 << 14) != 0)?;
        let prev_vert_ref = if flags & (1 << 8) != 0 {
            Some(parser.parse_as_with::<_, Ref<Same>>(false)?)
        } else {
            None
        };

        Ok(Self {
            version,
            flags,
            seq_no,
            vert_seq_no,
            shard,
            gen_utime,
            start_lt,
            end_lt,
            gen_validator_list_hash_short,
            gen_catchain_seqno,
            min_ref_mc_seqno,
            prev_key_block_seqno,
            gen_software,
            master_ref,
            prev_ref,
            prev_vert_ref,
        })
    }
}
