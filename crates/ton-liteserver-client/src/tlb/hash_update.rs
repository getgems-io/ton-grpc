use std::marker::PhantomData;
use toner::tlb::bits::NBits;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};

/// ```tlb
/// update_hashes#72 {X:Type} old_hash:bits256 new_hash:bits256
///   = HASH_UPDATE X;
/// ```
#[derive(Debug, Clone, Copy, Eq)]
pub struct HashUpdate<X> {
    old_hash: [u8; 32],
    new_hash: [u8; 32],
    _phantom: PhantomData<X>,
}

impl<X> PartialEq for HashUpdate<X> {
    fn eq(&self, other: &Self) -> bool {
        self.old_hash == other.old_hash && self.new_hash == other.new_hash
    }
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

        let old_hash = reader.unpack(())?;
        let new_hash = reader.unpack(())?;

        Ok(Self {
            old_hash,
            new_hash,
            _phantom: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::tlb::block::Block;
    use crate::tlb::hash_update::HashUpdate;
    use toner::tlb::bits::bitvec::bitvec;
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::bitvec::view::BitView;
    use toner::tlb::bits::de::unpack_fully;

    #[test]
    fn unpack() {
        let mut bits = bitvec![u8, Msb0;];
        bits.extend(0x72u8.view_bits::<Msb0>());
        bits.extend([1u8; 32].view_bits::<Msb0>());
        bits.extend([2u8; 32].view_bits::<Msb0>());

        let result: HashUpdate<Block> = unpack_fully(&bits, ()).unwrap();

        assert_eq!(
            result,
            HashUpdate {
                old_hash: [1u8; 32],
                new_hash: [2u8; 32],
                _phantom: Default::default(),
            }
        );
    }
}
