use crate::tlb::merkle_proof::MerkleProof;
use anyhow::anyhow;
use toner::tlb::bits::de::unpack_bytes_fully;
use toner::tlb::{BoC, Cell};

pub(super) fn list_block_transactions_mode(
    has_after: bool,
    reverse: bool,
    want_proof: bool,
) -> i32 {
    const WITH_ACCOUNT: i32 = 1 << 0;
    const WITH_LT: i32 = 1 << 1;
    const WITH_HASH: i32 = 1 << 2;
    const WANT_PROOF: i32 = 1 << 5;
    const REVERSE_ORDER: i32 = 1 << 6;
    const AFTER: i32 = 1 << 7;

    let mut mode = WITH_ACCOUNT | WITH_LT | WITH_HASH;
    if want_proof {
        mode |= WANT_PROOF;
    }
    if reverse {
        mode |= REVERSE_ORDER;
    }
    if has_after {
        mode |= AFTER;
    }
    mode
}

pub(super) fn verify_header_proof(
    proof_bytes: &[u8],
    expected_root_hash: &[u8; 32],
) -> anyhow::Result<()> {
    let boc: BoC = unpack_bytes_fully(proof_bytes, ())?;
    let root = boc
        .single_root()
        .ok_or_else(|| anyhow!("header proof: single root expected"))?;

    let proof: MerkleProof<Cell> = root.parse_fully(())?;
    if &proof.virtual_hash != expected_root_hash {
        return Err(anyhow!(
            "header proof root hash mismatch: expected {}, got {}",
            hex::encode(expected_root_hash),
            hex::encode(proof.virtual_hash)
        ));
    }

    Ok(())
}

// TODO verify individual transaction inclusion via ShardAccountBlocks dict traversal in proof
pub(super) fn verify_block_proof(
    proof_bytes: &[u8],
    expected_root_hash: &[u8; 32],
) -> anyhow::Result<()> {
    if proof_bytes.is_empty() {
        return Err(anyhow!("empty proof"));
    }

    let boc: BoC = BoC::deserialize(proof_bytes)?;
    let root = boc
        .single_root()
        .ok_or_else(|| anyhow!("proof: single root expected"))?;

    let proof: MerkleProof<Cell> = root.parse_fully(())?;

    if &proof.virtual_hash != expected_root_hash {
        return Err(anyhow!(
            "proof root hash mismatch: expected {}, got {}",
            hex::encode(expected_root_hash),
            hex::encode(proof.virtual_hash)
        ));
    }

    Ok(())
}
