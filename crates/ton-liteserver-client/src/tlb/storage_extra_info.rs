use toner::tlb::Error;
use toner::tlb::bits::NBits;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};

/// ```tlb
/// storage_extra_none$000 = StorageExtraInfo;
/// storage_extra_info$001 dict_hash:uint256 = StorageExtraInfo;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageExtraInfo {
    None,
    Info { dict_hash: [u8; 32] },
}

impl<'de> BitUnpack<'de> for StorageExtraInfo {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let tag: u8 = reader.unpack_as::<_, NBits<3>>(())?;
        match tag {
            0b000 => Ok(StorageExtraInfo::None),
            0b001 => Ok(StorageExtraInfo::Info {
                dict_hash: reader.unpack(())?,
            }),
            _ => Err(R::Error::custom("Invalid StorageExtraInfo tag")),
        }
    }
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

        let actual: StorageExtraInfo = unpack_fully(&bits, ()).unwrap();

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
