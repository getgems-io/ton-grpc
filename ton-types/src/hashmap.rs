use std::collections::HashMap;
use bitter::{BitReader, LittleEndianReader};
use crate::bag_of_cells::CellInBag;
use crate::cell::CellId;

trait FromBitReader {
    fn from_bit_reader(reader: &mut LittleEndianReader) -> Self;
}

pub struct Unary {
    n: usize
}

impl FromBitReader for Unary {
    fn from_bit_reader(reader: &mut LittleEndianReader) -> Self {
        let mut n = 0;
        loop {
            let bit = reader.read_bit().expect("incorrect bit sequence");

            if bit {
                n += 1;
            } else {
                return Self { n }
            }
        }
    }
}

#[derive(Debug)]
pub struct HmLabel {
    n: u32,
    m: u32,
    label: u32
}

impl HmLabel {
    pub fn read(m: u32, reader: &mut LittleEndianReader) -> Self {
        let W = len_bits(m);
        let bit = reader.read_bit().unwrap();
        if bit {
            let bit = reader.read_bit().unwrap();
            if bit {
                let bit = reader.read_bit().unwrap();
                let len = reader.read_bits(len_bits(m)).unwrap();

                if bit {
                    Self { label: (1u32 << len) - 1, m, n: len as u32 }
                } else {
                    Self { label: 0, m, n: len as u32 }
                }
            } else {
                let len = reader.read_bits(len_bits(m)).unwrap() as u32;
                let label= reader.read_bits(len).unwrap() as u32;

                Self { label: label, m, n: len }
            }
        } else {
            let Unary { n: len} = Unary::from_bit_reader(reader);
            let label = reader.read_bits(len as u32).unwrap() as u32;

            Self { label, m, n: len as u32 }
        }
    }
}

const fn len_bits(value: u32) -> u32 {
    32 - (value - 1).leading_zeros()
}

struct Hashmap<X> {
    label: HmLabel,
    hashmap_node: HashmapNode<X>
}

pub enum HashmapNode<X> {
    Leaf { value: X },
    Fork { left: CellId, right: CellId }
}

#[derive(Default, Debug)]
struct HashmapE<const K: u32, X> {
    inner: HashMap<u32, X>
}

impl<const K: u32, X> HashmapE<K, X> where X: Default {
    fn from_bit_reader(cell: &CellInBag) -> Self {
        let mut inner = HashMap::new();
        let mut reader = LittleEndianReader::new(cell.as_ref());

        let bit = reader.read_bit().unwrap();
        if bit {
            let root = cell.children().next().unwrap();
            println!("root: {:?}", root);
            let mut reader = LittleEndianReader::new(root.as_ref());
            let label = HmLabel::read(K, &mut reader);
            println!("label: {:?}", label);

            let m = K - label.n;
            println!("m: {:?}", m);
            if m > 0 {
                let mut iter = root.children();
                let left = iter.next().unwrap();
                println!("left: {:?}", left);
                for c in left.children() {
                    println!("c: {:?}", c);
                }

                let right = iter.next().unwrap();
                println!("right: {:?}", right);
            } else {
                let v = X::default();
                // let v = X::from_bit_reader(&mut reader);
                inner.insert(label.label, v);
            }

            Self { inner }

        } else {
            Self { inner: Default::default() }
        }
    }
}

/**
hme_empty$0 {n:#} {X:Type} = HashmapE n X;
hme_root$1 {n:#} {X:Type} root:^(Hashmap n X) = HashmapE n X

hm_edge#_ {n:#} {X:Type} {l:#} {m:#} label:(HmLabel ~l n)
          {n = (~m) + l} node:(HashmapNode m X) = Hashmap n X;

hmn_leaf#_ {X:Type} value:X = HashmapNode 0 X;
hmn_fork#_ {n:#} {X:Type} left:^(Hashmap n X)
           right:^(Hashmap n X) = HashmapNode (n + 1) X;

hml_short$0 {m:#} {n:#} len:(Unary ~n) {n <= m} s:(n * Bit) = HmLabel ~n m;
hml_long$10 {m:#} n:(#<= m) s:(n * Bit) = HmLabel ~n m;
hml_same$11 {m:#} v:Bit n:(#<= m) = HmLabel ~n m;


_ (HashmapE 32 ^(BinTree ShardDescr)) = ShardHashes;

bt_leaf$0 {X:Type} leaf:X = BinTree X;
bt_fork$1 {X:Type} left:^(BinTree X) right:^(BinTree X)
          = BinTree X;

**/


#[cfg(test)]
mod tests {
    use bitter::LittleEndianReader;
    use crate::bag_of_cells::BagOfCells;
    use crate::deserializer::from_bytes;
    use crate::hashmap::{FromBitReader, HashmapE, HmLabel, Unary};

    #[test]
    fn parser_ordering_test() {
        let n = 0b10000000;

        assert_eq!(128, n);
    }

    #[test]
    fn unary_zero_test() {
        let input = vec![0_u8];
        let mut reader = LittleEndianReader::new(input.as_slice());

        let unary = Unary::from_bit_reader(&mut reader);

        assert_eq!(unary.n, 0);
    }

    #[test]
    fn unary_succ_test() {
        let input = vec![0b1110000_u8];
        let mut reader = LittleEndianReader::new(input.as_slice());

        let unary = Unary::from_bit_reader(&mut reader);

        assert_eq!(unary.n, 3);
    }

    #[test]
    fn hmlabel_short_test() {
        let input = vec![0b1010_111_0_u8];
        let mut reader = LittleEndianReader::new(input.as_slice());

        let label: HmLabel = HmLabel::read(32, &mut reader);

        assert_eq!(label.label, 0b00000101);
        assert_eq!(label.m, 32);
        assert_eq!(label.n, 3);
    }

    #[test]
    fn hmlabel_long_test() {
        // let input = vec![0b101_011_01_u8];
        let input = vec![0b10_110_101_u8];
        let mut reader = LittleEndianReader::new(input.as_slice());

        let label: HmLabel = HmLabel::read(8, &mut reader);

        assert_eq!(label.label, 0b00000101);
        assert_eq!(label.m, 8);
        assert_eq!(label.n, 3);
    }

    #[test]
    fn hmlabel_same_test() {
        let input = vec![0b01000_1_11_u8];
        let mut reader = LittleEndianReader::new(input.as_slice());

        let label: HmLabel = HmLabel::read(32, &mut reader);

        assert_eq!(label.label, 0b11111111);
        assert_eq!(label.m, 32);
        assert_eq!(label.n, 8);
    }

    #[test]
    fn shard_hashes_test() {
        let bytes = hex::decode("b5ee9c7201020701000110000101c0010103d040020201c0030401eb5014c376901214cdb0000152890a35b600000152890a35b85e31d8be7f5f1b44600e445b3cf778b40eaad885db5153838bea3e8f0f4a9b25e36422b74bfadf372f7d3e16b48c05f4866b05d2c7e5787bd954a5d79ad9fdb6990000450f5a00000000000000001214cd933228b81ccc8a2e52000000c90501db5014c367381214cda8000152890aafc800000152890aafcefff0db0738592205986066e14fa1221d28f0156604fd4346cea0b705712ddd2872d9dc6b6fd4eb6624bf6cb9b77d673d2df07a993f5ed281b375f3c659c25e4df80000450f5e00000000000000001214cd933228b8020600134591048ab20ee6b28020001343332bfa820ee6b28020").unwrap();
        let boc = from_bytes::<BagOfCells>(&bytes).unwrap();
        let root = boc.root().unwrap();

        println!("root: {:?}", root);

        let hashmap = HashmapE::<32, bool>::from_bit_reader(&root);

        assert_eq!(hashmap.inner.len(), 2);
    }
}
