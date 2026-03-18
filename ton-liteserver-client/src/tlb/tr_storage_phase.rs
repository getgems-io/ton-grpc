use num_bigint::BigUint;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};
use toner::ton::currency::Grams;

use crate::tlb::acc_status_change::AccStatusChange;

/// ```tlb
/// tr_phase_storage$_ storage_fees_collected:Grams
///   storage_fees_due:(Maybe Grams)
///   status_change:AccStatusChange
///   = TrStoragePhase;
/// ```
#[derive(Debug, Clone)]
pub struct TrStoragePhase {
    pub storage_fees_collected: BigUint,
    pub storage_fees_due: Option<BigUint>,
    pub status_change: AccStatusChange,
}

impl<'de> BitUnpack<'de> for TrStoragePhase {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let storage_fees_collected: BigUint = reader.unpack_as::<_, Grams>(())?;
        let storage_fees_due = reader.unpack_as::<_, Option<Grams>>(())?;
        let status_change: AccStatusChange = reader.unpack(())?;

        Ok(Self {
            storage_fees_collected,
            storage_fees_due,
            status_change,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::bitvec::vec::BitVec;
    use toner::tlb::bits::de::unpack_fully;

    #[test]
    fn unpack_without_due() {
        let mut bits = BitVec::<u8, Msb0>::new();
        push_grams(&mut bits, 100);
        bits.push(false); // Maybe: None
        bits.push(false); // AccStatusChange: Unchanged ($0)

        let result: TrStoragePhase = unpack_fully(&bits, ()).unwrap();

        assert_eq!(result.storage_fees_collected, BigUint::from(100u64));
        assert!(result.storage_fees_due.is_none());
        assert_eq!(result.status_change, AccStatusChange::Unchanged);
    }

    #[test]
    fn unpack_with_due_and_frozen() {
        let mut bits = BitVec::<u8, Msb0>::new();
        push_grams(&mut bits, 500);
        bits.push(true); // Maybe: Some
        push_grams(&mut bits, 200);
        bits.push(true);  // AccStatusChange: $10 = Frozen
        bits.push(false);

        let result: TrStoragePhase = unpack_fully(&bits, ()).unwrap();

        assert_eq!(result.storage_fees_collected, BigUint::from(500u64));
        assert_eq!(result.storage_fees_due, Some(BigUint::from(200u64)));
        assert_eq!(result.status_change, AccStatusChange::Frozen);
    }

    #[test]
    fn unpack_with_due_and_deleted() {
        let mut bits = BitVec::<u8, Msb0>::new();
        push_grams(&mut bits, 0);
        bits.push(true); // Maybe: Some
        push_grams(&mut bits, 1000);
        bits.push(true); // AccStatusChange: $11 = Deleted
        bits.push(true);

        let result: TrStoragePhase = unpack_fully(&bits, ()).unwrap();

        assert_eq!(result.storage_fees_collected, BigUint::from(0u64));
        assert_eq!(result.storage_fees_due, Some(BigUint::from(1000u64)));
        assert_eq!(result.status_change, AccStatusChange::Deleted);
    }

    fn push_bits(bits: &mut BitVec<u8, Msb0>, byte: u8, count: usize) {
        for i in (0..count).rev() {
            bits.push((byte >> i) & 1 == 1);
        }
    }

    fn push_grams(bits: &mut BitVec<u8, Msb0>, value: u64) {
        if value == 0 {
            push_bits(bits, 0, 4);
            return;
        }
        let bytes = value.to_be_bytes();
        let first_nonzero = bytes.iter().position(|&b| b != 0).unwrap();
        let len = 8 - first_nonzero;
        push_bits(bits, len as u8, 4);
        for &b in &bytes[first_nonzero..] {
            push_bits(bits, b, 8);
        }
    }
}
