use crate::tlb::account_storage::AccountStorage;
use crate::tlb::msg_address_int::MsgAddressInt;
use crate::tlb::storage_info::StorageInfo;
use toner::tlb::Error;
use toner::tlb::bits::de::{BitReader, BitReaderExt};
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};

/// ```tlb
/// account_none$0 = Account;
/// account$1 addr:MsgAddressInt storage_stat:StorageInfo
///           storage:AccountStorage = Account;
/// ```
pub enum Account {
    None,
    Account {
        addr: MsgAddressInt,
        storage_stat: StorageInfo,
        storage: AccountStorage,
    },
}

impl<'de> CellDeserialize<'de> for Account {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag = parser.read_bit()?;
        match tag {
            Some(false) => Ok(Account::None),
            Some(true) => Ok(Account::Account {
                addr: parser.unpack(())?,
                storage_stat: parser.unpack(())?,
                storage: parser.parse(())?,
            }),
            None => Err(Error::custom("not enough bits to read")),
        }
    }
}
