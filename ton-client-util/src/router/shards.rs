use bitvec::order::Msb0;
use bitvec::vec::BitVec;
use bitvec::view::AsBits;

pub fn shard_id_into_prefix(shard_id: u64) -> BitVec<u8, Msb0> {
    let idx = shard_id.trailing_zeros();

    shard_id.to_be_bytes().as_bits()[0..(64 - idx - 1) as usize].into()
}

#[cfg(test)]
mod tests {
    use crate::router::shards::shard_id_into_prefix;
    use bitvec::bitvec;
    use bitvec::prelude::Msb0;

    #[test]
    fn shard_id_into_prefix_test() {
        assert_eq!(
            shard_id_into_prefix(
                0b1000000000000000000000000000000000000000000000000000000000000000_u64
            ),
            bitvec![u8, Msb0;]
        );
        assert_eq!(
            shard_id_into_prefix(
                0b0100000000000000000000000000000000000000000000000000000000000000_u64
            ),
            bitvec![u8, Msb0; 0]
        );
        assert_eq!(
            shard_id_into_prefix(
                0b1100000000000000000000000000000000000000000000000000000000000000_u64
            ),
            bitvec![u8, Msb0; 1]
        );
        assert_eq!(
            shard_id_into_prefix(
                0b1110000000000000000000000000000000000000000000000000000000000000_u64
            ),
            bitvec![u8, Msb0; 1, 1]
        );

        assert_eq!(shard_id_into_prefix(u64::MAX), bitvec![u8, Msb0; 1; 63]);
    }
}
