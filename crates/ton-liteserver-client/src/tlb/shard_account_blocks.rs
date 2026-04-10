use crate::tlb::account_block::AccountBlock;
use crate::tlb::currency_collection::CurrencyCollection;
use toner::tlb::Same;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::hashmap::aug::HashmapAugE;

/// ```tlb
/// _ (HashmapAugE 256 AccountBlock CurrencyCollection) = ShardAccountBlocks;
/// ```
#[derive(Debug, Clone)]
pub struct ShardAccountBlocks(pub HashmapAugE<AccountBlock, CurrencyCollection>);

impl<'de> CellDeserialize<'de> for ShardAccountBlocks {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let inner = parser.parse_as::<_, HashmapAugE<Same, Same>>((256, (), ()))?;

        Ok(Self(inner))
    }
}
