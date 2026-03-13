use crate::tlb::crypto_signature::CryptoSignaturePair;
use crate::tlb::in_msg::InMsg;
use crate::tlb::shard_hashes::ShardHashes;
use std::collections::HashMap;
use toner::tlb::bits::bitvec::order::Msb0;
use toner::tlb::bits::bitvec::vec::BitVec;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::bits::NBits;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::hashmap::aug::HashmapAugE;
use toner::tlb::hashmap::HashmapE;
use toner::tlb::{Cell, Error, Ref, Same};
use toner::ton::currency::CurrencyCollection;

/// ```tlb
/// _ fees:CurrencyCollection create:CurrencyCollection = ShardFeeCreated;
/// ```
#[derive(Debug, Clone)]
pub struct ShardFeeCreated {
    pub fees: CurrencyCollection,
    pub create: CurrencyCollection,
}

impl<'de> CellDeserialize<'de> for ShardFeeCreated {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let fees = parser.parse(())?;
        let create = parser.parse(())?;

        Ok(Self { fees, create })
    }
}

/// ```tlb
/// _ (HashmapAugE 96 ShardFeeCreated ShardFeeCreated) = ShardFees;
/// ```
pub type ShardFees = HashmapAugE<ShardFeeCreated, ShardFeeCreated>;

/// ```tlb
/// masterchain_block_extra#cca5
///   key_block:(## 1)
///   shard_hashes:ShardHashes
///   shard_fees:ShardFees
///   ^[ prev_blk_signatures:(HashmapE 16 CryptoSignaturePair)
///      recover_create_msg:(Maybe ^InMsg)
///      mint_msg:(Maybe ^InMsg) ]
///   config:key_block?ConfigParams
/// = McBlockExtra;
/// ```
#[derive(Debug)]
pub struct McBlockExtra {
    pub key_block: bool,
    pub shard_hashes: ShardHashes,
    pub shard_fees: ShardFees,
    pub prev_blk_signatures: HashMap<BitVec<u8, Msb0>, CryptoSignaturePair>,
    pub recover_create_msg: Option<InMsg>,
    pub mint_msg: Option<InMsg>,
    pub config: Option<Cell>,
}

impl<'de> CellDeserialize<'de> for McBlockExtra {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u16 = parser.unpack_as::<_, NBits<16>>(())?;
        if tag != 0xcca5 {
            return Err(Error::custom(format!(
                "invalid McBlockExtra tag: 0x{:04x}",
                tag
            )));
        }

        let key_block: u8 = parser.unpack_as::<_, NBits<1>>(())?;
        let key_block = key_block != 0;
        let shard_hashes: ShardHashes = parser.parse(())?;
        let shard_fees: ShardFees = parser.parse_as::<_, HashmapAugE<Same, Same>>((96, (), ()))?;

        let (prev_blk_signatures, recover_create_msg, mint_msg): (
            HashMap<BitVec<u8, Msb0>, CryptoSignaturePair>,
            Option<InMsg>,
            Option<InMsg>,
        ) = parser.parse_as::<_, Ref<(HashmapE<Same, ()>, Option<Ref>, Option<Ref>)>>((
            (16, ()),
            (),
            (),
        ))?;

        let config = if key_block {
            Some(parser.parse(())?)
        } else {
            None
        };

        parser.ensure_empty()?;

        Ok(Self {
            key_block,
            shard_hashes,
            shard_fees,
            prev_blk_signatures,
            recover_create_msg,
            mint_msg,
            config,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::tlb::block::Block;
    use crate::tlb::merkle_update::tests::BLOCK_HEX;
    use toner::tlb::bits::de::unpack_bytes;
    use toner::tlb::BoC;

    #[test]
    fn test_mc_block_extra_parse_ok() {
        let data = hex::decode(BLOCK_HEX).unwrap();
        let boc: BoC = unpack_bytes(&data, ()).unwrap();
        let root = boc.single_root().unwrap();

        let block: Block = root.parse_fully(()).unwrap();

        let mc_extra = block.extra.custom.unwrap();
        assert!(!mc_extra.shard_hashes.is_empty());
    }
}
