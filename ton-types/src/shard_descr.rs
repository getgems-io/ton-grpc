use nom::IResult;
use crate::deserializer::BitInput;
use crate::hashmap::FromBitReader;

pub enum FutureSplitMerge {
    None, // fsm_none$0 = FutureSplitMerge;
    Split { split_utime: u32, interval: u32 }, // fsm_split$10 split_utime:uint32 interval:uint32 = FutureSplitMerge;
    Merge { merge_utime: u32, interval: u32 }, // fsm_merge$11 merge_utime:uint32 interval:uint32 = FutureSplitMerge;
}

impl FromBitReader for FutureSplitMerge {
    fn from_bit_reader(input: BitInput) -> IResult<BitInput, Self> {
        let (input, bit) = nom::bits::complete::bool(input)?;
        if bit {
            let (input, bit) = nom::bits::complete::bool(input)?;
            let (input, utime) = nom::bits::complete::take(32_usize)(input)?;
            let (input, interval) = nom::bits::complete::take(32_usize)(input)?;
            if bit {
                Ok((input, FutureSplitMerge::Merge { merge_utime: utime, interval }))
            } else {
                Ok((input, FutureSplitMerge::Split { split_utime: utime, interval }))
            }
        } else {
            Ok((input, FutureSplitMerge::None))
        }
    }
}

/**
shard_descr#b seq_no:uint32 reg_mc_seqno:uint32
  start_lt:uint64 end_lt:uint64
  root_hash:bits256 file_hash:bits256
  before_split:Bool before_merge:Bool
  want_split:Bool want_merge:Bool
  nx_cc_updated:Bool flags:(## 3) { flags = 0 }
  next_catchain_seqno:uint32 next_validator_shard:uint64
  min_ref_mc_seqno:uint32 gen_utime:uint32
  split_merge_at:FutureSplitMerge
  fees_collected:CurrencyCollection
  funds_created:CurrencyCollection = ShardDescr;

shard_descr_new#a seq_no:uint32 reg_mc_seqno:uint32
  start_lt:uint64 end_lt:uint64
  root_hash:bits256 file_hash:bits256
  before_split:Bool before_merge:Bool
  want_split:Bool want_merge:Bool
  nx_cc_updated:Bool flags:(## 3) { flags = 0 }
  next_catchain_seqno:uint32 next_validator_shard:uint64
  min_ref_mc_seqno:uint32 gen_utime:uint32
  split_merge_at:FutureSplitMerge
  ^[ fees_collected:CurrencyCollection
     funds_created:CurrencyCollection ] = ShardDescr;

var_uint$_ {n:#} len:(#< n) value:(uint (len * 8))
         = VarUInteger n;
var_int$_ {n:#} len:(#< n) value:(int (len * 8))
        = VarInteger n;
nanograms$_ amount:(VarUInteger 16) = Grams;

_ grams:Grams = Coins;

//
extra_currencies$_ dict:(HashmapE 32 (VarUInteger 32))
                 = ExtraCurrencyCollection;
currencies$_ grams:Grams other:ExtraCurrencyCollection
           = CurrencyCollection;
 **/

pub struct ShardDescr {
    seq_no: u32,
    reg_mc_seqno: u32,
    start_lt: u64,
    end_lt: u64,
    root_hash: [u8; 32],
    file_hash: [u8; 32],
    flags: u8,
    next_catchain_seqno: u32,
    next_validator_shard: u64,
    min_ref_mc_seqno: u32,
    gen_utime: u32,
    split_merge_at: FutureSplitMerge,
    // pub fees_collected: CurrencyCollection,
    // pub funds_created: CurrencyCollection,
}

impl FromBitReader for ShardDescr {
    fn from_bit_reader(input: BitInput) -> IResult<BitInput, Self> {
        let (input, seq_no): (_, u32) = nom::bits::complete::take(32_u32)(input)?;
        let (input, reg_mc_seqno): (_, u32) = nom::bits::complete::take(32_u32)(input)?;
        let (input, start_lt): (_, u64) = nom::bits::complete::take(64_u64)(input)?;
        let (input, end_lt): (_, u64) = nom::bits::complete::take(64_u64)(input)?;
        let (input, root_hash) = <[u8; 32]>::from_bit_reader(input)?;
        let (input, file_hash) = <[u8; 32]>::from_bit_reader(input)?;
        let (input, flags): (_, u8) = nom::bits::complete::take(8_u8)(input)?;
        let (input, next_catchain_seqno): (_, u32) = nom::bits::complete::take(32_u32)(input)?;
        let (input, next_validator_shard): (_, u64) = nom::bits::complete::take(64_u64)(input)?;
        let (input, min_ref_mc_seqno): (_, u32) = nom::bits::complete::take(32_u32)(input)?;
        let (input, gen_utime): (_, u32) = nom::bits::complete::take(32_u32)(input)?;
        let (input, split_merge_at): (_, FutureSplitMerge) = FutureSplitMerge::from_bit_reader(input)?;

        Ok((input, ShardDescr {
            seq_no,
            reg_mc_seqno,
            start_lt,
            end_lt,
            root_hash,
            file_hash,
            flags,
            next_catchain_seqno,
            next_validator_shard,
            min_ref_mc_seqno,
            gen_utime,
            split_merge_at,
        }))
    }
}

impl FromBitReader for [u8; 32] {
    fn from_bit_reader(input: BitInput) -> IResult<BitInput, Self> {
        let mut input = input;
        let mut output = [0_u8; 32];

        for i in 0..32 {
            (input, output[i]) = nom::bits::complete::take(8_usize)(input)?;
        }

        Ok((input, output))
    }
}
