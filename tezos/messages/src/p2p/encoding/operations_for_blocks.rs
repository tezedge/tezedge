// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::{CopyGetters, Getters};
use nom::{
    branch::alt,
    bytes::complete::{tag, take},
    combinator::{flat_map, into, map, success, verify},
    multi::many_till,
    sequence::preceded,
};
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, Hash, HashTrait, HashType, OperationListListHash};
use tezos_encoding::nom::NomResult;
use tezos_encoding::{enc::BinError, nom::NomReader};
use tezos_encoding::{
    enc::BinWriter,
    generator::{self, Generated, Generator},
};
use tezos_encoding::{
    encoding::{Encoding, HasEncoding},
    has_encoding,
};

use crate::p2p::encoding::operation::Operation;

use super::limits::{GET_OPERATIONS_FOR_BLOCKS_MAX_LENGTH, OPERATION_LIST_MAX_SIZE};

#[cfg(feature = "fuzzing")]
use fuzzcheck::{
    mutators::{
        tuples::{Tuple1, Tuple1Mutator, TupleMutatorWrapper, TupleStructure},
        vector::VecMutator,
    },
    DefaultMutator,
};

/// Maximal length for path in a Merkle tree for list of lists of operations.
/// This is calculated from Tezos limit on that Operation_list_list size:
///
/// `let operation_max_pass = ref (Some 8) (* FIXME: arbitrary *)`
///
/// See https://gitlab.com/tezedge/tezos/-/blob/master/src/lib_shell/distributed_db_message.ml#L65
///
/// A  Merkle tree for that list of lenght 8 will be 3 (log_2(8)) levels at most,
/// thus any path should be 3 steps at most.
///
pub const MAX_PASS_MERKLE_DEPTH: usize = 3;
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Debug,
    CopyGetters,
    Getters,
    HasEncoding,
    NomReader,
    BinWriter,
    tezos_encoding::generator::Generated,
)]
pub struct OperationsForBlock {
    #[get = "pub"]
    hash: BlockHash,
    #[get_copy = "pub"]
    validation_pass: i8,
}

impl OperationsForBlock {
    pub fn new(hash: BlockHash, validation_pass: i8) -> Self {
        OperationsForBlock {
            hash,
            validation_pass,
        }
    }

    /// alternative getter because .hash() causes problem with hash() method from Hash trait
    #[inline(always)]
    pub fn block_hash(&self) -> &BlockHash {
        &self.hash
    }
}

// -----------------------------------------------------------------------------------------------
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Debug,
    Getters,
    HasEncoding,
    NomReader,
    BinWriter,
    Generated,
)]
pub struct OperationsForBlocksMessage {
    #[get = "pub"]
    operations_for_block: OperationsForBlock,
    #[get = "pub"]
    operation_hashes_path: Path,
    #[get = "pub"]
    #[encoding(bounded = "OPERATION_LIST_MAX_SIZE", list, dynamic)]
    operations: Vec<Operation>,
}

impl OperationsForBlocksMessage {
    pub fn new(
        operations_for_block: OperationsForBlock,
        operation_hashes_path: Path,
        operations: Vec<Operation>,
    ) -> Self {
        OperationsForBlocksMessage {
            operations_for_block,
            operation_hashes_path,
            operations,
        }
    }
}

impl From<OperationsForBlocksMessage> for Vec<Operation> {
    fn from(msg: OperationsForBlocksMessage) -> Self {
        msg.operations
    }
}

// -----------------------------------------------------------------------------------------------
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Getters)]
pub struct PathRight {
    #[get = "pub"]
    left: Hash,
}

impl PathRight {
    pub fn new(left: Hash) -> Self {
        Self { left }
    }
}

impl Generated for PathRight {
    fn generator<F: generator::GeneratorFactory>(
        prefix: &str,
        f: &mut F,
    ) -> Box<dyn generator::Generator<Item = Self>> {
        Box::new(
            f.hash_bytes(
                &(prefix.to_string() + ".left"),
                OperationListListHash::hash_type(),
            )
            .map(|left| Self { left }),
        )
    }
}

// -----------------------------------------------------------------------------------------------
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Getters)]
pub struct PathLeft {
    #[get = "pub"]
    right: Hash,
}

impl PathLeft {
    pub fn new(right: Hash) -> Self {
        Self { right }
    }
}

impl Generated for PathLeft {
    fn generator<F: generator::GeneratorFactory>(
        prefix: &str,
        f: &mut F,
    ) -> Box<dyn generator::Generator<Item = Self>> {
        Box::new(
            f.hash_bytes(
                &(prefix.to_string() + ".right"),
                OperationListListHash::hash_type(),
            )
            .map(|right| Self { right }),
        )
    }
}

// -----------------------------------------------------------------------------------------------
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, tezos_encoding::generator::Generated)]
pub enum PathItem {
    Right(PathRight),
    Left(PathLeft),
}

impl PathItem {
    pub fn right(left: Hash) -> PathItem {
        PathItem::Right(PathRight::new(left))
    }
    pub fn left(right: Hash) -> PathItem {
        PathItem::Left(PathLeft::new(right))
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct Path(pub Vec<PathItem>);

#[cfg(feature = "fuzzing")]
impl TupleStructure<Tuple1<Vec<PathItem>>> for Path {
    #[no_coverage]
    fn get_ref<'a>(&'a self) -> (&'a Vec<PathItem>,) {
        (&self.0,)
    }
    #[no_coverage]
    fn get_mut<'a>(&'a mut self) -> (&'a mut Vec<PathItem>,) {
        (&mut self.0,)
    }
    #[no_coverage]
    fn new(t: (Vec<PathItem>,)) -> Self {
        Self { 0: t.0 }
    }
}

#[cfg(feature = "fuzzing")]
type VecPathItemMutator = VecMutator<PathItem, <PathItem as fuzzcheck::DefaultMutator>::Mutator>;

#[cfg(feature = "fuzzing")]
pub struct PathMutator {
    mutator: TupleMutatorWrapper<Tuple1Mutator<VecPathItemMutator>, Tuple1<Vec<PathItem>>>,
}

#[cfg(feature = "fuzzing")]
impl PathMutator {
    #[no_coverage]
    pub fn new() -> Self {
        let bounded_mut =
            VecPathItemMutator::new(PathItem::default_mutator(), 0..=MAX_PASS_MERKLE_DEPTH);

        Self {
            mutator: TupleMutatorWrapper::new(Tuple1Mutator::new(bounded_mut)),
        }
    }
}

#[cfg(feature = "fuzzing")]
impl fuzzcheck::MutatorWrapper for PathMutator {
    type Wrapped = TupleMutatorWrapper<Tuple1Mutator<VecPathItemMutator>, Tuple1<Vec<PathItem>>>;
    #[no_coverage]
    fn wrapped_mutator(&self) -> &Self::Wrapped {
        &self.mutator
    }
}

#[cfg(feature = "fuzzing")]
impl fuzzcheck::DefaultMutator for Path {
    type Mutator = PathMutator;
    #[no_coverage]
    fn default_mutator() -> Self::Mutator {
        PathMutator::new()
    }
}

impl Path {
    pub fn op() -> Self {
        Path(Vec::new())
    }
}

/// Manual serializization ensures that path depth does not exceed max value
impl Serialize for Path {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.0.len() > MAX_PASS_MERKLE_DEPTH {
            use serde::ser::Error;
            return Err(Error::custom(format!(
                "Path size exceedes its boundary {} for encoding",
                MAX_PASS_MERKLE_DEPTH
            )));
        }
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        self.0.iter().try_for_each(|i| seq.serialize_element(i))?;
        seq.end()
    }
}

has_encoding!(Path, PATH_ENCODING, { Encoding::Custom });

#[derive(Clone)]
enum DecodePathNode {
    Left,
    Right(Hash),
}

impl From<Vec<u8>> for DecodePathNode {
    fn from(bytes: Vec<u8>) -> Self {
        DecodePathNode::Right(bytes)
    }
}

fn hash(input: &[u8]) -> NomResult<Vec<u8>> {
    map(
        take(HashType::OperationListListHash.size()),
        |slice: &[u8]| slice.to_vec(),
    )(input)
}

fn path_left(input: &[u8]) -> NomResult<DecodePathNode> {
    preceded(tag(0xf0u8.to_be_bytes()), success(DecodePathNode::Left))(input)
}

fn path_right(input: &[u8]) -> NomResult<DecodePathNode> {
    preceded(tag(0x0fu8.to_be_bytes()), into(hash))(input)
}

fn path_op(input: &[u8]) -> NomResult<()> {
    preceded(tag(0x00u8.to_be_bytes()), success(()))(input)
}

fn path_complete(nodes: Vec<DecodePathNode>) -> impl FnMut(&[u8]) -> NomResult<Path> {
    move |mut input| {
        let mut res = Vec::new();
        for node in nodes.clone().into_iter().rev() {
            match node {
                DecodePathNode::Left => {
                    let (i, h) = hash(input)?;
                    res.push(PathItem::left(h));
                    input = i;
                }
                DecodePathNode::Right(h) => res.push(PathItem::right(h)),
            }
        }
        res.reverse();
        Ok((input, Path(res)))
    }
}

impl NomReader for Path {
    fn nom_read(bytes: &[u8]) -> tezos_encoding::nom::NomResult<Self> {
        flat_map(
            verify(
                map(many_till(alt((path_left, path_right)), path_op), |(v, _)| v),
                |nodes: &Vec<DecodePathNode>| MAX_PASS_MERKLE_DEPTH >= nodes.len(),
            ),
            path_complete,
        )(bytes)
    }
}

fn bin_write_path_items(
    path_items: &[PathItem],
    out: &mut Vec<u8>,
) -> tezos_encoding::enc::BinResult {
    match path_items.split_first() {
        None => {
            tezos_encoding::enc::u8(&0x00, out)?;
        }
        Some((PathItem::Left(left), rest)) => {
            tezos_encoding::enc::u8(&0xf0, out)?;
            bin_write_path_items(rest, out)?;
            tezos_encoding::enc::put_bytes(&left.right, out);
        }
        Some((PathItem::Right(right), rest)) => {
            tezos_encoding::enc::u8(&0x0f, out)?;
            tezos_encoding::enc::put_bytes(&right.left, out);
            bin_write_path_items(rest, out)?;
        }
    }
    Ok(())
}

impl BinWriter for Path {
    fn bin_write(&self, out: &mut Vec<u8>) -> tezos_encoding::enc::BinResult {
        if self.0.len() > MAX_PASS_MERKLE_DEPTH {
            return Err(BinError::custom(format!(
                "Merkle path size {} is greater than max {}",
                self.0.len(),
                MAX_PASS_MERKLE_DEPTH
            )));
        }
        bin_write_path_items(self.0.as_slice(), out)
    }
}

impl tezos_encoding::generator::Generated for Path {
    fn generator<F: tezos_encoding::generator::GeneratorFactory>(
        prefix: &str,
        f: &mut F,
    ) -> Box<dyn tezos_encoding::generator::Generator<Item = Self>> {
        Box::new(
            generator::vec_of_items(
                PathItem::generator(&(prefix.to_string() + "[]"), f),
                f.size(prefix, Path::encoding().clone(), Encoding::Unit), //generator::values(&[0, 1, MAX_PASS_MERKLE_DEPTH, MAX_PASS_MERKLE_DEPTH+1, MAX_PASS_MERKLE_DEPTH * 2])
            )
            .map(|vec| Self(vec)),
        )
    }
}

// -----------------------------------------------------------------------------------------------
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Serialize,
    Deserialize,
    Debug,
    Getters,
    Clone,
    HasEncoding,
    NomReader,
    BinWriter,
    tezos_encoding::generator::Generated,
)]
pub struct GetOperationsForBlocksMessage {
    #[get = "pub"]
    #[encoding(dynamic, list = "GET_OPERATIONS_FOR_BLOCKS_MAX_LENGTH")]
    get_operations_for_blocks: Vec<OperationsForBlock>,
}

impl GetOperationsForBlocksMessage {
    pub fn new(get_operations_for_blocks: Vec<OperationsForBlock>) -> Self {
        GetOperationsForBlocksMessage {
            get_operations_for_blocks,
        }
    }
}
