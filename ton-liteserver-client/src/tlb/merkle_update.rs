use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::bits::NBits;
use toner::tlb::de::{
    CellDeserialize, CellDeserializeOwned, CellParser, CellParserError,
};
use toner::tlb::{Cell, Error, Ref};

/// ```tlb
/// !merkle_update#04 {X:Type} old_hash:bits256 new_hash:bits256 old_depth:uint16 new_depth:uint16
///   old:^X new:^X = MERKLE_UPDATE X;
/// ```
#[derive(Debug, Clone)]
pub struct MerkleUpdate<X> {
    pub old_hash: [u8; 32],
    pub new_hash: [u8; 32],
    pub old_depth: u16,
    pub new_depth: u16,
    pub old: X,
    pub new: X,
}

impl<'de, X> CellDeserialize<'de> for MerkleUpdate<X>
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
        if tag != 0x04 {
            return Err(Error::custom(format!(
                "invalid MerkleUpdate tag: 0x{:02x}",
                tag
            )));
        };

        let old_hash: [u8; 32] = parser.unpack(())?;
        let new_hash: [u8; 32] = parser.unpack(())?;
        let old_depth: u16 = parser.unpack(())?;
        let new_depth: u16 = parser.unpack(())?;

        let old_cell: Cell = parser.parse_as::<_, Ref>(())?;
        let new_cell: Cell = parser.parse_as::<_, Ref>(())?;

        let actual_old_hash = old_cell.hash();
        if actual_old_hash != old_hash {
            return Err(Error::custom(format!(
                "MerkleUpdate old_hash mismatch: expected {}, actual {}",
                hex::encode(old_hash),
                hex::encode(actual_old_hash)
            )));
        }

        let actual_new_hash = new_cell.hash();
        if actual_new_hash != new_hash {
            return Err(Error::custom(format!(
                "MerkleUpdate new_hash mismatch: expected {}, actual {}",
                hex::encode(new_hash),
                hex::encode(actual_new_hash)
            )));
        }

        let actual_old_depth = old_cell.max_depth();
        if actual_old_depth != old_depth {
            return Err(Error::custom(format!(
                "MerkleUpdate old_depth mismatch: expected {}, actual {}",
                old_depth, actual_old_depth
            )));
        }

        let actual_new_depth = new_cell.max_depth();
        if actual_new_depth != new_depth {
            return Err(Error::custom(format!(
                "MerkleUpdate new_depth mismatch: expected {}, actual {}",
                new_depth, actual_new_depth
            )));
        }

        let old = old_cell.parse_fully::<X>(())?;
        let new = new_cell.parse_fully::<X>(())?;

        parser.ensure_empty()?;

        Ok(Self {
            old_hash,
            new_hash,
            old_depth,
            new_depth,
            old,
            new,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::tlb::tests::BLOCK_HEX;
    use std::sync::Arc;
    use toner::tlb::bits::de::unpack_bytes;
    use toner::tlb::{BoC, Cell};
    use crate::tlb::block::Block;

    #[test]
    fn test_merkle_update_ok() {
        // TODO[akostylev0]: test MerkleUpdate
        let root = given_block_root_cell();

        root.parse_fully::<Block>(()).unwrap();
    }

    fn given_block_root_cell() -> Arc<Cell> {
        let data = hex::decode(BLOCK_HEX).unwrap();

        unpack_bytes::<BoC>(&data, ())
            .unwrap()
            .into_single_root()
            .unwrap()
    }
}
