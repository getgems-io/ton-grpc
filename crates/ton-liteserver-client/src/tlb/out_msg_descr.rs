use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::out_msg::OutMsg;
use toner::tlb::Same;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::hashmap::aug::HashmapAugE;

/// ```tlb
/// _ (HashmapAugE 256 OutMsg CurrencyCollection) = OutMsgDescr;
/// ```
/// #TODO[akostylev0]: use std Hashmap
#[derive(Debug, Clone)]
pub struct OutMsgDescr(pub HashmapAugE<OutMsg, CurrencyCollection>);

impl<'de> CellDeserialize<'de> for OutMsgDescr {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let inner = parser.parse_as::<_, HashmapAugE<Same, Same>>((256, (), ()))?;

        parser.ensure_empty()?;

        Ok(Self(inner))
    }
}
