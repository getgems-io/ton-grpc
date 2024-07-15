use bitvec::order::Msb0;
use bitvec::vec::BitVec;
use bitvec::view::AsBits;

#[derive(Debug, PartialEq, Eq)]
pub struct ShardPrefix(BitVec<u8, Msb0>);

impl ShardPrefix {
    pub fn new(inner: BitVec<u8, Msb0>) -> Self {
        Self(inner)
    }

    pub fn from_shard_id(shard_id: u64) -> Self {
        let idx = shard_id.trailing_zeros();

        Self(shard_id.to_be_bytes().as_bits()[0..(64 - idx - 1) as usize].into())
    }

    pub fn matches(&self, address: &[u8; 32]) -> bool {
        address.as_bits::<Msb0>().starts_with(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use crate::router::shard_prefix::ShardPrefix;
    use bitvec::bitvec;
    use bitvec::prelude::Msb0;

    #[test]
    fn prefix_from_shard_id_test() {
        assert_eq!(
            ShardPrefix::from_shard_id(
                0b1000000000000000000000000000000000000000000000000000000000000000_u64
            ),
            ShardPrefix::new(bitvec![u8, Msb0;])
        );
        assert_eq!(
            ShardPrefix::from_shard_id(
                0b0100000000000000000000000000000000000000000000000000000000000000_u64
            ),
            ShardPrefix::new(bitvec![u8, Msb0; 0])
        );
        assert_eq!(
            ShardPrefix::from_shard_id(
                0b1100000000000000000000000000000000000000000000000000000000000000_u64
            ),
            ShardPrefix::new(bitvec![u8, Msb0; 1])
        );
        assert_eq!(
            ShardPrefix::from_shard_id(
                0b1110000000000000000000000000000000000000000000000000000000000000_u64
            ),
            ShardPrefix::new(bitvec![u8, Msb0; 1, 1])
        );
        assert_eq!(
            ShardPrefix::from_shard_id(u64::MAX),
            ShardPrefix::new(bitvec![u8, Msb0; 1; 63])
        );
    }

    #[test]
    fn empty_prefix_matches_address() {
        let prefix = ShardPrefix::from_shard_id(
            0b1000000000000000000000000000000000000000000000000000000000000000_u64,
        );

        assert_eq!(prefix.matches(&[0u8; 32]), true);
        assert_eq!(prefix.matches(&[1u8; 32]), true);
    }
}
