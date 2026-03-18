use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};

/// ```tlb
/// acst_unchanged$0 = AccStatusChange;
/// acst_frozen$10 = AccStatusChange;
/// acst_deleted$11 = AccStatusChange;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum AccStatusChange {
    Unchanged,
    Frozen,
    Deleted,
}

impl<'de> BitUnpack<'de> for AccStatusChange {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let b0: bool = reader.unpack(())?;
        if !b0 {
            return Ok(AccStatusChange::Unchanged);
        }

        let b1: bool = reader.unpack(())?;
        match b1 {
            false => Ok(AccStatusChange::Frozen),
            true => Ok(AccStatusChange::Deleted),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::bitvec::vec::BitVec;
    use toner::tlb::bits::de::unpack_fully;

    #[test]
    fn unpack_unchanged() {
        let mut bits = BitVec::<u8, Msb0>::new();
        bits.push(false);

        let result: AccStatusChange = unpack_fully(&bits, ()).unwrap();

        assert_eq!(result, AccStatusChange::Unchanged);
    }

    #[test]
    fn unpack_frozen() {
        let mut bits = BitVec::<u8, Msb0>::new();
        bits.push(true);
        bits.push(false);

        let result: AccStatusChange = unpack_fully(&bits, ()).unwrap();

        assert_eq!(result, AccStatusChange::Frozen);
    }

    #[test]
    fn unpack_deleted() {
        let mut bits = BitVec::<u8, Msb0>::new();
        bits.push(true);
        bits.push(true);

        let result: AccStatusChange = unpack_fully(&bits, ()).unwrap();

        assert_eq!(result, AccStatusChange::Deleted);
    }
}
