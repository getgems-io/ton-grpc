use adnl_tcp::types::Int256;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};
use toner::tlb::bits::NBits;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};

/// ```tlb
/// !pruned_branch#01 {level:#[1..3]} {hashes:level * bits256} {depths:level * uint16} = PrunedBranch;
/// ```
#[derive(Debug, Clone)]
pub struct PrunedBranch {
    pub level: u8,
    pub hashes: Vec<Int256>,
    pub depths: Vec<u16>,
}

impl<'de> BitUnpack<'de> for PrunedBranch {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let tag: u8 = reader.unpack_as::<_, NBits<8>>(())?;
        if tag != 0x01 {
            unreachable!("invalid pruned cell tag: {:02x}", tag);
        }

        let level: u8 = reader.unpack_as::<_, NBits<8>>(())?;
        let mut hashes = Vec::with_capacity(level as usize);
        for _ in 0..level {
            hashes.push(reader.unpack(())?);
        }
        let mut depths = Vec::with_capacity(level as usize);
        for _ in 0..level {
            depths.push(reader.unpack(())?);
        }

        Ok(Self {
            level,
            hashes,
            depths,
        })
    }
}

impl<'de> CellDeserialize<'de> for PrunedBranch {
    type Args = ();

    fn parse(parser: &mut CellParser<'de>, args: Self::Args) -> Result<Self, CellParserError<'de>> {
        Ok(parser.unpack(args)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toner::tlb::bits::bitvec::vec::BitVec;

    #[test]
    fn test_parse_pruned_cell() {
        // !pruned_branch#01 {level:#[1..3]} {hashes:level * bits256} {depths:level * uint16}
        // tag: 0x01
        // level: 0x01
        // hash: 32 bytes of 0xAA
        // depth: 0x0005 (u16)
        let mut data = vec![0x01, 0x01];
        data.extend_from_slice(&[0xAA; 32]);
        data.extend_from_slice(&[0x00, 0x05]);

        let bit_packed: BitVec<u8, toner::tlb::bits::bitvec::order::Msb0> = BitVec::from_vec(data);
        let mut reader = bit_packed.as_bitslice();
        let pruned: PrunedBranch = reader.unpack(()).unwrap();

        assert_eq!(pruned.level, 1);
        assert_eq!(pruned.hashes.len(), 1);
        assert_eq!(pruned.depths.len(), 1);
        assert_eq!(pruned.depths[0], 5);
        assert_eq!(pruned.hashes[0], [0xAA; 32]);
    }

    #[test]
    fn test_parse_pruned_cell_level_2() {
        // tag: 0x01
        // level: 0x02
        // hashes: 32 bytes of 0xAA, 32 bytes of 0xBB
        // depths: 0x0005, 0x0007
        let mut data = vec![0x01, 0x02];
        data.extend_from_slice(&[0xAA; 32]);
        data.extend_from_slice(&[0xBB; 32]);
        data.extend_from_slice(&[0x00, 0x05]);
        data.extend_from_slice(&[0x00, 0x07]);

        let bit_packed: BitVec<u8, toner::tlb::bits::bitvec::order::Msb0> = BitVec::from_vec(data);
        let mut reader = bit_packed.as_bitslice();
        let pruned: PrunedBranch = reader.unpack(()).unwrap();

        assert_eq!(pruned.level, 2);
        assert_eq!(pruned.hashes.len(), 2);
        assert_eq!(pruned.depths.len(), 2);
        assert_eq!(pruned.hashes[0], [0xAA; 32]);
        assert_eq!(pruned.hashes[1], [0xBB; 32]);
        assert_eq!(pruned.depths[0], 5);
        assert_eq!(pruned.depths[1], 7);
    }
}
