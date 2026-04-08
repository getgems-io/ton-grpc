use crate::tlb::account_state::AccountState;
use crate::tlb::currency_collection::CurrencyCollection;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};

/// ```tlb
/// account_storage$_ last_trans_lt:uint64
///     balance:CurrencyCollection state:AccountState
///   = AccountStorage;
/// ```
pub struct AccountStorage {
    last_trans_lt: u64,
    balance: CurrencyCollection,
    state: AccountState,
}

impl<'de> CellDeserialize<'de> for AccountStorage {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        Ok(Self {
            last_trans_lt: parser.unpack(())?,
            balance: parser.parse(())?,
            state: parser.unpack(())?,
        })
    }
}
