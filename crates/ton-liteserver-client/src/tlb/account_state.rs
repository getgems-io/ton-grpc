use toner::tlb::Error;
use toner::tlb::bits::NBits;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};

/// ```tlb
/// account_uninit$00 = AccountState;
/// account_active$1 _:StateInit = AccountState;
/// account_frozen$01 state_hash:bits256 = AccountState;
/// ```
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AccountState {
    Uninit,
    Active,
    Frozen,
}

impl<'de> BitUnpack<'de> for AccountState {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let tag = reader.unpack_as::<_, NBits<2>>(())?;
        match tag {
            0b00 => Ok(AccountState::Uninit),
            0b10 => Ok(AccountState::Active),
            0b01 => Ok(AccountState::Frozen),
            _ => Err(R::Error::custom("Invalid AccountState value")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tlb::account_state::AccountState;
    use toner::tlb::bits::bitvec::bits;
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::de::unpack_fully;

    #[test]
    fn unpack_uninit() {
        let bits = bits![u8, Msb0; 0, 0];

        let actual: AccountState = unpack_fully(&bits, ()).unwrap();

        assert_eq!(actual, AccountState::Uninit);
    }

    #[test]
    fn unpack_active() {
        let bits = bits![u8, Msb0; 1, 0];

        let actual: AccountState = unpack_fully(&bits, ()).unwrap();

        assert_eq!(actual, AccountState::Active);
    }
    #[test]
    fn unpack_frozen() {
        let bits = bits![u8, Msb0; 0, 1];

        let actual: AccountState = unpack_fully(&bits, ()).unwrap();

        assert_eq!(actual, AccountState::Frozen);
    }
}
