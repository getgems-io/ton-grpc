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

impl ShardIdent {
    /// Computes the canonical shard ID by adding the sentinel bit to the raw prefix.
    ///
    /// In TON, a `ShardId` is encoded as `prefix_bits | sentinel_bit | zeros`.
    /// The sentinel bit position is `63 - shard_pfx_bits`.
    /// Reference: `ton/crypto/block/block-parse.cpp` `ShardIdent::unpack`.
    pub fn shard_id(&self) -> u64 {
        let sentinel = 1u64 << (63 - self.shard_pfx_bits);
        self.shard_prefix | sentinel
    }
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

    #[test]
    fn should_compute_shard_id_for_masterchain() {
        let shard = ShardIdent {
            shard_pfx_bits: 0,
            workchain_id: -1,
            shard_prefix: 0,
        };

        assert_eq!(shard.shard_id(), 0x8000_0000_0000_0000);
        assert_eq!(shard.shard_id() as i64, i64::MIN);
    }

    #[test]
    fn should_compute_shard_id_with_prefix_bits() {
        let shard = ShardIdent {
            shard_pfx_bits: 1,
            workchain_id: 0,
            shard_prefix: 0x8000_0000_0000_0000,
        };

        assert_eq!(shard.shard_id(), 0xC000_0000_0000_0000);
    }
}
