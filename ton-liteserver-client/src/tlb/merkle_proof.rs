use crate::tlb::block_header::BlockHeader;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::bits::r#as::NBits;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::r#as::{Ref, Same};

/// ```tlb
/// !merkle_proof#03 {X:Type} virtual_hash:bits256 depth:uint16 virtual_root:^X = MERKLE_PROOF X;
/// ```
#[derive(Debug, Clone)]
pub struct MerkleProof {
    pub virtual_hash: [u8; 32],
    pub depth: u16,
    pub virtual_root: BlockHeader,
}

impl<'de> CellDeserialize<'de> for MerkleProof {
    fn parse(parser: &mut CellParser<'de>) -> Result<Self, CellParserError<'de>> {
        let tag: u8 = parser.unpack_as::<_, NBits<8>>()?;
        if tag != 0x03 {
            unreachable!()
        };

        let virtual_hash = parser.unpack()?;
        let depth = parser.unpack()?;
        let virtual_root = parser.parse_as::<_, Ref<Same>>()?;

        Ok(Self {
            virtual_hash,
            depth,
            virtual_root,
        })
    }
}
