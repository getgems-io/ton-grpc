use toner_tlb_macros::BitUnpack;

/// ```tlb
/// storage_extra_none$000 = StorageExtraInfo;
/// storage_extra_info$001 dict_hash:uint256 = StorageExtraInfo;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, BitUnpack)]
pub enum StorageExtraInfo {
    #[tlb(tag = "0b000")]
    None,
    #[tlb(tag = "0b001")]
    Info { dict_hash: [u8; 32] },
}

#[cfg(test)]
mod tests {
    use super::*;
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::bitvec::view::BitView;
    use toner::tlb::bits::bitvec::{bits, bitvec};
    use toner::tlb::bits::de::unpack_fully;

    #[test]
    fn unpack_none() {
        let bits = bits![u8, Msb0; 0, 0, 0];

        let actual: StorageExtraInfo = unpack_fully(bits, ()).unwrap();

        assert_eq!(actual, StorageExtraInfo::None);
    }

    #[test]
    fn unpack_info() {
        let mut bits = bitvec![u8, Msb0; 0, 0, 1];
        bits.extend([1u8; 32].view_bits::<Msb0>());

        let actual: StorageExtraInfo = unpack_fully(&bits, ()).unwrap();

        assert_eq!(
            actual,
            StorageExtraInfo::Info {
                dict_hash: [1u8; 32]
            }
        );
    }
}
