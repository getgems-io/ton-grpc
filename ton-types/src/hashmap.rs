use bitter::{BitReader, LittleEndianReader};

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

pub struct HmLabel {
    n: u32,
    m: u32,
    label: u32
}

fn read_hm_label(m: u32, reader: &mut LittleEndianReader) -> HmLabel {
    let bit = reader.read_bit().unwrap();
    if bit {
        let bit = reader.read_bit().unwrap();
        if bit {
            let bit = reader.read_bit().unwrap();
            let len = reader.read_bits(len_bits(m)).unwrap() as u32;

            if bit {
                HmLabel { label: (1u32 << len) - 1, m, n: len }
            } else {
                HmLabel { label: 0, m, n: len }
            }
        } else {
            let len = reader.read_bits(len_bits(m)).unwrap() as u32;
            let label= reader.read_bits(len).unwrap() as u32;

            HmLabel { label, m, n: len }
        }
    } else {
        let Unary { n: len} = Unary::from_bit_reader(reader);
        let label = reader.read_bits(len as u32).unwrap() as u32;

        HmLabel { label, m, n: len as u32 }
    }
}

const fn len_bits(value: u32) -> u32 {
    32 - (value - 1).leading_zeros()
}

struct Hashmap<X> {
    label: HmLabel,
    hashmap_node: HashmapNode<X>
}

pub enum HashmapNode <X> {
    Leaf { value: X },
    Fork { left: Option<X>, right: Option<X> }
}


#[cfg(test)]
mod tests {
    use bitter::LittleEndianReader;
    use crate::hashmap::{FromBitReader, HmLabel, read_hm_label, Unary};

    #[test]
    fn unary_zero_test() {
        let input = vec![0];
        let mut reader = LittleEndianReader::new(&input);

        let unary = Unary::from_bit_reader(&mut reader);

        assert_eq!(unary.n, 0);
    }

    #[test]
    fn unary_succ_test() {
        let input = vec![0b0000111];
        let mut reader = LittleEndianReader::new(&input);

        let unary = Unary::from_bit_reader(&mut reader);

        assert_eq!(unary.n, 3);
    }

    #[test]
    fn hmlabel_short_test() {
        let input = vec![0b1010_111_0];
        let mut reader = LittleEndianReader::new(&input);

        let label: HmLabel = read_hm_label(32, &mut reader);

        assert_eq!(label.label, 0b00000101);
        assert_eq!(label.m, 32);
        assert_eq!(label.n, 3);
    }

    #[test]
    fn hmlabel_long_test() {
        let input = vec![0b101_011_01];
        let mut reader = LittleEndianReader::new(&input);

        let label: HmLabel = read_hm_label(8, &mut reader);

        assert_eq!(label.label, 0b00000101);
        assert_eq!(label.m, 8);
        assert_eq!(label.n, 3);
    }

    #[test]
    fn hmlabel_same_test() {
        let input = vec![0b01000_1_11];
        let mut reader = LittleEndianReader::new(&input);

        let label: HmLabel = read_hm_label(32, &mut reader);

        assert_eq!(label.label, 0b11111111);
        assert_eq!(label.m, 32);
        assert_eq!(label.n, 8);
    }
}
