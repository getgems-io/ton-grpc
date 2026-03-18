use adnl_tcp::types::Int256;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};
use toner::tlb::bits::NBits;

/// ```tlb
/// storage_extra_none$000 = StorageExtraInfo;
/// storage_extra_info$001 dict_hash:uint256 = StorageExtraInfo;
/// ```
pub enum StorageExtraInfo {
    None,
    Info { dict_hash: Int256 },
}

impl<'de> BitUnpack<'de> for StorageExtraInfo {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let tag: u8 = reader.unpack_as::<_, NBits<3>>(())?;
        match tag {
            0b00 => Ok(StorageExtraInfo::None),
            0b01 => {
                let dict_hash = reader.unpack(())?;
                Ok(StorageExtraInfo::Info { dict_hash })
            }
            _ => unreachable!(),
        }
    }
}
