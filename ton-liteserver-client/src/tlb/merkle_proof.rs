use toner::tlb::bits::{de::BitReaderExt, NBits};
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::{ParseFully, Ref};

/// ```tlb
/// !merkle_proof#03 {X:Type} virtual_hash:bits256 depth:uint16 virtual_root:^X = MERKLE_PROOF X;
/// ```
#[derive(Debug, Clone)]
pub struct MerkleProof<X> {
    pub virtual_hash: [u8; 32],
    pub depth: u16,
    pub virtual_root: X,
}

impl<'de, X> CellDeserialize<'de> for MerkleProof<X>
where
    X: CellDeserialize<'de, Args = ()>,
{
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u8 = parser.unpack_as::<_, NBits<8>>(())?;
        if tag != 0x03 {
            unreachable!()
        };

        let virtual_hash = parser.unpack(())?;
        let depth = parser.unpack(())?;
        let virtual_root = parser.parse_as::<X, Ref<ParseFully>>(())?;

        Ok(Self {
            virtual_hash,
            depth,
            virtual_root,
        })
    }
}
