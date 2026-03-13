use toner::tlb::bits::{de::BitReaderExt, NBits};
use toner::tlb::de::{CellDeserialize, CellDeserializeOwned, CellParser, CellParserError};
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

        let virtual_hash: [u8; 32] = parser.unpack(())?;
        let depth: u16 = parser.unpack(())?;
        let virtual_root_cell = parser.parse_as::<Cell, Ref>(())?;

        // Verify virtual_hash (always use level 0 hash for LiteServer proofs)
        let actual_hash = virtual_root_cell.hash();
        if actual_hash != virtual_hash {
            return Err(Error::custom(format!(
                "MerkleProof virtual_hash mismatch: expected {}, actual {}",
                hex::encode(virtual_hash),
                hex::encode(actual_hash)
            )));
        }

        // Verify depth
        let actual_depth = virtual_root_cell.max_depth();
        if actual_depth != depth {
            return Err(Error::custom(format!(
                "MerkleProof depth mismatch: expected {}, actual {}",
                depth, actual_depth
            )));
        }

        let virtual_root = virtual_root_cell.parse_fully::<X>(())?;

        parser.ensure_empty()?;

        Ok(Self {
            virtual_hash,
            depth,
            virtual_root,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tlb::block_header::BlockHeader;
    use toner::tlb::bits::de::unpack_bytes;
    use toner::tlb::BoC;

    const BLOCK_HEADER_MERKLE_PROOF_HEX: &str = "b5ee9c7201020701000147000946039b3184087274bb28db6a90ce88e0d3918bdebf723f89fc121a1e77d02e34cf5f001601241011ef55aaffffff110203040501a09bc7a9870000000004010377cc200000000100ffffffff000000000000000069b18e3900003db4349b4a8000003db4349b4a840156eebf000c24fe0377cc1d0377b158c40000000d00000000000003ee062848010115de070e339a9502e7a8a6eaeedaec93a25541912432b6e092953bc9fe37d4b90003284801014ee51101facacb57d95fd8a1db5635f2c340a39344fafeef7550186d72fb58ee001528480101fc889cc027e955ec7bc16fb1d58657b8ca58bbcc712d8d750ce10242f25c8d6c0007009800003db4348c08440377cc1f8160dc2356c91ba646a663ee57a97297fad1efac8e825527461c1fb6292041ab647b042a0f67c0e15558e713eb76b4f000454f75a0f4500c0f4b28bace1d2dc9";

    #[test]
    fn test_merkle_proof_parse_ok() {
        let data = hex::decode(BLOCK_HEADER_MERKLE_PROOF_HEX).unwrap();
        let boc: BoC = unpack_bytes(&data, ()).unwrap();
        let root = boc.single_root().unwrap();

        let header: MerkleProof<BlockHeader> = root.parse_fully(()).unwrap();

        assert_eq!(header.depth, 22);
        assert_eq!(
            hex::encode(header.virtual_hash),
            "9b3184087274bb28db6a90ce88e0d3918bdebf723f89fc121a1e77d02e34cf5f"
        );
    }

    #[test]
    fn test_merkle_proof_invalid_hash() {
        let mut data = hex::decode(BLOCK_HEADER_MERKLE_PROOF_HEX).unwrap();
        let hash_hex = "9b3184087274bb28db6a90ce88e0d3918bdebf723f89fc121a1e77d02e34cf5f";
        let pos = BLOCK_HEADER_MERKLE_PROOF_HEX.find(hash_hex).unwrap() / 2;
        data[pos] ^= 0xFF;

        let boc: BoC = unpack_bytes(&data, ()).unwrap();
        let root = boc.single_root().unwrap();

        let res: Result<MerkleProof<BlockHeader>, _> = root.parse_fully(());

        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("MerkleProof virtual_hash mismatch"));
    }

    #[test]
    fn test_merkle_proof_invalid_depth() {
        let mut data = hex::decode(BLOCK_HEADER_MERKLE_PROOF_HEX).unwrap();
        let hash_hex = "9b3184087274bb28db6a90ce88e0d3918bdebf723f89fc121a1e77d02e34cf5f";
        let pos = (BLOCK_HEADER_MERKLE_PROOF_HEX.find(hash_hex).unwrap() / 2) + 32;
        data[pos + 1] ^= 0xFF; // Change depth

        let boc: BoC = unpack_bytes(&data, ()).unwrap();
        let root = boc.single_root().unwrap();

        let res: Result<MerkleProof<BlockHeader>, _> = root.parse_fully(());

        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("MerkleProof depth mismatch"));
    }

    #[test]
    fn test_merkle_proof_invalid_root_data() {
        let mut data = hex::decode(BLOCK_HEADER_MERKLE_PROOF_HEX).unwrap();
        // Change the very last byte of the BoC (part of the leaf cell data)
        let last = data.len() - 1;
        data[last] ^= 0xFF;

        let boc: BoC = unpack_bytes(&data, ()).unwrap();
        let root = boc.single_root().unwrap();

        let res: Result<MerkleProof<BlockHeader>, _> = root.parse_fully(());

        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("MerkleProof virtual_hash mismatch"));
    }
}
