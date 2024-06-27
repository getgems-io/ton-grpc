use std::collections::HashMap;
use std::ops::Deref;
use toner::tlb::bits::bitvec::field::BitField;
use toner::tlb::bits::bitvec::order::Msb0;
use toner::tlb::bits::bitvec::vec::BitVec;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::r#as::{NoArgs, ParseFully, Ref};
use toner::ton::bin_tree::BinTree;
use toner::ton::hashmap::HashmapE;
use crate::tlb::shard_descr::ShardDescr;

/// ```tlb
/// _ (HashmapE 32 ^(BinTree ShardDescr)) = ShardHashes;
/// ```
/// NOTE[akosyulev0]: next_validator_shard == shard_id
#[derive(Debug)]
pub struct ShardHashes(HashMap<u32, Vec<ShardDescr>>);

impl Deref for ShardHashes {
    type Target = HashMap<u32, Vec<ShardDescr>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de> CellDeserialize<'de> for ShardHashes {
    fn parse(parser: &mut CellParser<'de>) -> Result<Self, CellParserError<'de>> {
        let hashmap = parser.parse_as_with::<
            HashMap<BitVec<u8, Msb0>, _>,
            HashmapE<Ref<ParseFully<BinTree<NoArgs<_>>>>, ()>
        >((32, ()))?;

        let inner = hashmap
            .into_iter()
            .map(|(k, v)|
                (k.load_be::<u32>(), v)
            )
            .collect();

        Ok(Self(inner))
    }
}

#[cfg(test)]
mod tests {
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::bitvec::vec::BitVec;
    use toner::tlb::bits::de::unpack_fully;
    use toner::ton::boc::BoC;
    use crate::tlb::shard_hashes::ShardHashes;

    #[test]
    fn parse_shard_hashes() {
        let packed = hex::decode("b5ee9c7201020d0100020c000101c0010103d040020201c003040201c005060201c0090a01db5014f0a6c8123be8880001559e44ca1a000001559e44ca1a3cc1d224aa5b9f1e6610d94e89e37decdb0d75981a5646e0a7e0c099461abacf307c8d69b412105ec8734aea8b926d380f91ff42c7e4f61cf731b2e9ff500913d00000460d810000000000000000123be87b3319d9020701db5014f07dc8123be8880001559e43d5f6000001559e43d5f7bca2dd37526cdc93834ae03666706139de4812cb71ff5d384506cb8a7e933e1fd04e3511e9949ecffba9f6b530e7c43182c325e25daad18d303adaccf4a315b8400000460d830000000000000000123be8733319d8d208001344d69059b2165a0bc02000134394054c02077359402001db5014f09b18123be8880001559e44ca1a000001559e44ca1b8cbe3ea21e6a78ccdb3e0a76f292fdf5c8580a40ea7f61004cdcb7b0fdfa2f78210ad6cdda8f5fd6b1c7678dae076bc87e7d2c4da65a0cc64a08c7db7e081b23600000460da50000000000000000123be87b3319d9020b01db5014f0a2f8123be8880001559e45442c000001559e45442c29bd15b1b5f524b85b1d91d21994dc39d8bee1a70831ac069dc00db0421e1e1e5b56542ec60ee32e6f66d846e736e92f450766e79d002c476077a0848f223599080000460d970000000000000000123be87b3319d8ea0c001346728c8162165a0bc0200013429cd691720ee6b28020").unwrap();
        let bit_packed:BitVec<u8, Msb0> = BitVec::from_vec(packed);
        let boc: BoC = unpack_fully(&bit_packed).unwrap();
        let root = boc.single_root().unwrap();

        let actual: ShardHashes = root.parse_fully().unwrap();

        assert_eq!(1, actual.len());
        assert!(actual.contains_key(&0));
        assert_eq!(4, actual.get(&0).unwrap().len());
    }
}
