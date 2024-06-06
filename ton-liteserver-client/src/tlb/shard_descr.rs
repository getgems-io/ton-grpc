use toner::tlb::bits::de::{BitReader, BitReaderExt};
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::r#as::{ParseFully, Ref};
use toner::tlb::ton::currency::CurrencyCollection;
use adnl_tcp::types::Int256;
use crate::tlb::future_split_merge::FutureSplitMerge;

/// ```tlb
/// shard_descr_new#a
/// seq_no:uint32 reg_mc_seqno:uint32
/// start_lt:uint64 end_lt:uint64
/// root_hash:bits256 file_hash:bits256
/// before_split:Bool before_merge:Bool
/// want_split:Bool want_merge:Bool
/// nx_cc_updated:Bool flags:(## 3) { flags = 0 }
/// next_catchain_seqno:uint32 next_validator_shard:uint64
/// min_ref_mc_seqno:uint32 gen_utime:uint32
/// split_merge_at:FutureSplitMerge
/// ^[ fees_collected:CurrencyCollection funds_created:CurrencyCollection ] = ShardDescr;
///
/// shard_descr#b
/// seq_no:uint32 reg_mc_seqno:uint32
/// start_lt:uint64 end_lt:uint64
/// root_hash:bits256 file_hash:bits256
/// before_split:Bool before_merge:Bool
/// want_split:Bool want_merge:Bool
/// nx_cc_updated:Bool flags:(## 3) { flags = 0 }
/// next_catchain_seqno:uint32 next_validator_shard:uint64
/// min_ref_mc_seqno:uint32 gen_utime:uint32
/// split_merge_at:FutureSplitMerge
/// fees_collected:CurrencyCollection funds_created:CurrencyCollection = ShardDescr;
/// ```
pub struct ShardDescr {
    pub seq_no: u32,
    pub reg_mc_seqno: u32,
    pub start_lt: u64,
    pub end_lt: u64,
    pub root_hash: Int256,
    pub file_hash: Int256,
    pub before_split: bool,
    pub before_merge: bool,
    pub want_split: bool,
    pub want_merge: bool,
    pub nx_cc_updated: bool,
    pub flags: u32,
    pub next_catchain_seqno: u32,
    pub next_validator_shard: u64,
    pub min_ref_mc_seqno: u32,
    pub gen_utime: u32,
    pub split_merge_at: FutureSplitMerge,
    pub fees_collected: CurrencyCollection,
    pub funds_created: CurrencyCollection,
}

impl<'de> CellDeserialize<'de> for ShardDescr {
    fn parse(parser: &mut CellParser<'de>) -> Result<Self, CellParserError<'de>> {
        let tag: u8 = parser.unpack()?;

        let seq_no = parser.unpack()?;
        let reg_mc_seqno = parser.unpack()?;
        let start_lt = parser.unpack()?;
        let end_lt = parser.unpack()?;
        let root_hash = parser.unpack()?;
        let file_hash = parser.unpack()?;
        let before_split = parser.read_bit()?;
        let before_merge = parser.read_bit()?;
        let want_split = parser.read_bit()?;
        let want_merge = parser.read_bit()?;
        let nx_cc_updated = parser.read_bit()?;
        let flags = parser.unpack()?;
        let next_catchain_seqno = parser.unpack()?;
        let next_validator_shard = parser.unpack()?;
        let min_ref_mc_seqno = parser.unpack()?;
        let gen_utime = parser.unpack()?;
        let split_merge_at = parser.parse()?;
        let (fees_collected, funds_created) = match tag {
            0xa => parser.parse()?,
            0xb => parser.parse_as::<_, Ref<ParseFully>>()?,
            _ => unreachable!()
        };

        Ok(Self {
            seq_no,
            reg_mc_seqno,
            start_lt,
            end_lt,
            root_hash,
            file_hash,
            before_split,
            before_merge,
            want_split,
            want_merge,
            nx_cc_updated,
            flags,
            next_catchain_seqno,
            next_validator_shard,
            min_ref_mc_seqno,
            gen_utime,
            split_merge_at,
            fees_collected,
            funds_created,
        })
    }
}
