use crate::hashmap::{FromBitReader, Slice};

#[derive(Debug)]
pub enum FutureSplitMerge {
    None, // fsm_none$0 = FutureSplitMerge;
    Split { split_utime: u32, interval: u32 }, // fsm_split$10 split_utime:uint32 interval:uint32 = FutureSplitMerge;
    Merge { merge_utime: u32, interval: u32 }, // fsm_merge$11 merge_utime:uint32 interval:uint32 = FutureSplitMerge;
}

impl FromBitReader for FutureSplitMerge {
    fn from_bit_reader(input: &mut Slice) -> Result<Self, crate::hashmap::Error> {
        let bit = input.read_bit()?;
        if bit {
            let bit = input.read_bit()?;
            let utime = input.read_bits(32)? as u32;
            let interval = input.read_bits(32)? as u32;

            if bit {
                Ok(FutureSplitMerge::Merge { merge_utime: utime, interval })
            } else {
                Ok(FutureSplitMerge::Split { split_utime: utime, interval })
            }
        } else {
            Ok(FutureSplitMerge::None)
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

#[derive(Debug)]
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
    fn from_bit_reader(input: &mut Slice) -> Result<Self, crate::hashmap::Error> {
        let tag = input.read_bits(4)? as u8;

        println!("input: {:?}", input);

        let seq_no = input.read_bits(32)? as u32;
        let reg_mc_seqno = input.read_bits(32)? as u32;
        let start_lt = input.read_bits(64)? as u64;
        let end_lt = input.read_bits(64)? as u64;
        let root_hash = <[u8; 32]>::from_bit_reader(input)?;
        let file_hash = <[u8; 32]>::from_bit_reader(input)?;
        let flags = input.read_bits(8)? as u8;;
        let next_catchain_seqno = input.read_bits(32)? as u32;
        let next_validator_shard = input.read_bits(64)? as u64;
        let min_ref_mc_seqno = input.read_bits(32)? as u32;
        let gen_utime = input.read_bits(32)? as u32;
        let split_merge_at = FutureSplitMerge::from_bit_reader(input)?;

        Ok(ShardDescr {
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
        })
    }
}

impl FromBitReader for [u8; 32] {
    fn from_bit_reader(input: &mut Slice) -> Result<Self, crate::hashmap::Error> {
        let mut output = [0_u8; 32];

        for i in 0..32 {
            output[i] = input.read_bits(8)? as u8;
        }

        Ok(output)
    }
}
