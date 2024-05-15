use bytes::Buf;

#[derive(Debug, PartialEq, Eq)]
pub struct Cell {
    content: Vec<u8>,
    refs: Vec<u8>
}

impl Cell {
    pub fn deserialize(mut data: &[u8]) -> Self {
        let refs_descriptor = data.get_u8();
        let bits_descriptor = data.get_u8();

        let refs_count = refs_descriptor & 0b00000111;

        // bits_descriptor is the number of 4-bit groups in content,
        // so we need to divide it by 2 to get the number of bytes in content,
        // but we also need to add 1 if bits_descriptor is odd
        let len = ((bits_descriptor / 2) + (bits_descriptor % 2)) as usize;

        let mut content = vec![0; len];
        data.copy_to_slice(&mut content);

        // if bits_descriptor is odd, we need to clear the least significant bit of the last byte in content
        if bits_descriptor % 2 > 0 {
            content[len - 1] = content[len - 1] & (content[len - 1] - 1);
        }

        let mut refs = vec![0; refs_count as usize];
        data.copy_to_slice(&mut refs);

        Cell { content, refs }
    }
}


#[cfg(test)]
mod test {
    use crate::cell::Cell;

    #[test]
    fn cell_test() {
        let data = hex::decode("0201C00201").unwrap();

        let cell = Cell::deserialize(&data);

        println!("{:?}", cell);

        assert_eq!(cell, Cell { content: vec![0b10000000], refs: vec![2, 1] })
    }
}
