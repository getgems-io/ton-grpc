use toner::tlb::bits::NBits;
use toner_tlb_macros::{BitPack, BitUnpack};

/// `tlb
/// shard_ident$00
/// shard_pfx_bits:(#<= 60)
/// workchain_id:int32
/// shard_prefix:uint64 = ShardIdent;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, BitPack, BitUnpack)]
#[tlb(tag = "0b00")]
pub struct ShardIdent {
    #[tlb(bits, as = "NBits<6>")]
    pub shard_pfx_bits: u8,
    pub workchain_id: i32,
    pub shard_prefix: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use toner::tlb::bits::de::unpack_fully;
    use toner::tlb::bits::ser::pack;

    #[test]
    fn should_roundtrip_shard_ident() {
        let original = ShardIdent {
            shard_pfx_bits: 0b010101,
            workchain_id: -1,
            shard_prefix: 0x8000_0000_0000_0000,
        };

        let bits = pack(&original, ()).unwrap();
        let decoded: ShardIdent = unpack_fully(bits.as_bitslice(), ()).unwrap();

        assert_eq!(decoded, original);
    }

    #[test]
    fn should_pack_tag_prefix() {
        let value = ShardIdent {
            shard_pfx_bits: 0,
            workchain_id: 0,
            shard_prefix: 0,
        };

        let bits = pack(&value, ()).unwrap();

        assert_eq!(bits.len(), 2 + 6 + 32 + 64);
        assert!(!bits[0]);
        assert!(!bits[1]);
    }
}
