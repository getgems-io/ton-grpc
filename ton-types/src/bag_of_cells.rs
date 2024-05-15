use crate::deserializer::{Deserialize, DeserializeBare, Deserializer, DeserializerError};

#[derive(Debug, PartialEq, Eq)]
pub enum BagOfCells {
    SerializedBocIdx {
        size: u8,
        off_bytes: u8,
        cells: u32,
        roots: u32,
        absent: u32,
        tot_cells_size: u64,
        index: Vec<u8>,
        cell_data: Vec<u8>,
    },
    SerializedBocIdxCrc32 {
        size: u8,
        off_bytes: u8,
        cells: u32,
        roots: u32,
        absent: u32,
        tot_cells_size: u64,
        index: Vec<u8>,
        cell_data: Vec<u8>,
        crc32c: u32
    },
    SerializedBoc {
        flags_and_size: u8,
        off_bytes: u8,
        cells: u32,
        roots: u32,
        absent: u32,
        tot_cells_size: u64,
        root_list: Vec<u8>,
        index: Option<Vec<u8>>,
        cell_data: Vec<u8>,
        crc32c: Option<u32>
    }
}

impl DeserializeBare<0x68ff65f3> for BagOfCells {
    fn deserialize_bare(de: &mut Deserializer) -> Result<Self, DeserializerError> {
        let size = de.parse_u8()?; // { size <= 4 }
        let off_bytes = de.parse_u8()?; // { off_bytes <= 8 }

        let cells = de.parse_sized_u32(size as usize)?;
        let roots = de.parse_sized_u32(size as usize)?; // { roots = 1 }
        let absent = de.parse_sized_u32(size as usize)?; // { roots + absent <= cells }
        let tot_cells_size = de.parse_sized_u64(off_bytes as usize)?;

        let index = de.parse_u8_vec((off_bytes as u32 * cells) as usize)?;
        let cell_data = de.parse_u8_vec(tot_cells_size as usize)?;

        Ok(Self::SerializedBocIdx { size, off_bytes, cells, roots, absent, tot_cells_size, index, cell_data })
    }
}

impl DeserializeBare<0xacc3a728> for BagOfCells {
    fn deserialize_bare(de: &mut Deserializer) -> Result<Self, DeserializerError> {
        let size = de.parse_u8()?; // { size <= 4 }
        let off_bytes = de.parse_u8()?; // { off_bytes <= 8 }

        let cells = de.parse_sized_u32(size as usize)?;
        let roots = de.parse_sized_u32(size as usize)?; // { roots = 1 }
        let absent = de.parse_sized_u32(size as usize)?; // { roots + absent <= cells }
        let tot_cells_size = de.parse_sized_u64(off_bytes as usize)?;

        let index = de.parse_u8_vec((off_bytes as u32 * cells) as usize)?;
        let cell_data = de.parse_u8_vec(tot_cells_size as usize)?;

        let crc32c = de.parse_u32()?;

        Ok(Self::SerializedBocIdxCrc32 { size, off_bytes, cells, roots, absent, tot_cells_size, index, cell_data, crc32c })
    }
}

impl DeserializeBare<0xb5ee9c72> for BagOfCells {
    fn deserialize_bare(de: &mut Deserializer) -> Result<Self, DeserializerError> {
        let size_and_flags = de.parse_u8()?; // has_idx:(## 1) has_crc32c:(## 1) has_cache_bits:(## 1) flags:(## 2) { flags = 0 } size:(## 3) { size <= 4 }
        let size = size_and_flags & 0b00000111;
        let off_bytes = de.parse_u8()?; // { off_bytes <= 8 }

        let cells = de.parse_sized_u32(size as usize)?;
        let roots = de.parse_sized_u32(size as usize)?;
        let absent = de.parse_sized_u32(size as usize)?;
        let tot_cells_size = de.parse_sized_u64(off_bytes as usize)?;

        let root_list = de.parse_u8_vec(roots as usize * size as usize)?;
        let index = match size_and_flags & 0b10000000 > 0 {
            true => Some(de.parse_u8_vec(cells as usize * off_bytes as usize)?),
            false => None
        };

        let cell_data = de.parse_u8_vec(tot_cells_size as usize)?;
        let crc32c = match size_and_flags & 0b01000000 > 0 {
            true => Some(de.parse_u32()?),
            false => None
        };

        Ok(Self::SerializedBoc { flags_and_size: size_and_flags, off_bytes, cells, roots, absent, tot_cells_size, root_list, index, cell_data, crc32c })
    }
}

impl Deserialize for BagOfCells {
    fn deserialize(de: &mut Deserializer) -> Result<Self, DeserializerError> {
        let constructor_number = de.parse_constructor_numer()?;

        match constructor_number {
            0x68ff65f3 => {
                Ok(<BagOfCells as DeserializeBare<0x68ff65f3>>::deserialize_bare(de)?)
            },
            0xacc3a728 => {
                Ok(<BagOfCells as DeserializeBare<0xacc3a728>>::deserialize_bare(de)?)
            },
            0xb5ee9c72 => {
                Ok(<BagOfCells as DeserializeBare<0xb5ee9c72>>::deserialize_bare(de)?)
            },
            _ => Err(DeserializerError::UnexpectedConstructorNumber(constructor_number))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::deserializer::Deserialize;
    use super::*;

    #[test]
    fn boc_deserialize() {
        let bytes = hex::decode("b5ee9c7201010301000e000201c002010101ff0200060aaaaa").unwrap();
        let mut de = Deserializer::new(&bytes);

        let boc = BagOfCells::deserialize(&mut de).unwrap();

        assert_eq!(boc, BagOfCells::SerializedBoc {
            flags_and_size: 1,
            off_bytes: 1,
            cells: 3,
            roots: 1,
            absent: 0,
            tot_cells_size: 14,
            root_list: vec![0],
            index: None,
            cell_data: hex::decode("0201c002010101ff0200060aaaaa").unwrap(),
            crc32c: None,
        });
    }
}
