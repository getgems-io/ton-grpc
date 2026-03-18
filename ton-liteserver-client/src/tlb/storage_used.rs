use num_bigint::BigUint;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};
use toner::tlb::bits::VarInt;

/// ```tlb
/// storage_used$_ cells:(VarUInteger 7) bits:(VarUInteger 7) = StorageUsed;
/// ```
#[derive(Debug, Clone)]
pub struct StorageUsed {
    pub cells: BigUint,
    pub bits: BigUint,
}

impl<'de> BitUnpack<'de> for StorageUsed {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let cells = reader.unpack_as::<_, VarInt<3>>(())?;
        let bits = reader.unpack_as::<_, VarInt<3>>(())?;

        Ok(Self { cells, bits })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::bitvec::vec::BitVec;
    use toner::tlb::bits::de::unpack_fully;

    fn push_bits(bv: &mut BitVec<u8, Msb0>, byte: u8, count: usize) {
        for i in (0..count).rev() {
            bv.push((byte >> i) & 1 == 1);
        }
    }

    fn push_var_uint7(bv: &mut BitVec<u8, Msb0>, value: u64) {
        if value == 0 {
            push_bits(bv, 0, 3); // len = 0
            return;
        }
        let bytes = value.to_be_bytes();
        let first_nonzero = bytes.iter().position(|&b| b != 0).unwrap();
        let len = 8 - first_nonzero;
        push_bits(bv, len as u8, 3); // len in 7 bits
        for &b in &bytes[first_nonzero..] {
            push_bits(bv, b, 8);
        }
    }

    #[test]
    fn unpack_zeros() {
        let mut bits = BitVec::<u8, Msb0>::new();
        push_var_uint7(&mut bits, 0);
        push_var_uint7(&mut bits, 0);

        let result: StorageUsed = unpack_fully(&bits, ()).unwrap();

        assert_eq!(result.cells, BigUint::from(0u64));
        assert_eq!(result.bits, BigUint::from(0u64));
    }

    #[test]
    fn unpack_small_values() {
        let mut bits = BitVec::<u8, Msb0>::new();
        push_var_uint7(&mut bits, 42);
        push_var_uint7(&mut bits, 100);

        let result: StorageUsed = unpack_fully(&bits, ()).unwrap();

        assert_eq!(result.cells, BigUint::from(42u64));
        assert_eq!(result.bits, BigUint::from(100u64));
    }

    #[test]
    fn unpack_large_values() {
        let mut bits = BitVec::<u8, Msb0>::new();
        push_var_uint7(&mut bits, 1_000_000);
        push_var_uint7(&mut bits, 5_000_000);

        let result: StorageUsed = unpack_fully(&bits, ()).unwrap();

        assert_eq!(result.cells, BigUint::from(1_000_000u64));
        assert_eq!(result.bits, BigUint::from(5_000_000u64));
    }
}
