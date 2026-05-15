use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::ton::state_init::StateInit;

/// ```tlb
/// account_uninit$00 = AccountState;
/// account_active$1 _:StateInit = AccountState;
/// account_frozen$01 state_hash:bits256 = AccountState;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountState {
    Uninit,
    Active(StateInit),
    Frozen { state_hash: [u8; 32] },
}

impl<'de> CellDeserialize<'de> for AccountState {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let first: bool = parser.unpack(())?;
        if first {
            let state_init: StateInit = parser.parse(())?;
            return Ok(AccountState::Active(state_init));
        }

        let second: bool = parser.unpack(())?;
        if second {
            let state_hash: [u8; 32] = parser.unpack(())?;
            return Ok(AccountState::Frozen { state_hash });
        }

        Ok(AccountState::Uninit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toner::tlb::bits::ser::BitWriterExt;
    use toner::tlb::ser::{CellBuilder, CellBuilderError, CellSerialize, CellSerializeExt};

    #[test]
    fn parses_uninit() {
        struct Wrapper;
        impl CellSerialize for Wrapper {
            type Args = ();
            fn store(&self, b: &mut CellBuilder, _: ()) -> Result<(), CellBuilderError> {
                b.pack(false, ())?.pack(false, ())?;
                Ok(())
            }
        }
        let cell = Wrapper.to_cell(()).unwrap();

        let actual: AccountState = cell.parse_fully(()).unwrap();

        assert_eq!(actual, AccountState::Uninit);
    }

    #[test]
    fn parses_frozen() {
        struct Wrapper;
        impl CellSerialize for Wrapper {
            type Args = ();
            fn store(&self, b: &mut CellBuilder, _: ()) -> Result<(), CellBuilderError> {
                b.pack(false, ())?;
                b.pack(true, ())?;
                let hash: [u8; 32] = [0xab; 32];
                b.pack(hash, ())?;
                Ok(())
            }
        }
        let cell = Wrapper.to_cell(()).unwrap();

        let actual: AccountState = cell.parse_fully(()).unwrap();

        assert_eq!(
            actual,
            AccountState::Frozen {
                state_hash: [0xab; 32]
            }
        );
    }

    #[test]
    fn parses_active_with_empty_state_init() {
        let state_init = StateInit::<toner::tlb::Cell, toner::tlb::Cell>::default();
        struct Wrapper(StateInit);
        impl CellSerialize for Wrapper {
            type Args = ();
            fn store(&self, b: &mut CellBuilder, _: ()) -> Result<(), CellBuilderError> {
                b.pack(true, ())?.store(&self.0, ())?;
                Ok(())
            }
        }
        let cell = Wrapper(state_init.clone()).to_cell(()).unwrap();

        let actual: AccountState = cell.parse_fully(()).unwrap();

        assert_eq!(actual, AccountState::Active(state_init));
    }
}
