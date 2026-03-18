use std::marker::PhantomData;

use adnl_tcp::types::Int256;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};
use toner::tlb::bits::NBits;

/// ```tlb
/// update_hashes#72 {X:Type} old_hash:bits256 new_hash:bits256
///   = HASH_UPDATE X;
/// ```
///
/// `X` is a phantom type parameter — it is not used in the binary layout,
/// but indicates what type of data the hash update refers to (e.g., `HASH_UPDATE Account`).
#[derive(Debug, Clone)]
pub struct HashUpdate<X> {
    pub old_hash: Int256,
    pub new_hash: Int256,
    _phantom: PhantomData<X>,
}

impl<'de, X> BitUnpack<'de> for HashUpdate<X> {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let tag: u8 = reader.unpack_as::<_, NBits<8>>(())?;
        if tag != 0x72 {
            unreachable!("invalid HASH_UPDATE tag: 0x{:02x}, expected 0x72", tag);
        }

        let old_hash: Int256 = reader.unpack(())?;
        let new_hash: Int256 = reader.unpack(())?;

        Ok(Self {
            old_hash,
            new_hash,
            _phantom: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::bitvec::vec::BitVec;
    use toner::tlb::bits::de::unpack_fully;

    struct Account;

    #[test]
    fn unpack_hash_update() {
        let mut data = vec![0x72];
        data.extend_from_slice(&[0xAA; 32]);
        data.extend_from_slice(&[0xBB; 32]);

        let bits: BitVec<u8, Msb0> = BitVec::from_vec(data);
        let result: HashUpdate<Account> = unpack_fully(&bits, ()).unwrap();

        assert_eq!(result.old_hash, [0xAA; 32]);
        assert_eq!(result.new_hash, [0xBB; 32]);
    }

}
