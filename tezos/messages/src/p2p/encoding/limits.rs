// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::HashType;

/// P2P message encoding maximal size in OCaml
///
/// OCaml refs:
///
/// [lib_p2p/p2p_message.ml:37](https://gitlab.com/tezedge/tezos/-/blob/9aac95765dc8290ce2f722b7bd71042ff9609f46/src/lib_p2p/p2p_message.ml#L37)
/// ```ocaml
/// let encoding msg_encoding =
///  let open Data_encoding in
///  check_size (100 * 1024 * 1024)
///  (*Very high, arbitrary upper bound for message encodings  *)
///  ...
/// ```
pub const OCAML_MESSAGE_MAX_SIZE: usize = 100 * 1024 * 1024;

/// P2P message encoding maximal size.
///
/// This is calculated from encoding schema as a maximal possible size of all message variants.
/// The biggest one happens to be a `PeerResponseMessage::CurrentHead`.
pub const MESSAGE_MAX_SIZE: usize = 4 +         // dynamic block size
    2 +                                         // tag size
    (                                           // CurrentHeadMessage
        HashType::ChainId.size() +              //   chain_id: ChainId
        4 + BLOCK_HEADER_MAX_SIZE +                 //   current_block_header: BlockHeader
        MEMPOOL_MAX_SIZE                        //   current_mempool: Mempool,
    );

/// P2P Point ID encoding maximal size
///
/// OCaml refs:
/// tezos/src/lib_p2p/p2p_message.ml:64 (comment)
///
/// lib_base/p2p_point.ml:127
/// [lib_base/p2p_point.ml#L127](https://gitlab.com/tezedge/tezos/-/blob/736f733f661877b5868f96d5c878f3f4a0486cb6/src/lib_base/p2p_point.ml#L127)
/// ```ocaml
///   let encoding =
///   let open Data_encoding in
///    check_size
///      ( 4 (* Uint30 that gives the size of the encoded string *)
///      + (8 (*number of IPv6 chunks *) * (*size of IPv6 chunks*) 4)
///      + (*IPv6 chunk separators*) 7 + (*optional enclosing bracket*) 2
///      + (*port separator*) 1 + (*size of port number*) 5 )
///    @@ def "p2p_point.id" ~description:"Identifier for a peer point"
///    @@ conv to_string of_string_exn string
/// ```
pub const P2P_POINT_MAX_SIZE: usize = 4 + 4 * 8 + 7 + 2 + 1 + 5; // 51

/// P2P Point ID maximal length
///
/// 4 bytes less than its encoding
pub const P2P_POINT_MAX_LENGTH: usize = P2P_POINT_MAX_SIZE - 4; // 47

/// NACK's `potential_peers_to_connect` maximal length
///
/// OCaml refs:
/// [lib_p2p/p2p_socket.ml:252](https://gitlab.com/tezedge/tezos/-/blob/8166ed7ad215544be69241df99af95deebeaeee3/src/lib_p2p/p2p_socket.ml#L252)
/// ```ocaml
///     let nack_encoding =
///      obj2
///        (req "nack_motive" P2p_rejection.encoding)
///        (req
///           "nack_list"
///           (Data_encoding.list ~max_length:100 P2p_point.Id.encoding))
/// ```
pub const NACK_PEERS_MAX_LENGTH: usize = 100;

/// Number of validation passes/operation groups
///
/// OCaml ref: tezos/src/lib_shell/distributed_db_message.ml:65,
/// `operation_max_pass`
///
/// ```ocaml
///   let operation_max_pass = ref (Some 8) (* FIXME: arbitrary *)
/// ```
pub const OPERATION_MAX_PASS: usize = 8;

/// Advertise list, max 100 items
///
/// OCaml ref: tezos/src/lib_p2p/p2p_message.ml:64
/// ```ocaml
///                 (req "id" (Variable.list ~max_length:100 P2p_point.Id.encoding))
/// ```
pub const ADVERTISE_ID_LIST_MAX_LENGTH: usize = 100;

///
/// OCaml ref: tezos/src/lib_p2p/p2p_connect_handler.ml
/// ```ocaml
///         P2p_pool.list_known_points ~ignore_private:true ~size:50 t.pool
///         (* Never send more than 100 points, you would be greylisted *)
/// ```
pub const ADVERTISE_ID_LIST_MAX_LENGTH_FOR_SEND: usize = 50;

/// CurrentBranch history max length
///
/// OCaml refs:
///
/// `block_locator_max_length` in
/// [lib_shell/distributed_db_message.ml:32](https://gitlab.com/tezedge/tezos/-/blob/922212530bf3442ab5535072ee95c406a0203816/src/lib_shell/distributed_db_message.ml#L32)
///
/// ```ocaml
///   let block_locator_max_length = ref 1000
/// ````
pub const CURRENT_BRANCH_HISTORY_MAX_LENGTH: usize = 1000;

/// CurrentBranch history max length used when constructing
/// current branch.
///
/// OCaml refs:
/// `max_locator_size` in
/// [lib_shell/state.ml:1801](https://gitlab.com/tezos/tezos/-/blob/latest-release/src/lib_shell/state.ml#L1801)
///
/// ```ocaml
/// (* FIXME: this should not be hard-coded *)
/// let max_locator_size = 200
/// ```
pub const HISTORY_MAX_SIZE: u8 = 200;

/// Block header max size
///
/// OCaml refs:
/// [lib_shell/distributed_db_message.ml:30](https://gitlab.com/tezedge/tezos/-/blob/922212530bf3442ab5535072ee95c406a0203816/src/lib_shell/distributed_db_message.ml#L30)
/// `block_header_max_size`
///
/// ```ocaml
///   let block_header_max_size = ref (8 * 1024 * 1024)
/// ```
pub const BLOCK_HEADER_MAX_SIZE: usize = 8 * 1024 * 1024;

/// Operation max size
///
/// OCaml ref: tezos/src/lib_shell/distributed_db_message.ml:59,
/// `operation_max_size`
///
/// ```ocaml
///   let operation_max_size = ref (Some (128 * 1024)) (* FIXME: arbitrary *)
/// ```
pub const OPERATION_MAX_SIZE: usize = 128 * 1024;

/// Protocol max size
///
/// OCaml ref: tezos/src/lib_shell/distributed_db_message.ml:122,
/// `protocol_max_size`
///
/// ```ocaml
///   let protocol_max_size = ref (Some (2 * 1024 * 1024)) (* FIXME: arbitrary *)
/// ```
pub const PROTOCOL_MAX_SIZE: usize = 2 * 1024 * 1024;

/// Mempool maximum operations number
///
/// OCaml refs:
///
/// `mempool_max_operations` in
/// [lib_shell/distributed_db_message.ml:142](https://gitlab.com/tezedge/tezos/-/blob/3c6f7c0126472693cd2cd5208e5301aae9283dde/src/lib_shell/distributed_db_message.ml#L142)
/// ```ocaml
///   let mempool_max_operations = ref (Some 4000)
/// ```
pub const MEMPOOL_MAX_OPERATIONS: usize = 4000;

/// Mempool encoding maximum size
///
/// OCaml refs:
///
/// [lib_base/mempool.ml:44](https://gitlab.com/tezedge/tezos/-/blob/3c6f7c0126472693cd2cd5208e5301aae9283dde/src/lib_base/mempool.ml#L44)
/// ```ocaml
/// let bounded_encoding ?max_operations () =
///  match max_operations with
///  | None ->
///      encoding
///  | Some max_operations ->
///      Data_encoding.check_size
///        (8 + (max_operations * Operation_hash.size))
///        encoding
/// ```
pub const MEMPOOL_MAX_SIZE: usize = 12 + MEMPOOL_MAX_OPERATIONS * HashType::OperationHash.size();

/// Get block headers max length
///
/// OCaml ref: tezos/src/lib_shell/distributed_db_message.ml:217,
///
/// ```ocaml
///       (obj1 (req "get_block_headers" (list ~max_length:10 Block_hash.encoding)))
/// ```
pub const GET_BLOCK_HEADERS_MAX_LENGTH: usize = 10;

/// Get operations max length
///
/// OCaml ref: tezos/src/lib_shell/distributed_db_message.ml:230,
///
/// ```ocaml
///         (req "get_operations" (list ~max_length:10 Operation_hash.encoding)))
/// ```
pub const GET_OPERATIONS_MAX_LENGTH: usize = 10;

/// Get protocols max length
///
/// OCaml ref: tezos/src/lib_shell/distributed_db_message.ml:242,
///
/// ```ocaml
///       (obj1 (req "get_protocols" (list ~max_length:10 Protocol_hash.encoding)))
/// ```
pub const GET_PROTOCOLS_MAX_LENGTH: usize = 10;

/// Max size of Protocol::components encoding, six bytes less than Protocol
pub const PROTOCOL_COMPONENT_MAX_SIZE: usize = PROTOCOL_MAX_SIZE - 6;

/// Get operations for block max length
///
/// OCaml ref: tezos/src/lib_shell/distributed_db_message.ml:258,
///
/// ```ocaml
///      (obj1
///         (req
///            "get_operations_for_blocks"
///            (list
///               ~max_length:10
///               (obj2
///                  (req "hash" Block_hash.encoding)
///                  (req "validation_pass" int8)))))
/// ```
pub const GET_OPERATIONS_FOR_BLOCKS_MAX_LENGTH: usize = 10;

/// Operations for block's operations max encoded size
///
/// OCaml ref: tezos/src/lib_shell/distributed_db_message.ml:61,
/// `operation_list_max_size`
///
/// ```ocaml
///  let operation_list_max_size = ref (Some (1024 * 1024)) (* FIXME: arbitrary *)
/// ```
pub const OPERATION_LIST_MAX_SIZE: usize = 1024 * 1024;

/// Maximal lenght for Tezos chain name.
///
/// Currently the longest one is the `TEZOS_ALPHANET_CARTHAGE_2019-11-28T13:02:13Z`.
/// It should be pretty safe to have the length greater than that.
///
pub const CHAIN_NAME_MAX_LENGTH: usize = 128;

/// Maximal length of the component name.
///
/// TODO: actual limit.
pub const COMPONENT_NAME_MAX_LENGTH: usize = 1024;

/// Maximal length of the component interface.
///
/// TODO: actual limit.
pub const COMPONENT_INTERFACE_MAX_LENGTH: usize = 1024 * 100;

/// Maximal length of the component implementation.
///
/// TODO: actual limit.
pub const COMPONENT_IMPLEMENTATION_MAX_LENGTH: usize = 1024 * 100;

/// Maximal number of element in `fitness`.
pub const BLOCK_HEADER_FITNESS_ELEMENTS: usize = 2;

/// Maximal length of a `fitness` element.
pub const BLOCK_HEADER_FITNESS_ELEMENT_LENGTH: usize = 8;

/// Maximal size of fitness.
///
/// TODO: actual limit. Currently this is a one-byte list and 8-bytes list.
/// We can safely limit it to a two-element list of 8-byte lists.
pub const BLOCK_HEADER_FITNESS_MAX_SIZE: usize =
    4 + BLOCK_HEADER_FITNESS_ELEMENTS * (4 + BLOCK_HEADER_FITNESS_ELEMENT_LENGTH); // total size + 2 elements of (size + bytes)

/// Maximal size of [BlockHeader::protocol_data].
///
pub const BLOCK_HEADER_PROTOCOL_DATA_MAX_SIZE: usize = BLOCK_HEADER_MAX_SIZE
    - (4 + 1
        + HashType::BlockHash.size()
        + 8
        + 1
        + HashType::OperationListListHash.size()
        + 4
        + BLOCK_HEADER_FITNESS_MAX_SIZE
        + HashType::ContextHash.size());
