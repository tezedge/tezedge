// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use tezos_encoding::encoding::{Encoding, Field};

use tezos_encoding::encoding::HasEncoding;
use tezos_messages::p2p::binary_message::BinaryMessage;

mod message_limit;
use message_limit::*;

/// Encoding node kind
#[derive(Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// Bounded dynamic encoding,
    Dynamic(Option<usize>),
    /// Bounded encoding
    Bounded(Option<usize>),
    /// Bounded list
    List(Option<usize>),
    /// Bounded string
    String(Option<usize>),
    /// Field
    Field(String),
    /// Field
    Tag(u16),
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NodeKind::Dynamic(m) => write!(f, "dynamic({:?})", m),
            NodeKind::Bounded(m) => write!(f, "bounded({:?})", m),
            NodeKind::List(m) => write!(f, "list({:?})", m),
            NodeKind::String(m) => write!(f, "string({:?})", m),
            NodeKind::Field(name) => write!(f, "{}", name),
            NodeKind::Tag(n) => write!(f, "tag({})", n),
        }
    }
}

/// Path identifying encoding node
#[derive(Debug, PartialEq, Eq)]
pub enum NodePath {
    /// Root encoding element
    Root,
    /// Child encoding
    Child(Rc<NodePath>, NodeKind),
}

impl NodePath {
    fn root() -> Rc<Self> {
        Rc::new(NodePath::Root)
    }

    fn child(parent: &Rc<NodePath>, kind: NodeKind) -> Rc<Self> {
        Rc::new(NodePath::Child(parent.clone(), kind))
    }

    fn is_child_of(self: &NodePath, other: &NodePath) -> bool {
        match self {
            NodePath::Root => false,
            NodePath::Child(child, _) => child.as_ref() == other || child.is_child_of(other),
        }
    }
}

use std::{cmp, fmt};

impl fmt::Display for NodePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NodePath::Root => write!(f, "root"),
            NodePath::Child(p, k) => write!(f, "{}.{}", p, k),
        }
    }
}

/// Kind of data encoding generation
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum GenKind {
    /// Minimal possible size
    Min,
    /// Maximal possible size
    Max,
    /// Oversized encoded data
    Over,
}

impl GenKind {
    fn iterator() -> std::slice::Iter<'static, Self> {
        static ITEMS: [GenKind; 3] = [GenKind::Min, GenKind::Max, GenKind::Over];
        ITEMS.iter()
    }

    fn is_valid(&self) -> bool {
        !matches!(self, GenKind::Over)
    }
}

impl fmt::Display for GenKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            GenKind::Min => "min",
            GenKind::Max => "max",
            GenKind::Over => "over",
        };
        write!(f, "{}", name)
    }
}

/// Browses encoding structure and extracts elements with limits.
struct EncodingExplorer {
    paths: Vec<Rc<NodePath>>,
}

impl EncodingExplorer {
    pub fn new() -> Self {
        Self { paths: Vec::new() }
    }

    fn register_path(&mut self, path: &Rc<NodePath>) {
        self.paths.push(path.clone())
    }

    /// Calculates paths to encoding elements speifying limits
    pub fn calculate_paths(encoding: &Encoding) -> Vec<Rc<NodePath>> {
        let mut slf = EncodingExplorer::new();
        slf.get_paths(NodePath::root(), encoding);
        slf.paths
    }

    fn get_paths(&mut self, path: Rc<NodePath>, encoding: &Encoding) {
        match encoding {
            Encoding::Bounded(max, encoding) => {
                let path = NodePath::child(&path, NodeKind::Bounded(Some(*max)));
                self.register_path(&path);
                self.get_paths(path, encoding);
            }
            Encoding::BoundedDynamic(max, encoding) => {
                let path = NodePath::child(&path, NodeKind::Dynamic(Some(*max)));
                self.register_path(&path);
                self.get_paths(path, encoding);
            }
            Encoding::BoundedList(max, encoding) => {
                let path = NodePath::child(&path, NodeKind::List(Some(*max)));
                self.register_path(&path);
                self.get_paths(path, encoding);
            }
            Encoding::BoundedString(max) => {
                let path = NodePath::child(&path, NodeKind::String(Some(*max)));
                self.register_path(&path);
            }
            Encoding::Dynamic(encoding) => {
                let path = NodePath::child(&path, NodeKind::Dynamic(None));
                self.get_paths(path, encoding);
            }
            Encoding::List(encoding) => {
                let path = NodePath::child(&path, NodeKind::List(None));
                self.get_paths(path, encoding);
            }
            Encoding::Obj(_, fields) => {
                for field in fields {
                    let path = NodePath::child(&path, NodeKind::Field(field.get_name().clone()));
                    self.get_paths(path, field.get_encoding());
                }
            }
            Encoding::Tags(_, tags) => {
                for tag in tags.tags() {
                    let path = NodePath::child(&path, NodeKind::Tag(tag.get_id()));
                    self.get_paths(path, tag.get_encoding());
                }
            }
            _ => {}
        }
    }
}

/// Encoded data generation mode
enum GenMode {
    Min,
    /// Generate minimal non-empty content
    MinNonEmpty,
    /// Generate content with greatest possible size
    Fill,
}

/// Generates encoded data for given encoding, focusing on the element
/// specified by `Path`, generating minimal, maximal or oversized data for that
/// encoding part
struct EncodedDataGenerator {
    path: Rc<NodePath>,
    kind: GenKind,
    mode: GenMode,
    filling: bool,

    next_byte: u8,
    avail: Option<usize>,
}

impl EncodedDataGenerator {
    pub fn generate_data(path: &Rc<NodePath>, kind: GenKind, encoding: &Encoding) -> Vec<u8> {
        Self::new(path, kind).generate(&NodePath::root(), encoding)
    }

    fn new(path: &Rc<NodePath>, kind: GenKind) -> EncodedDataGenerator {
        Self {
            path: path.clone(),
            kind,
            mode: GenMode::MinNonEmpty,
            filling: false,
            avail: None,
            next_byte: 0,
        }
    }

    fn decrease_avail(&mut self, len: usize) {
        self.avail = match self.avail {
            Some(avail) if avail >= len => Some(avail - len),
            Some(_) => Some(0),
            _ => None,
        };
    }

    fn increase_avail(&mut self, len: usize) {
        self.avail = self.avail.map(|avail| avail + len);
    }

    fn size(&mut self, size: usize) -> Vec<u8> {
        (size as u32).to_be_bytes().to_vec()
    }

    fn byte(&mut self, val: u8) -> Vec<u8> {
        self.decrease_avail(1);
        vec![val]
    }

    fn bytes(&mut self, len: usize) -> Vec<u8> {
        self.decrease_avail(len);
        let res = std::iter::repeat(self.next_byte).take(len).collect();
        if self.next_byte == 0xff {
            self.next_byte = 0;
        } else {
            self.next_byte += 1;
        }
        res
    }

    fn focus_size(&self, max: usize) -> usize {
        match self.kind {
            GenKind::Min => 0,
            GenKind::Max => max,
            GenKind::Over => max + 1,
        }
    }

    fn string_fill_size(&self, max: Option<usize>) -> usize {
        let avail = self.available();
        if self.filling || self.kind != GenKind::Over {
            max.map(|m| std::cmp::min(avail, m)).unwrap_or(avail)
        } else {
            avail + 1
        }
    }

    fn string(&mut self, path: &Rc<NodePath>, max: Option<usize>) -> Vec<u8> {
        self.decrease_avail(4);
        let len = if *path == self.path {
            self.focus_size(max.expect("Focus string without limit"))
        } else {
            match self.mode {
                GenMode::Min => 0,
                GenMode::MinNonEmpty => 1,
                GenMode::Fill => self.string_fill_size(max),
            }
        };
        let mut res = self.size(len);
        res.extend(self.bytes(len));
        res
    }

    fn available(&self) -> usize {
        self.avail.expect("Not in fill mode")
    }

    fn bounded_min(&mut self, path: &Rc<NodePath>, _max: usize, encoding: &Encoding) -> Vec<u8> {
        self.mode = GenMode::Min;
        let res = self.generate(path, encoding);
        self.mode = GenMode::MinNonEmpty;
        res
    }

    fn bounded_non_min(
        &mut self,
        path: &Rc<NodePath>,
        _max: usize,
        encoding: &Encoding,
    ) -> Vec<u8> {
        self.mode = GenMode::Fill;
        let res = self.generate(path, encoding);
        self.mode = GenMode::MinNonEmpty;
        res
    }

    fn bounded_focused(&mut self, path: &Rc<NodePath>, max: usize, encoding: &Encoding) -> Vec<u8> {
        match self.kind {
            GenKind::Min => self.bounded_min(path, max, encoding),
            _ => self.bounded_non_min(path, max, encoding),
        }
    }

    fn bounded_other(&mut self, path: &Rc<NodePath>, _max: usize, encoding: &Encoding) -> Vec<u8> {
        self.generate(path, encoding)
    }

    fn bounded(&mut self, path: &Rc<NodePath>, max: usize, encoding: &Encoding) -> Vec<u8> {
        let self_avail = self.avail;
        self.avail = Some(max);
        let res = if *path == self.path {
            self.bounded_focused(path, max, encoding)
        } else {
            self.bounded_other(path, max, encoding)
        };
        self.avail = self_avail;
        res
    }

    fn dynamic_focused(
        &mut self,
        path: &Rc<NodePath>,
        _max: usize,
        encoding: &Encoding,
    ) -> Vec<u8> {
        self.mode = match self.kind {
            GenKind::Min => GenMode::Min,
            GenKind::Max | GenKind::Over => GenMode::Fill,
        };
        let res = self.generate(path, encoding);
        self.mode = GenMode::MinNonEmpty;
        res
    }

    fn dynamic_other(&mut self, path: &Rc<NodePath>, encoding: &Encoding) -> Vec<u8> {
        self.generate(path, encoding)
    }

    fn dynamic(&mut self, path: &Rc<NodePath>, max: Option<usize>, encoding: &Encoding) -> Vec<u8> {
        self.decrease_avail(4);
        let self_avail = self.avail;
        if let Some(max) = max {
            self.avail = Some(cmp::min(max, self.avail.unwrap_or(max)));
        }
        let res = if *path == self.path {
            self.dynamic_focused(path, max.expect("Focused dynamic without limit"), encoding)
        } else {
            self.dynamic_other(path, encoding)
        };
        let len = res.len();
        let mut size = self.size(len);
        size.extend(res);
        self.avail = self_avail;
        self.decrease_avail(len);
        size
    }

    fn list_focused(&mut self, path: &Rc<NodePath>, max: usize, encoding: &Encoding) -> Vec<u8> {
        let len = self.focus_size(max);
        let res = self.generate(path, encoding);
        std::iter::repeat(res).take(len).flatten().collect()
    }

    fn list_fill(
        &mut self,
        path: &Rc<NodePath>,
        _max: Option<usize>,
        encoding: &Encoding,
    ) -> Vec<u8> {
        self.filling = true;
        let mut res = Vec::new();
        let mut avail = self.available();
        let self_kind = self.kind;
        if self.kind == GenKind::Over {
            self.kind = GenKind::Max;
        }
        if let Encoding::Uint8 = encoding {
            let elt = self.generate(path, encoding);
            let elts = vec![elt[0]; avail];
            self.avail = Some(0);
            res.extend(elts);
        } else {
            while avail > 0 {
                let elt = self.generate(path, encoding);
                assert!(!elt.is_empty());
                if avail < elt.len() {
                    // restore avail
                    self.avail = Some(avail);
                    break;
                }
                avail -= elt.len();
                self.avail = Some(avail);
                res.extend(elt);
            }
        }
        self.kind = self_kind;
        if self.kind == GenKind::Over {
            self.mode = GenMode::MinNonEmpty;
            let elt = self.generate(path, encoding);
            assert!(!elt.is_empty());
            res.extend(elt);
        }
        res
    }

    fn list_min_non_empty(
        &mut self,
        path: &Rc<NodePath>,
        _max: Option<usize>,
        encoding: &Encoding,
    ) -> Vec<u8> {
        self.generate(path, encoding)
    }

    fn list(&mut self, path: &Rc<NodePath>, max: Option<usize>, encoding: &Encoding) -> Vec<u8> {
        if *path == self.path {
            self.list_focused(path, max.expect("Focused dynamic without limit"), encoding)
        } else {
            match self.mode {
                GenMode::Min => self.bytes(0),
                GenMode::Fill => self.list_fill(path, max, encoding),
                GenMode::MinNonEmpty => self.list_min_non_empty(path, max, encoding),
            }
        }
    }

    fn obj_fill(&mut self, path: &Rc<NodePath>, fields: &[Field]) -> Vec<u8> {
        let mut res = Vec::new();
        let self_kind = self.kind;
        if self_kind == GenKind::Over {
            self.kind = GenKind::Max;
        }
        let limits = fields
            .iter()
            .map(|f| get_limits(f.get_encoding()))
            .collect::<Vec<_>>();
        let min_sizes = limits.iter().map(Limits::lower).sum();
        self.decrease_avail(min_sizes);
        let last_var_len_field = limits.iter().enumerate().rev().find_map(|(i, l)| {
            if l.upper().is_variable_length() {
                Some(i)
            } else {
                None
            }
        });
        for (i, (field, min_size)) in fields.iter().zip(limits.iter()).enumerate() {
            self.increase_avail(min_size.lower());
            let path = NodePath::child(path, NodeKind::Field(field.get_name().clone()));
            if let Some(ind) = last_var_len_field {
                if ind == i {
                    self.kind = self_kind;
                }
            }
            res.extend(self.generate(&path, field.get_encoding()));
        }
        res
    }

    fn obj_other(&mut self, path: &Rc<NodePath>, fields: &[Field]) -> Vec<u8> {
        let mut res = Vec::new();
        for field in fields {
            let path = NodePath::child(path, NodeKind::Field(field.get_name().clone()));
            self.mode = if self.path.is_child_of(&path) {
                GenMode::MinNonEmpty
            } else {
                GenMode::Min
            };
            res.extend(self.generate(&path, field.get_encoding()));
        }
        res
    }

    fn obj(&mut self, path: &Rc<NodePath>, fields: &[Field]) -> Vec<u8> {
        match self.mode {
            GenMode::Fill => self.obj_fill(path, fields),
            _ => self.obj_other(path, fields),
        }
    }

    pub fn generate(&mut self, path: &Rc<NodePath>, encoding: &Encoding) -> Vec<u8> {
        let res = match encoding {
            Encoding::Bounded(max, encoding) => {
                let path = NodePath::child(path, NodeKind::Bounded(Some(*max)));
                self.bounded(&path, *max, encoding)
            }
            Encoding::BoundedDynamic(max, encoding) => {
                let path = NodePath::child(path, NodeKind::Dynamic(Some(*max)));
                self.dynamic(&path, Some(*max), encoding)
            }
            Encoding::BoundedList(max, encoding) => {
                let path = NodePath::child(path, NodeKind::List(Some(*max)));
                self.list(&path, Some(*max), encoding)
            }
            Encoding::BoundedString(max) => {
                let path = NodePath::child(path, NodeKind::String(Some(*max)));
                self.string(&path, Some(*max))
            }
            Encoding::Dynamic(encoding) => {
                let path = NodePath::child(path, NodeKind::Dynamic(None));
                self.dynamic(&path, None, encoding)
            }
            Encoding::List(encoding) => {
                let path = NodePath::child(path, NodeKind::List(None));
                self.list(&path, None, encoding)
            }
            Encoding::String => {
                let path = NodePath::child(path, NodeKind::String(None));
                self.string(&path, None)
            }
            Encoding::Obj(_, fields) => self.obj(path, fields),
            Encoding::OptionalField(_) => self.byte(0x00),
            Encoding::Unit => self.bytes(0),
            Encoding::Int8 | Encoding::Uint8 => self.bytes(1),
            Encoding::Int16 | Encoding::Uint16 => self.bytes(2),
            Encoding::Int31 | Encoding::Int32 | Encoding::Uint32 => self.bytes(4),
            Encoding::Int64 | Encoding::Timestamp => self.bytes(8),
            Encoding::Hash(hash) => self.bytes(hash.size()),
            /*
            Encoding::Tags(_, tags) => {
                for tag in tags.tags() {
                    let path = NodePath::child(&path, NodeKind::Tag(tag.get_id()));

                    self.get_paths(path, tag.get_encoding());
                }
            }
            */
            _ => unimplemented!("{:?}", encoding),
        };
        res
    }
}

/// Generates various encoded data samples and applies the `f` to each
/// passing data and data validity flag.
pub fn with_generated_encoded_data<F>(encoding: &Encoding, f: F)
where
    F: Fn(Vec<u8>, bool, &Rc<NodePath>, GenKind),
{
    let paths = EncodingExplorer::calculate_paths(encoding);
    for path in paths {
        for kind in GenKind::iterator() {
            let data = EncodedDataGenerator::generate_data(&path, *kind, encoding);
            f(data, kind.is_valid(), &path, *kind);
        }
    }
}

/// Applies p2p message binary decoder to each generated data
/// and checks that decoding result corresponds with data validity.
pub fn test_decoding_generated_data<T>()
where
    T: HasEncoding + BinaryMessage,
{
    with_generated_encoded_data(&T::encoding(), |data, correct, _, _| {
        let res = T::from_bytes(&data);
        match (correct, res) {
            (true, Err(e)) => panic!("Expected correct decoding, got {:?}", e),
            (false, Ok(_)) => panic!("Expected failing decoding, got Ok: {}", hex::encode(data)),
            _ => (),
        }
    });
}

mod tests {
    use tezos_messages::p2p::encoding::prelude::*;

    use super::test_decoding_generated_data;

    #[test]
    fn decode_generated_data_advertise() {
        test_decoding_generated_data::<AdvertiseMessage>();
    }

    #[test]
    fn decode_generated_data_swap() {
        test_decoding_generated_data::<SwapMessage>();
    }

    #[test]
    fn decode_generated_data_current_branch() {
        test_decoding_generated_data::<CurrentBranchMessage>();
    }

    #[test]
    fn decode_generated_data_deactivate() {
        test_decoding_generated_data::<DeactivateMessage>();
    }

    #[test]
    fn decode_generated_data_get_current_head() {
        test_decoding_generated_data::<GetCurrentHeadMessage>();
    }

    #[test]
    fn decode_generated_data_current_head() {
        test_decoding_generated_data::<CurrentHeadMessage>();
    }

    #[test]
    fn decode_generated_data_get_block_headers() {
        test_decoding_generated_data::<GetBlockHeadersMessage>();
    }

    #[test]
    fn decode_generated_data_block_header() {
        test_decoding_generated_data::<BlockHeaderMessage>();
    }

    #[test]
    fn decode_generated_data_get_operations() {
        test_decoding_generated_data::<GetOperationsMessage>();
    }

    #[test]
    fn decode_generated_data_operation() {
        test_decoding_generated_data::<OperationMessage>();
    }

    #[test]
    fn decode_generated_data_get_protocols() {
        test_decoding_generated_data::<GetProtocolsMessage>();
    }

    #[test]
    fn decode_generated_data_protocol() {
        test_decoding_generated_data::<ProtocolMessage>();
    }

    #[test]
    fn decode_generated_data_get_operations_for_blocks() {
        test_decoding_generated_data::<GetOperationsForBlocksMessage>();
    }
}
