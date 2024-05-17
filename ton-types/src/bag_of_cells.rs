use crate::cell::Cell;
use crate::deserializer::{Deserialize, DeserializeBare, Deserializer, DeserializerError};

#[derive(Debug, PartialEq, Eq)]
pub enum BagOfCells {
    SerializedBocIdx {
        size: u8,
        off_bytes: u8,
        cells: u32,
        roots: u32,
        absent: u32,
        index: Vec<u8>,
        cell_data: Vec<u8>,
    },
    SerializedBocIdxCrc32 {
        size: u8,
        off_bytes: u8,
        cells: u32,
        roots: u32,
        absent: u32,
        index: Vec<u8>,
        cell_data: Vec<u8>,
        crc32c: u32
    },
    SerializedBoc {
        flags_and_size: u8,
        off_bytes: u8,
        cells: usize,
        roots: u32,
        absent: u32,
        root_list: Vec<u8>,
        index: Option<Vec<u8>>,
        cell_data: Vec<Cell>,
        crc32c: Option<u32>
    }
}

impl BagOfCells {
    pub fn total_cells_size(&self) -> usize {
        match self {
            BagOfCells::SerializedBocIdx { cell_data, .. } => cell_data.len(),
            BagOfCells::SerializedBocIdxCrc32 { cell_data, .. } => cell_data.len(),
            BagOfCells::SerializedBoc { cell_data, .. } => cell_data.len(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            BagOfCells::SerializedBocIdx { cells, .. } => { *cells as usize }
            BagOfCells::SerializedBocIdxCrc32 { cells, .. } => { *cells as usize }
            BagOfCells::SerializedBoc { cell_data, .. } => { cell_data.len() }
        }
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

        Ok(Self::SerializedBocIdx { size, off_bytes, cells, roots, absent, index, cell_data })
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

        Ok(Self::SerializedBocIdxCrc32 { size, off_bytes, cells, roots, absent, index, cell_data, crc32c })
    }
}

impl DeserializeBare<0xb5ee9c72> for BagOfCells {
    fn deserialize_bare(de: &mut Deserializer) -> Result<Self, DeserializerError> {
        let flags_and_size = de.parse_u8()?; // has_idx:(## 1) has_crc32c:(## 1) has_cache_bits:(## 1) flags:(## 2) { flags = 0 } size:(## 3) { size <= 4 }
        let size = flags_and_size & 0b00000111;
        let off_bytes = de.parse_u8()?; // { off_bytes <= 8 }

        let cells_count = de.parse_sized_u32(size as usize)? as usize;
        let roots = de.parse_sized_u32(size as usize)?;
        let absent = de.parse_sized_u32(size as usize)?;
        let _ = de.parse_sized_u64(off_bytes as usize)?;

        let root_list = de.parse_u8_vec(roots as usize * size as usize)?;
        let index = match flags_and_size & 0b10000000 > 0 {
            true => Some(de.parse_u8_vec(cells_count * off_bytes as usize)?),
            false => None
        };

        let mut cells = Vec::with_capacity(cells_count);
        for _ in 0..cells_count {
            let cell = de.parse_cell()?;

            cells.push(cell)
        }
        let crc32c = match flags_and_size & 0b01000000 > 0 {
            true => Some(de.parse_u32()?),
            false => None
        };

        Ok(Self::SerializedBoc { flags_and_size, off_bytes, cells: cells_count, roots, absent, root_list, index, cell_data: cells, crc32c })
    }
}

impl Deserialize for BagOfCells {
    fn deserialize(de: &mut Deserializer) -> Result<Self, DeserializerError> {
        let constructor_number = de.parse_constructor_numer()?;

        match constructor_number {
            0x68ff65f3 => Ok(<BagOfCells as DeserializeBare<0x68ff65f3>>::deserialize_bare(de)?),
            0xacc3a728 => Ok(<BagOfCells as DeserializeBare<0xacc3a728>>::deserialize_bare(de)?),
            0xb5ee9c72 => Ok(<BagOfCells as DeserializeBare<0xb5ee9c72>>::deserialize_bare(de)?),
            _ => Err(DeserializerError::UnexpectedConstructorNumber(constructor_number))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::deserializer::from_bytes;
    use super::*;

    #[test]
    fn boc_deserialize_example_test() {
        let bytes = hex::decode("b5ee9c7201010301000e000201c002010101ff0200060aaaaa").unwrap();

        let boc = from_bytes::<BagOfCells>(&bytes).unwrap();

        assert_eq!(boc, BagOfCells::SerializedBoc {
            flags_and_size: 1,
            off_bytes: 1,
            cells: 3,
            roots: 1,
            absent: 0,
            root_list: vec![0],
            index: None,
            cell_data: vec![
                Cell::new(vec![128], vec![2, 1]),
                Cell::new(vec![254], vec![2]),
                Cell::new(vec![10, 170, 170], vec![]),
            ],
            crc32c: None,
        });
    }

    #[test]
    fn boc_shards_info_deserialize_test() {
        let bytes = hex::decode("b5ee9c7201020701000110000101c0010103d040020201c0030401eb5014c376901214cdb0000152890a35b600000152890a35b85e31d8be7f5f1b44600e445b3cf778b40eaad885db5153838bea3e8f0f4a9b25e36422b74bfadf372f7d3e16b48c05f4866b05d2c7e5787bd954a5d79ad9fdb6990000450f5a00000000000000001214cd933228b81ccc8a2e52000000c90501db5014c367381214cda8000152890aafc800000152890aafcefff0db0738592205986066e14fa1221d28f0156604fd4346cea0b705712ddd2872d9dc6b6fd4eb6624bf6cb9b77d673d2df07a993f5ed281b375f3c659c25e4df80000450f5e00000000000000001214cd933228b8020600134591048ab20ee6b28020001343332bfa820ee6b28020").unwrap();

        let boc = from_bytes::<BagOfCells>(&bytes).unwrap();

        println!("{:?}", boc);

        assert_eq!(boc.len(), 7)
    }
}
