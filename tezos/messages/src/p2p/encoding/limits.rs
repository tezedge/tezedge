// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

/// P2P Point length
/// `ip4:port` or `[ip6]:port`, ip4 max is 3 * 4 + 3, port max is 5
/// ipv6 is 8 * 4 (octets) + 8 (: separators) + 2 ([]) + 5 (port)
///
/// OCaml ref: tezos/src/lib_p2p/p2p_message.ml:64 (comment)
///
pub const P2P_POINT_MAX_LENGTH: usize = 8 * 4 + 8 + 2 + 5;

/// Peer fixed length
///
/// OCaml ref: tezos/src/lib_p2p/p2p_message.ml:81 (comment)
pub const PEER_ID_LENGTH: usize = 16;

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

/// CurrentBranch history max length
///
/// OCaml ref: tezos/src/lib_shell/state.ml:1794,
/// `max_locator_size`
///
/// ```ocaml
/// let max_locator_size = 200
/// ````
pub const CURRENT_BRANCH_HISTORY_MAX_LENGTH: usize = 200;

/// Block header max size
///
/// OCaml ref: tezos/src/lib_shell/distributed_db_message.ml:30,
/// `block_header_max_size`
///
/// ```ocaml
///   let block_header_max_size = ref (Some (8 * 1024 * 1024))
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

/// Mempool size
///
/// Not bounded in OCaml. Calculated on the idea that there should't be more
/// operations than can exist in a single block.
///
/// TODO Check with Tezos
/// https://simplestakingcom.atlassian.net/browse/TE-407
pub const MEMPOOL_MAX_SIZE: usize = OPERATION_MAX_PASS * 1024 * 1024 * 32;

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

/// Max size of Protocol::components encoding, two bytes less than Protocol
pub const PROTOCOL_COMPONENT_MAX_SIZE: usize = PROTOCOL_MAX_SIZE - 2;

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
