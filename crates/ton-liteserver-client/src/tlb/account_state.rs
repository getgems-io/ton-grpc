use std::sync::Arc;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::hashmap::HashmapE;
use toner::tlb::{Cell, Context, Ref, Same};
use toner::ton::state_init::{SimpleLib, TickTock};

/// ```tlb
/// account_uninit$00 = AccountState;
/// account_active$1 _:StateInit = AccountState;
/// account_frozen$01 state_hash:bits256 = AccountState;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountState {
    Uninit,
    Active { state_init: RawStateInit },
    Frozen { state_hash: [u8; 32] },
}

/// Like toner's `StateInit`, but stores code/data as original `Arc<Cell>`
/// to preserve byte-exact BoC serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawStateInit {
    pub code: Option<Arc<Cell>>,
    pub data: Option<Arc<Cell>>,
}

impl<'de> CellDeserialize<'de> for AccountState {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let first: bool = parser.unpack(()).context("tag[0]")?;
        if first {
            let state_init = parser.parse(()).context("StateInit")?;
            Ok(AccountState::Active { state_init })
        } else {
            let second: bool = parser.unpack(()).context("tag[1]")?;
            if second {
                let state_hash = parser.unpack(()).context("state_hash")?;
                Ok(AccountState::Frozen { state_hash })
            } else {
                Ok(AccountState::Uninit)
            }
        }
    }
}

impl<'de> CellDeserialize<'de> for RawStateInit {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        use toner::tlb::bits::NBits;

        // split_depth:(Maybe (## 5))
        let _split_depth: Option<u8> = parser.unpack_as::<_, Option<NBits<5>>>(())?;
        // special:(Maybe TickTock)
        let _special: Option<TickTock> = parser.unpack(())?;
        // code:(Maybe ^Cell)
        let code = parser
            .parse_as::<_, Option<Ref<Same>>>(())
            .context("code")?;
        // data:(Maybe ^Cell)
        let data = parser
            .parse_as::<_, Option<Ref<Same>>>(())
            .context("data")?;
        // library:(HashmapE 256 SimpleLib)
        let _library: HashmapE<SimpleLib> =
            parser.parse_as::<_, HashmapE<Same, Same>>((256, (), ()))?;

        Ok(RawStateInit { code, data })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toner::tlb::bits::ser::BitWriterExt;
    use toner::tlb::ser::{CellBuilder, CellBuilderError, CellSerialize, CellSerializeExt};
    use toner::ton::state_init::StateInit;

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

        assert!(matches!(actual, AccountState::Uninit));
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

        match actual {
            AccountState::Frozen { state_hash } => assert_eq!(state_hash, [0xab; 32]),
            _ => panic!("expected Frozen"),
        }
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
        let cell = Wrapper(state_init).to_cell(()).unwrap();

        let actual: AccountState = cell.parse_fully(()).unwrap();

        match actual {
            AccountState::Active { state_init } => {
                assert!(state_init.code.is_none());
                assert!(state_init.data.is_none());
            }
            _ => panic!("expected Active"),
        }
    }
}
