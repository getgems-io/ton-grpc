use std::marker::PhantomData;

use toner::tlb::bits::bitvec::order::Msb0;
use toner::tlb::bits::bitvec::slice::BitSlice;
use toner::tlb::bits::bitvec::store::BitStore;
use toner::tlb::bits::bitvec::vec::BitVec;
use toner::tlb::bits::bitvec::view::BitView;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpackAs};
use toner::tlb::bits::ser::{BitPackAs, BitWriter, BitWriterExt};
use toner::tlb::de::{CellDeserialize, CellDeserializeAs, CellParser, CellParserError};
use toner::tlb::ser::{CellBuilder, CellBuilderError, CellSerialize, CellSerializeAs};
use toner::tlb::{Cell, Error, ParseFully, Ref, Same};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashmapAugE<K, V, const BITS: usize, E = ()> {
    pub hashmap: HashmapE<K, V, BITS, E>,
    pub extra: E,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum HashmapE<K, V, const BITS: usize, E = ()> {
    #[default]
    Empty,
    Root(Hashmap<K, V, BITS, E>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hashmap<K, V, const BITS: usize, E = ()> {
    pub tree: HashmapTree<V, E>,
    key: PhantomData<fn() -> K>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashmapTree<V, E = ()> {
    Edge {
        prefix: BitVec<u8, Msb0>,
        node: HashmapAugNode<V, E>,
    },
    Pruned(Cell),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashmapAugNode<V, E = ()> {
    pub extra: E,
    pub node: HashmapNode<V, E>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashmapNode<V, E = ()> {
    Leaf(V),
    Fork([Box<HashmapTree<V, E>>; 2]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashmapLookup<'a, V> {
    Found(&'a V),
    Absent,
    Pruned(&'a Cell),
}

impl<K, V, const BITS: usize, E> Hashmap<K, V, BITS, E> {
    pub const fn new(tree: HashmapTree<V, E>) -> Self {
        Self {
            tree,
            key: PhantomData,
        }
    }

    pub fn lookup(&self, key: &K) -> HashmapLookup<'_, V>
    where
        K: BitView,
    {
        let bits = key.view_bits::<Msb0>();
        if BITS > bits.len() {
            return HashmapLookup::Absent;
        }
        self.tree.lookup_bits(&bits[bits.len() - BITS..])
    }
}

impl<K, V, const BITS: usize, E> HashmapE<K, V, BITS, E> {
    pub const fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    pub fn lookup(&self, key: &K) -> HashmapLookup<'_, V>
    where
        K: BitView,
    {
        match self {
            Self::Empty => HashmapLookup::Absent,
            Self::Root(root) => root.lookup(key),
        }
    }
}

impl<V, E> HashmapTree<V, E> {
    fn lookup_bits<S>(&self, key: &BitSlice<S, Msb0>) -> HashmapLookup<'_, V>
    where
        S: BitStore,
    {
        match self {
            Self::Pruned(cell) => HashmapLookup::Pruned(cell),
            Self::Edge { prefix, node } => {
                if key.len() < prefix.len()
                    || !prefix
                        .iter()
                        .by_vals()
                        .eq(key[..prefix.len()].iter().by_vals())
                {
                    return HashmapLookup::Absent;
                }
                node.lookup_bits(&key[prefix.len()..])
            }
        }
    }
}

impl<V, E> HashmapAugNode<V, E> {
    fn lookup_bits<S>(&self, key: &BitSlice<S, Msb0>) -> HashmapLookup<'_, V>
    where
        S: BitStore,
    {
        self.node.lookup_bits(key)
    }
}

impl<V, E> HashmapNode<V, E> {
    fn lookup_bits<S>(&self, key: &BitSlice<S, Msb0>) -> HashmapLookup<'_, V>
    where
        S: BitStore,
    {
        match self {
            Self::Leaf(value) if key.is_empty() => HashmapLookup::Found(value),
            Self::Fork([left, right]) => key
                .split_first()
                .map_or(HashmapLookup::Absent, |(is_right, key)| {
                    if *is_right { right } else { left }.lookup_bits(key)
                }),
            _ => HashmapLookup::Absent,
        }
    }
}

impl<K, V, AsV, const BITS: usize, E, AsE> CellSerializeAs<HashmapAugE<K, V, BITS, E>>
    for HashmapAugE<K, AsV, BITS, AsE>
where
    AsV: CellSerializeAs<V>,
    AsV::Args: Clone,
    AsE: CellSerializeAs<E>,
    AsE::Args: Clone,
{
    type Args = (AsV::Args, AsE::Args);

    fn store_as(
        source: &HashmapAugE<K, V, BITS, E>,
        builder: &mut CellBuilder,
        (value_args, extra_args): Self::Args,
    ) -> Result<(), CellBuilderError> {
        builder
            .store_as::<_, &HashmapE<K, AsV, BITS, AsE>>(
                &source.hashmap,
                (value_args, extra_args.clone()),
            )?
            .store_as::<_, &AsE>(&source.extra, extra_args)?;
        Ok(())
    }
}

impl<'de, K, V, AsV, const BITS: usize, E, AsE> CellDeserializeAs<'de, HashmapAugE<K, V, BITS, E>>
    for HashmapAugE<K, AsV, BITS, AsE>
where
    AsV: CellDeserializeAs<'de, V>,
    AsV::Args: Clone,
    AsE: CellDeserializeAs<'de, E>,
    AsE::Args: Clone,
{
    type Args = (AsV::Args, AsE::Args);

    fn parse_as(
        parser: &mut CellParser<'de>,
        (value_args, extra_args): Self::Args,
    ) -> Result<HashmapAugE<K, V, BITS, E>, CellParserError<'de>> {
        Ok(HashmapAugE {
            hashmap: parser
                .parse_as::<_, HashmapE<K, AsV, BITS, AsE>>((value_args, extra_args.clone()))?,
            extra: parser.parse_as::<_, AsE>(extra_args)?,
        })
    }
}

impl<K, V, AsV, const BITS: usize, E, AsE> CellSerializeAs<HashmapE<K, V, BITS, E>>
    for HashmapE<K, AsV, BITS, AsE>
where
    AsV: CellSerializeAs<V>,
    AsV::Args: Clone,
    AsE: CellSerializeAs<E>,
    AsE::Args: Clone,
{
    type Args = (AsV::Args, AsE::Args);

    fn store_as(
        source: &HashmapE<K, V, BITS, E>,
        builder: &mut CellBuilder,
        args: Self::Args,
    ) -> Result<(), CellBuilderError> {
        match source {
            HashmapE::Empty => {
                builder.pack(false, ())?;
            }
            HashmapE::Root(root) => {
                builder
                    .pack(true, ())?
                    .store_as::<_, Ref<&Hashmap<K, AsV, BITS, AsE>>>(root, args)?;
            }
        }
        Ok(())
    }
}

impl<'de, K, V, AsV, const BITS: usize, E, AsE> CellDeserializeAs<'de, HashmapE<K, V, BITS, E>>
    for HashmapE<K, AsV, BITS, AsE>
where
    AsV: CellDeserializeAs<'de, V>,
    AsV::Args: Clone,
    AsE: CellDeserializeAs<'de, E>,
    AsE::Args: Clone,
{
    type Args = (AsV::Args, AsE::Args);

    fn parse_as(
        parser: &mut CellParser<'de>,
        args: Self::Args,
    ) -> Result<HashmapE<K, V, BITS, E>, CellParserError<'de>> {
        Ok(if parser.unpack(())? {
            HashmapE::Root(parser.parse_as::<_, Ref<ParseFully<Hashmap<K, AsV, BITS, AsE>>>>(args)?)
        } else {
            HashmapE::Empty
        })
    }
}

impl<K, V, AsV, const BITS: usize, E, AsE> CellSerializeAs<Hashmap<K, V, BITS, E>>
    for Hashmap<K, AsV, BITS, AsE>
where
    AsV: CellSerializeAs<V>,
    AsV::Args: Clone,
    AsE: CellSerializeAs<E>,
    AsE::Args: Clone,
{
    type Args = (AsV::Args, AsE::Args);

    fn store_as(
        source: &Hashmap<K, V, BITS, E>,
        builder: &mut CellBuilder,
        (value_args, extra_args): Self::Args,
    ) -> Result<(), CellBuilderError> {
        let n =
            u32::try_from(BITS).map_err(|_| CellBuilderError::custom("hashmap key is too wide"))?;
        builder.store_as::<_, &HashmapTree<AsV, AsE>>(&source.tree, (n, value_args, extra_args))?;
        Ok(())
    }
}

impl<'de, K, V, AsV, const BITS: usize, E, AsE> CellDeserializeAs<'de, Hashmap<K, V, BITS, E>>
    for Hashmap<K, AsV, BITS, AsE>
where
    AsV: CellDeserializeAs<'de, V>,
    AsV::Args: Clone,
    AsE: CellDeserializeAs<'de, E>,
    AsE::Args: Clone,
{
    type Args = (AsV::Args, AsE::Args);

    fn parse_as(
        parser: &mut CellParser<'de>,
        (value_args, extra_args): Self::Args,
    ) -> Result<Hashmap<K, V, BITS, E>, CellParserError<'de>> {
        let n =
            u32::try_from(BITS).map_err(|_| CellParserError::custom("hashmap key is too wide"))?;
        Ok(Hashmap::new(parser.parse_as::<_, HashmapTree<AsV, AsE>>(
            (n, value_args, extra_args),
        )?))
    }
}

impl<K, V, const BITS: usize, E> CellSerialize for Hashmap<K, V, BITS, E>
where
    V: CellSerialize,
    V::Args: Clone,
    E: CellSerialize,
    E::Args: Clone,
{
    type Args = (V::Args, E::Args);

    fn store(&self, builder: &mut CellBuilder, args: Self::Args) -> Result<(), CellBuilderError> {
        builder.store_as::<_, Same>(self, args)?;
        Ok(())
    }
}

impl<'de, K, V, const BITS: usize, E> CellDeserialize<'de> for Hashmap<K, V, BITS, E>
where
    V: CellDeserialize<'de>,
    V::Args: Clone,
    E: CellDeserialize<'de>,
    E::Args: Clone,
{
    type Args = (V::Args, E::Args);

    fn parse(parser: &mut CellParser<'de>, args: Self::Args) -> Result<Self, CellParserError<'de>> {
        parser.parse_as::<_, Same>(args)
    }
}

impl<V, AsV, E, AsE> CellSerializeAs<HashmapTree<V, E>> for HashmapTree<AsV, AsE>
where
    AsV: CellSerializeAs<V>,
    AsV::Args: Clone,
    AsE: CellSerializeAs<E>,
    AsE::Args: Clone,
{
    type Args = (u32, AsV::Args, AsE::Args);

    fn store_as(
        source: &HashmapTree<V, E>,
        builder: &mut CellBuilder,
        (n, value_args, extra_args): Self::Args,
    ) -> Result<(), CellBuilderError> {
        match source {
            HashmapTree::Pruned(cell) => cell.store(builder, ()),
            HashmapTree::Edge { prefix, node } => {
                let m = n.checked_sub(prefix.len() as u32).ok_or_else(|| {
                    CellBuilderError::custom("hashmap prefix exceeds remaining key")
                })?;
                builder
                    .pack_as::<_, &HmLabel>(prefix.as_bitslice(), n)?
                    .store_as::<_, &HashmapAugNode<AsV, AsE>>(node, (m, value_args, extra_args))?;
                Ok(())
            }
        }
    }
}

impl<'de, V, AsV, E, AsE> CellDeserializeAs<'de, HashmapTree<V, E>> for HashmapTree<AsV, AsE>
where
    AsV: CellDeserializeAs<'de, V>,
    AsV::Args: Clone,
    AsE: CellDeserializeAs<'de, E>,
    AsE::Args: Clone,
{
    type Args = (u32, AsV::Args, AsE::Args);

    fn parse_as(
        parser: &mut CellParser<'de>,
        (n, value_args, extra_args): Self::Args,
    ) -> Result<HashmapTree<V, E>, CellParserError<'de>> {
        if parser.is_exotic() {
            let cell: Cell = parser.parse(())?;
            validate_pruned_branch(&cell)?;
            return Ok(HashmapTree::Pruned(cell));
        }
        let prefix: BitVec<u8, Msb0> = parser.unpack_as::<_, HmLabel>(n)?;
        let m = n
            .checked_sub(prefix.len() as u32)
            .ok_or_else(|| CellParserError::custom("hashmap prefix exceeds remaining key"))?;
        Ok(HashmapTree::Edge {
            prefix,
            node: parser.parse_as::<_, HashmapAugNode<AsV, AsE>>((m, value_args, extra_args))?,
        })
    }
}

impl<V, AsV, E, AsE> CellSerializeAs<HashmapAugNode<V, E>> for HashmapAugNode<AsV, AsE>
where
    AsV: CellSerializeAs<V>,
    AsV::Args: Clone,
    AsE: CellSerializeAs<E>,
    AsE::Args: Clone,
{
    type Args = (u32, AsV::Args, AsE::Args);

    fn store_as(
        source: &HashmapAugNode<V, E>,
        builder: &mut CellBuilder,
        (n, value_args, extra_args): Self::Args,
    ) -> Result<(), CellBuilderError> {
        builder
            .store_as::<_, &AsE>(&source.extra, extra_args.clone())?
            .store_as::<_, &HashmapNode<AsV, AsE>>(&source.node, (n, value_args, extra_args))?;
        Ok(())
    }
}

impl<'de, V, AsV, E, AsE> CellDeserializeAs<'de, HashmapAugNode<V, E>> for HashmapAugNode<AsV, AsE>
where
    AsV: CellDeserializeAs<'de, V>,
    AsV::Args: Clone,
    AsE: CellDeserializeAs<'de, E>,
    AsE::Args: Clone,
{
    type Args = (u32, AsV::Args, AsE::Args);

    fn parse_as(
        parser: &mut CellParser<'de>,
        (n, value_args, extra_args): Self::Args,
    ) -> Result<HashmapAugNode<V, E>, CellParserError<'de>> {
        Ok(HashmapAugNode {
            extra: parser.parse_as::<_, AsE>(extra_args.clone())?,
            node: parser.parse_as::<_, HashmapNode<AsV, AsE>>((n, value_args, extra_args))?,
        })
    }
}

impl<V, AsV, E, AsE> CellSerializeAs<HashmapNode<V, E>> for HashmapNode<AsV, AsE>
where
    AsV: CellSerializeAs<V>,
    AsV::Args: Clone,
    AsE: CellSerializeAs<E>,
    AsE::Args: Clone,
{
    type Args = (u32, AsV::Args, AsE::Args);

    fn store_as(
        source: &HashmapNode<V, E>,
        builder: &mut CellBuilder,
        (n, value_args, extra_args): Self::Args,
    ) -> Result<(), CellBuilderError> {
        match source {
            HashmapNode::Leaf(value) => {
                if n != 0 {
                    return Err(CellBuilderError::custom("hashmap key is too small"));
                }
                builder.store_as::<_, &AsV>(value, value_args)?;
            }
            HashmapNode::Fork(children) => {
                if n == 0 {
                    return Err(CellBuilderError::custom("hashmap key is too long"));
                }
                builder.store_as::<_, &[Box<Ref<HashmapTree<AsV, AsE>>>; 2]>(
                    children,
                    (n - 1, value_args, extra_args),
                )?;
            }
        }
        Ok(())
    }
}

impl<'de, V, AsV, E, AsE> CellDeserializeAs<'de, HashmapNode<V, E>> for HashmapNode<AsV, AsE>
where
    AsV: CellDeserializeAs<'de, V>,
    AsV::Args: Clone,
    AsE: CellDeserializeAs<'de, E>,
    AsE::Args: Clone,
{
    type Args = (u32, AsV::Args, AsE::Args);

    fn parse_as(
        parser: &mut CellParser<'de>,
        (n, value_args, extra_args): Self::Args,
    ) -> Result<HashmapNode<V, E>, CellParserError<'de>> {
        if n == 0 {
            return parser.parse_as::<_, AsV>(value_args).map(HashmapNode::Leaf);
        }
        Ok(HashmapNode::Fork(
            parser.parse_as::<_, [Box<Ref<ParseFully<HashmapTree<AsV, AsE>>>>; 2]>((
                n - 1,
                value_args,
                extra_args,
            ))?,
        ))
    }
}

fn validate_pruned_branch(cell: &Cell) -> Result<(), CellParserError<'_>> {
    let bytes = cell.data.as_raw_slice();
    let valid_len = matches!(bytes.len(), 36 | 70 | 104);
    let valid_mask = bytes.get(1).is_some_and(|mask| {
        *mask != 0 && mask & !0b111 == 0 && mask.count_ones() as usize == (bytes.len() - 2) / 34
    });
    if !cell.is_exotic
        || !cell.data.len().is_multiple_of(8)
        || bytes.first() != Some(&1)
        || !cell.references.is_empty()
        || !valid_len
        || !valid_mask
    {
        return Err(CellParserError::custom("invalid pruned branch cell"));
    }
    Ok(())
}

struct HmLabel;

impl BitPackAs<BitSlice<u8, Msb0>> for HmLabel {
    type Args = u32;

    fn pack_as<W>(
        source: &BitSlice<u8, Msb0>,
        writer: &mut W,
        m: Self::Args,
    ) -> Result<(), W::Error>
    where
        W: BitWriter + ?Sized,
    {
        let n = source.len() as u32;
        if n > m {
            return Err(Error::custom("hashmap label exceeds key"));
        }
        if n < m || m == 0 {
            writer
                .pack(false, ())?
                .pack_as::<_, toner::tlb::bits::Unary>(n, ())?
                .write_bitslice(source)?;
            return Ok(());
        }
        let n_bits = m.checked_ilog2().unwrap_or(0) + 1;
        if source.all() || source.not_any() {
            writer
                .pack_as::<_, toner::tlb::bits::NBits<2>>(0b11_u8, ())?
                .pack(source.all(), ())?
                .pack_as::<_, toner::tlb::bits::VarNBits>(n, n_bits)?;
        } else {
            writer
                .pack_as::<_, toner::tlb::bits::NBits<2>>(0b10_u8, ())?
                .pack_as::<_, toner::tlb::bits::VarNBits>(n, n_bits)?
                .write_bitslice(source)?;
        }
        Ok(())
    }
}

impl<'de> BitUnpackAs<'de, BitVec<u8, Msb0>> for HmLabel {
    type Args = u32;

    fn unpack_as<R>(reader: &mut R, m: Self::Args) -> Result<BitVec<u8, Msb0>, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        if !reader.unpack::<bool>(())? {
            let n: u32 = reader.unpack_as::<_, toner::tlb::bits::Unary>(())?;
            if n > m {
                return Err(Error::custom("hashmap label exceeds key"));
            }
            return reader.unpack(n as usize);
        }
        let n_bits = m.checked_ilog2().unwrap_or(0) + 1;
        if !reader.unpack::<bool>(())? {
            let n: u32 = reader.unpack_as::<_, toner::tlb::bits::VarNBits>(n_bits)?;
            if n > m {
                return Err(Error::custom("hashmap label exceeds key"));
            }
            return reader.unpack(n as usize);
        }
        let value: bool = reader.unpack(())?;
        let n: u32 = reader.unpack_as::<_, toner::tlb::bits::VarNBits>(n_bits)?;
        if n > m {
            return Err(Error::custom("hashmap label exceeds key"));
        }
        Ok(BitVec::repeat(value, n as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::{Hashmap, HashmapAugNode, HashmapE, HashmapLookup, HashmapNode, HashmapTree};
    use toner::tlb::bits::bitvec::bits;
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::bitvec::vec::BitVec;
    use toner::tlb::ser::{CellSerializeExt, CellSerializeWrapAsExt};
    use toner::tlb::{Cell, Data, Same};

    #[test]
    fn should_lookup_leaf() {
        let hashmap: HashmapE<u8, u8, 8> = HashmapE::Root(Hashmap::new(HashmapTree::Edge {
            prefix: bits![u8, Msb0; 0, 0, 0, 0, 0, 0, 0, 1].to_bitvec(),
            node: HashmapAugNode {
                extra: (),
                node: HashmapNode::Leaf(42),
            },
        }));

        let actual = hashmap.lookup(&1_u8);

        assert_eq!(actual, HashmapLookup::Found(&42));
    }

    #[test]
    fn should_lookup_with_typed_u32_key() {
        let hashmap: HashmapE<u32, u8, 32> = HashmapE::Root(Hashmap::new(HashmapTree::Edge {
            prefix: bits![u8, Msb0; 0; 31].to_bitvec(),
            node: HashmapAugNode {
                extra: (),
                node: HashmapNode::Fork([
                    Box::new(HashmapTree::Edge {
                        prefix: BitVec::new(),
                        node: HashmapAugNode {
                            extra: (),
                            node: HashmapNode::Leaf(1),
                        },
                    }),
                    Box::new(HashmapTree::Edge {
                        prefix: BitVec::new(),
                        node: HashmapAugNode {
                            extra: (),
                            node: HashmapNode::Leaf(2),
                        },
                    }),
                ]),
            },
        }));

        assert_eq!(hashmap.lookup(&0_u32), HashmapLookup::Found(&1));
        assert_eq!(hashmap.lookup(&1_u32), HashmapLookup::Found(&2));
    }

    #[test]
    fn should_report_pruned_branch() {
        let pruned = pruned_cell();
        let hashmap: HashmapE<u8, u8, 8> =
            HashmapE::Root(Hashmap::new(HashmapTree::Pruned(pruned.clone())));

        let actual = hashmap.lookup(&1_u8);

        assert_eq!(actual, HashmapLookup::Pruned(&pruned));
    }

    #[test]
    fn should_roundtrip_ordinary_hashmap() {
        let expected: HashmapE<u8, u8, 8> = HashmapE::Root(Hashmap::new(HashmapTree::Edge {
            prefix: bits![u8, Msb0; 0, 0, 0, 0, 0, 0, 0, 1].to_bitvec(),
            node: HashmapAugNode {
                extra: (),
                node: HashmapNode::Leaf(42),
            },
        }));
        let cell = expected
            .wrap_as::<HashmapE<u8, Data, 8, Same>>()
            .to_cell(((), ()))
            .unwrap();

        let actual: HashmapE<u8, u8, 8> = cell
            .parse_fully_as::<_, HashmapE<u8, Data, 8, Same>>(((), ()))
            .unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn should_roundtrip_pruned_hashmap() {
        let expected: HashmapE<u8, u8, 8> =
            HashmapE::Root(Hashmap::new(HashmapTree::Pruned(pruned_cell())));
        let cell = expected
            .wrap_as::<HashmapE<u8, Data, 8, Same>>()
            .to_cell(((), ()))
            .unwrap();

        let actual: HashmapE<u8, u8, 8> = cell
            .parse_fully_as::<_, HashmapE<u8, Data, 8, Same>>(((), ()))
            .unwrap();

        assert_eq!(actual, expected);
    }

    fn pruned_cell() -> Cell {
        let mut data = vec![0_u8; 36];
        data[0] = 1;
        data[1] = 1;
        Cell {
            is_exotic: true,
            data: BitVec::from_vec(data),
            references: Vec::new(),
        }
    }
}
