use toner::ton::state_init::StateInit;
use toner_tlb_macros::CellDeserialize;

/// ```tlb
/// account_uninit$00 = AccountState;
/// account_active$1 _:StateInit = AccountState;
/// account_frozen$01 state_hash:bits256 = AccountState;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, CellDeserialize)]
pub enum AccountState {
    #[tlb(tag = "$00")]
    Uninit,
    #[tlb(tag = "$1")]
    Active { state_init: StateInit },
    #[tlb(tag = "$01")]
    Frozen {
        #[tlb(bits)]
        state_hash: [u8; 32],
    },
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

        assert_eq!(actual, AccountState::Active { state_init });
    }
}
