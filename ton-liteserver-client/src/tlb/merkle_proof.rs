use sha2::{Digest, Sha256};
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
        let actual_hash = ton_hash_level_0(&virtual_root_cell);
        if actual_hash != virtual_hash {
            return Err(Error::custom(format!(
                "MerkleProof virtual_hash mismatch: expected {}, actual {}",
                hex::encode(virtual_hash),
                hex::encode(actual_hash)
            )));
        }

        // Verify depth
        let actual_depth = get_ton_max_depth(&virtual_root_cell);
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

// TODO[akostylev0]: should be fixed in toner

/// Computes the TON representation hash (H_0) of a cell.
/// This implementation correctly handles exotic cells like PrunedBranch.
fn ton_hash_level_0(cell: &Cell) -> [u8; 32] {
    let data = cell.data.as_raw_slice();

    // If it's a PrunedBranch, its H_0 is stored in its data.
    if data.len() >= 2 && data[0] == 0x01 {
        if data.len() >= 34 {
            let mut h = [0u8; 32];
            h.copy_from_slice(&data[2..34]);
            return h;
        }
    }

    // Standard cell hashing for level 0
    let mut hasher = Sha256::new();

    // d1 descriptor: refs_count + is_exotic * 8 + level * 32
    // For level 0 hash, we use level = 0.
    let is_exotic = data.len() >= 1 && (data[0] == 0x01 || data[0] == 0x03 || data[0] == 0x04);
    let d1 = (cell.references.len() as u8) | (if is_exotic { 8 } else { 0 });

    // d2 descriptor: floor(bits/8) + ceil(bits/8)
    let bits = cell.data.len();
    let bytes = (bits + 7) / 8;
    let full_bytes = bits % 8 == 0;
    let d2 = (bits / 8) as u8 + bytes as u8;

    hasher.update([d1, d2]);

    // Data with padding
    if full_bytes {
        hasher.update(&data[..bytes]);
    } else if bytes > 0 {
        hasher.update(&data[..bytes - 1]);
        let mut last = data[bytes - 1];
        let padding_bits = bits % 8;
        last &= !0u8 << (8 - padding_bits);
        last |= 1 << (8 - padding_bits - 1);
        hasher.update([last]);
    }

    // Refs depths and hashes
    for r in &cell.references {
        hasher.update(get_ton_max_depth(r).to_be_bytes());
    }
    for r in &cell.references {
        hasher.update(ton_hash_level_0(r));
    }

    hasher.finalize().into()
}

/// Computes the TON max depth of a cell, correctly handling PrunedBranch.
fn get_ton_max_depth(cell: &Cell) -> u16 {
    let data = cell.data.as_raw_slice();

    // For PrunedBranch, depth is stored in its data.
    if data.len() >= 2 && data[0] == 0x01 {
        let level = data[1] as usize;
        let depth_offset = 2 + 32 * level;
        if data.len() >= depth_offset + 2 {
            return u16::from_be_bytes([data[depth_offset], data[depth_offset + 1]]);
        }
    }

    // For ordinary cells, depth is max(children depths) + 1
    cell.references
        .iter()
        .map(|r| get_ton_max_depth(r))
        .max()
        .map(|d| d + 1)
        .unwrap_or(0)
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

