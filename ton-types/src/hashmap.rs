use std::ptr::read;
use bitter::BitReader;
use crate::deserializer::{Deserialize, Deserializer, DeserializerError};

pub struct Unary {
    n: usize
}

impl Deserialize for Unary {
    fn deserialize(de: &mut Deserializer) -> Result<Self, DeserializerError> {
        let mut reader = de.bit_reader();

        let mut n = 0;
        loop {
            let bit = reader.read_bit().expect("incorrect bit sequence");

            if bit {
                n += 1;
            } else {
                return Ok(Self { n })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::deserializer::{Deserialize, Deserializer};
    use crate::hashmap::Unary;

    #[test]
    fn unary_zero_test() {
        let input = vec![0];
        let mut deserializer = Deserializer::new(&input);

        let unary = Unary::deserialize(&mut deserializer).unwrap();

        assert_eq!(unary.n, 0);
    }

    #[test]
    fn unary_succ_test() {
        let input = vec![0b111];
        let mut deserializer = Deserializer::new(&input);

        let unary = Unary::deserialize(&mut deserializer).unwrap();

        assert_eq!(unary.n, 3);
    }
}