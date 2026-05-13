use toner::tlb::bits::{NBits, de::BitReaderExt};
use toner::tlb::de::{
    CellDeserialize, CellDeserializeAs, CellDeserializeOwned, CellParser, CellParserError,
};
use toner::tlb::{Cell, Error, Ref};

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
    X: CellDeserializeOwned<Args = ()>,
{
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        parser.ensure_exotic()?;
        let tag: u8 = parser.unpack_as::<_, NBits<8>>(())?;
        if tag != 0x03 {
            return Err(Error::custom(format!(
                "invalid MerkleProof tag: 0x{:02x}",
                tag
            )));
        };

        let expected_hash = parser.unpack(())?;
        let expected_depth = parser.unpack(())?;
        let virtual_root_cell = parser.parse_as::<Cell, Ref>(())?;
        let Some((actual_depth, actual_hash)) = virtual_root_cell.level_hash(0) else {
            return Err(Error::custom(
                "MerkleProof virtual_root_cell has no level hash".to_string(),
            ));
        };

        if actual_hash != expected_hash {
            return Err(Error::custom(format!(
                "MerkleProof virtual_hash mismatch: expected {}, actual {}",
                hex::encode(expected_hash),
                hex::encode(actual_hash)
            )));
        }

        if actual_depth != expected_depth {
            return Err(Error::custom(format!(
                "MerkleProof depth mismatch: expected {}, actual {}",
                expected_depth, actual_depth
            )));
        }

        let virtual_root = virtual_root_cell.parse_fully::<X>(())?;

        parser.ensure_empty()?;

        Ok(Self {
            virtual_hash: expected_hash,
            depth: expected_depth,
            virtual_root,
        })
    }
}

impl<'de, X> CellDeserializeAs<'de, X> for MerkleProof<X>
where
    X: CellDeserializeOwned<Args = ()>,
{
    type Args = ();

    fn parse_as(parser: &mut CellParser<'de>, args: Self::Args) -> Result<X, CellParserError<'de>> {
        let inner = parser.parse::<MerkleProof<X>>(args)?;

        Ok(inner.virtual_root)
    }
}

#[cfg(test)]
mod tests {
    use crate::tlb::block_header::BlockHeader;
    use crate::tlb::merkle_proof::MerkleProof;
    use crate::tlb::tests::BLOCK_HEADER_MERKLE_PROOF_HEX;
    use std::sync::Arc;
    use toner::tlb::bits::de::unpack_bytes;
    use toner::tlb::{BoC, Cell};

    #[test]
    fn test_merkle_proof_parse_ok() {
        let root = given_block_header_root_cell();

        let header: MerkleProof<BlockHeader> = root.parse_fully(()).unwrap();

        assert_eq!(header.depth, 22);
        assert_eq!(
            hex::encode(header.virtual_hash),
            "9b3184087274bb28db6a90ce88e0d3918bdebf723f89fc121a1e77d02e34cf5f"
        );
    }

    #[test]
    fn test_merkle_proof_parse_as_ok() {
        let root = given_block_header_root_cell();

        let header: BlockHeader = root.parse_fully_as::<_, MerkleProof<_>>(()).unwrap();

        assert_eq!(header.global_id, -239);
    }

    fn given_block_header_root_cell() -> Arc<Cell> {
        let data = hex::decode(BLOCK_HEADER_MERKLE_PROOF_HEX).unwrap();

        unpack_bytes::<BoC>(&data, ())
            .unwrap()
            .into_single_root()
            .unwrap()
    }
}
