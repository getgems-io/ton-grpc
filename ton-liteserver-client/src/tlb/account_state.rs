use adnl_tcp::types::Int256;
use toner::tlb::bits::de::{BitReader, BitReaderExt};
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::ton::bits::de::BitUnpack;
use toner::ton::state_init::StateInit;

/// ```tlb
/// account_uninit$00 = AccountState;
/// account_active$1 _:StateInit = AccountState;
/// account_frozen$01 state_hash:bits256 = AccountState;
/// ```
pub enum AccountState {
    Uninit,
    Active { state_init: StateInit },
    Frozen { state_hash: Int256 },
}

impl<'de> CellDeserialize<'de> for AccountState {
    type Args = ();

    fn parse(parser: &mut CellParser<'de>, args: Self::Args) -> Result<Self, CellParserError<'de>> {
        let bit: bool = parser.unpack(())?;
        if bit {
            return Ok(AccountState::Active {
                state_init: parser.parse(())?,
            });
        }

        let bit: bool = parser.unpack(())?;
        match bit {
            false => Ok(AccountState::Uninit),
            true => Ok(AccountState::Frozen {
                state_hash: parser.unpack(())?,
            }),
        }
    }
}
