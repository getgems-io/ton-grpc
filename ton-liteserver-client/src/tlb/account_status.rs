use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};

/// ```tlb
/// acc_state_uninit$00 = AccountStatus;
/// acc_state_frozen$01 = AccountStatus;
/// acc_state_active$10 = AccountStatus;
/// acc_state_nonexist$11 = AccountStatus;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum AccountStatus {
    Uninit,
    Frozen,
    Active,
    Nonexist,
}

impl<'de> BitUnpack<'de> for AccountStatus {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let b0: bool = reader.unpack(())?;
        let b1: bool = reader.unpack(())?;

        match (b0, b1) {
            (false, false) => Ok(AccountStatus::Uninit),
            (false, true) => Ok(AccountStatus::Frozen),
            (true, false) => Ok(AccountStatus::Active),
            (true, true) => Ok(AccountStatus::Nonexist),
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
    fn unpack_uninit() {
        let mut bits = BitVec::<u8, Msb0>::new();
        bits.push(false);
        bits.push(false);

        let result: AccountStatus = unpack_fully(&bits, ()).unwrap();

        assert_eq!(result, AccountStatus::Uninit);
    }

    #[test]
    fn unpack_frozen() {
        let mut bits = BitVec::<u8, Msb0>::new();
        bits.push(false);
        bits.push(true);

        let result: AccountStatus = unpack_fully(&bits, ()).unwrap();

        assert_eq!(result, AccountStatus::Frozen);
    }

    #[test]
    fn unpack_active() {
        let mut bits = BitVec::<u8, Msb0>::new();
        bits.push(true);
        bits.push(false);

        let result: AccountStatus = unpack_fully(&bits, ()).unwrap();

        assert_eq!(result, AccountStatus::Active);
    }

    #[test]
    fn unpack_nonexist() {
        let mut bits = BitVec::<u8, Msb0>::new();
        bits.push(true);
        bits.push(true);

        let result: AccountStatus = unpack_fully(&bits, ()).unwrap();

        assert_eq!(result, AccountStatus::Nonexist);
    }
}
