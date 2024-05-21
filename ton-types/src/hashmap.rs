use bitter::{BitReader, LittleEndianReader};

pub struct Unary {
    n: usize
}

impl Unary {
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

pub struct HmLabel<const MAX: u32> {
    label: u64
}

impl<const MAX: u32> HmLabel<MAX> {
    fn from_bit_reader(reader: &mut LittleEndianReader) -> Self {
        let bit = reader.read_bit().unwrap();
        if bit {
            let bit = reader.read_bit().unwrap();
            if bit {
                let bit = reader.read_bit().unwrap();
                let len = reader.read_bits(len_bits(MAX)).unwrap();

                if bit { Self { label: (1u64 << len) - 1 } } else { Self { label: 0 } }
            } else {
                let len = reader.read_bits(len_bits(MAX)).unwrap();
                let label= reader.read_bits(len as u32).unwrap();

                Self { label }
            }
        } else {
            let Unary { n: len} = Unary::from_bit_reader(reader);
            let label = reader.read_bits(len as u32).unwrap();

            Self { label }
        }
    }
}

const fn len_bits(value: u32) -> u32 {
    32 - (value - 1).leading_zeros()
}


#[cfg(test)]
mod tests {
    use bitter::LittleEndianReader;
    use crate::hashmap::{HmLabel, Unary};

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

        let label: HmLabel<32> = HmLabel::from_bit_reader(&mut reader);

        assert_eq!(label.label, 0b00000101)
    }

    #[test]
    fn hmlabel_long_test() {
        let input = vec![0b101_011_01];
        let mut reader = LittleEndianReader::new(&input);

        let label: HmLabel<8> = HmLabel::from_bit_reader(&mut reader);

        assert_eq!(label.label, 0b00000101);
    }

    #[test]
    fn hmlabel_same_test() {
        let input = vec![0b01000_1_11];
        let mut reader = LittleEndianReader::new(&input);

        let label: HmLabel<32> = HmLabel::from_bit_reader(&mut reader);

        assert_eq!(label.label, 0b11111111);
    }
}