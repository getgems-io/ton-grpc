use toner_tlb_macros::{BitPack, BitUnpack};

/// ```tlb
/// ext_blk_ref$_
/// end_lt:uint64
/// seq_no:uint32
/// root_hash:bits256
/// file_hash:bits256
///   = ExtBlkRef;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, BitPack, BitUnpack)]
pub struct ExtBlkRef {
    pub end_lt: u64,
    pub seq_no: u32,
    pub root_hash: [u8; 32],
    pub file_hash: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::*;
    use toner::tlb::bits::de::unpack_fully;
    use toner::tlb::bits::ser::pack;

    #[test]
    fn should_roundtrip_ext_blk_ref() {
        let original = ExtBlkRef {
            end_lt: 0x1234_5678_9abc_def0,
            seq_no: 0x0a0b_0c0d,
            root_hash: [0x11; 32],
            file_hash: [0x22; 32],
        };

        let bits = pack(&original, ()).unwrap();
        let decoded: ExtBlkRef = unpack_fully(bits.as_bitslice(), ()).unwrap();

        assert_eq!(decoded, original);
    }
}
