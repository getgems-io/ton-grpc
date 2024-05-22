use std::fmt::{Debug, Formatter};
use crate::cell::Cell;
use crate::deserializer::{Deserialize, DeserializeBare, Deserializer, DeserializerError};

#[derive(Debug, PartialEq, Eq)]
pub struct BagOfCells {
    cells: Vec<Cell>
}

impl BagOfCells {
    pub fn root(&self) -> Option<CellInBag> {
        self.get(0)
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn get(&self, node_id: usize) -> Option<CellInBag> {
        self.cells.get(node_id)
            .map(|cell| CellInBag { cell, bag: self })
    }
}

pub struct CellInBag<'a> {
    cell: &'a Cell,
    bag: &'a BagOfCells,
}

impl<'a> AsRef<[u8]> for CellInBag<'a> {
    fn as_ref(&self) -> &[u8] {
        &self.cell.content
    }
}

impl Debug for CellInBag<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CellInBag")
            .field("cell", &self.cell)
        .finish()
    }
}

impl<'a> CellInBag<'a> {
    pub fn children(&self) -> impl Iterator<Item=CellInBag<'a>> {
        self.cell.refs()
            .iter()
            .filter_map(|node_id| self.bag.get(*node_id as usize))
    }
}

impl DeserializeBare<0x68ff65f3> for BagOfCells {
    fn deserialize_bare(de: &mut Deserializer) -> Result<Self, DeserializerError> {
        let size = de.parse_u8()?; // { size <= 4 }
        let off_bytes = de.parse_u8()?; // { off_bytes <= 8 }

        let cells_count = de.parse_sized_u32(size as usize)?;
        let _ = de.parse_sized_u32(size as usize)?; // { roots = 1 }
        let _ = de.parse_sized_u32(size as usize)?; // { roots + absent <= cells }
        let _ = de.parse_sized_u64(off_bytes as usize)?;

        // TODO[akostylev0]
        let _index = de.parse_u8_vec((off_bytes as u32 * cells_count) as usize)?;

        let cells = (0..cells_count)
            .map(|_| de.parse_cell())
            .collect::<Result<Vec<_>, DeserializerError>>()?;

        Ok(Self { cells })
    }
}

impl DeserializeBare<0xacc3a728> for BagOfCells {
    fn deserialize_bare(de: &mut Deserializer) -> Result<Self, DeserializerError> {
        let size = de.parse_u8()?; // { size <= 4 }
        let off_bytes = de.parse_u8()?; // { off_bytes <= 8 }

        let cells_count = de.parse_sized_u32(size as usize)?;
        let _ = de.parse_sized_u32(size as usize)?; // { roots = 1 }
        let _ = de.parse_sized_u32(size as usize)?; // { roots + absent <= cells }
        let _ = de.parse_sized_u64(off_bytes as usize)?;

        // TODO[akostylev0]
        let _index = de.parse_u8_vec((off_bytes as u32 * cells_count) as usize)?;

        let cells = (0..cells_count)
            .map(|_| de.parse_cell())
            .collect::<Result<Vec<_>, DeserializerError>>()?;

        // TODO[akostylev0] verify crc32
        let _crc32c = de.parse_u32()?;

        Ok(Self { cells })
    }
}

impl DeserializeBare<0xb5ee9c72> for BagOfCells {
    fn deserialize_bare(de: &mut Deserializer) -> Result<Self, DeserializerError> {
        let flags_and_size = de.parse_u8()?; // has_idx:(## 1) has_crc32c:(## 1) has_cache_bits:(## 1) flags:(## 2) { flags = 0 } size:(## 3) { size <= 4 }
        let size = flags_and_size & 0b00000111;
        let off_bytes = de.parse_u8()?; // { off_bytes <= 8 }

        let cells_count = de.parse_sized_u32(size as usize)? as usize;
        let _ = de.parse_sized_u32(size as usize)?;
        let _ = de.parse_sized_u32(size as usize)?;
        let _ = de.parse_sized_u64(off_bytes as usize)?;

        let _ = de.parse_u8_vec(size as usize)?;

        // TODO[akostylev0]
        let _index = match flags_and_size & 0b10000000 > 0 {
            true => Some(de.parse_u8_vec(cells_count * off_bytes as usize)?),
            false => None
        };

        let cells = (0..cells_count)
            .map(|_| de.parse_cell())
            .collect::<Result<Vec<_>, DeserializerError>>()?;

        // TODO[akostylev0] verify crc32
        let _crc32c = match flags_and_size & 0b01000000 > 0 {
            true => Some(de.parse_u32()?),
            false => None
        };

        Ok(Self { cells })
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

        assert_eq!(boc, BagOfCells { cells: vec![
            Cell::new(vec![128], vec![2, 1]),
            Cell::new(vec![254], vec![2]),
            Cell::new(vec![10, 170, 170], vec![]),
        ]});
    }

    #[test]
    fn boc_shards_info_deserialize_test() {
        let bytes = hex::decode("b5ee9c7201020701000110000101c0010103d040020201c0030401eb5014c376901214cdb0000152890a35b600000152890a35b85e31d8be7f5f1b44600e445b3cf778b40eaad885db5153838bea3e8f0f4a9b25e36422b74bfadf372f7d3e16b48c05f4866b05d2c7e5787bd954a5d79ad9fdb6990000450f5a00000000000000001214cd933228b81ccc8a2e52000000c90501db5014c367381214cda8000152890aafc800000152890aafcefff0db0738592205986066e14fa1221d28f0156604fd4346cea0b705712ddd2872d9dc6b6fd4eb6624bf6cb9b77d673d2df07a993f5ed281b375f3c659c25e4df80000450f5e00000000000000001214cd933228b8020600134591048ab20ee6b28020001343332bfa820ee6b28020").unwrap();
        let boc = from_bytes::<BagOfCells>(&bytes).unwrap();

        let root = boc.root().unwrap();
        for child in root.children() {
            println!("{:?}", &child)
        }

        assert_eq!(boc.len(), 7)
    }
}
