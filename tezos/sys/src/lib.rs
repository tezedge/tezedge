// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

//! Bindings to tezedge FFI functions in libtezos.so.

#[link(name = "tezos")]
extern "C" {
    pub fn initialize_tezedge_context_callbacks(
        tezedge_context_commit: unsafe extern "C" fn(isize, isize, isize, isize) -> isize,
        tezedge_context_hash: unsafe extern "C" fn(isize, isize, isize, isize) -> isize,
        tezedge_context_find_tree: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_context_add_tree: unsafe extern "C" fn(isize, isize, isize) -> isize,
        tezedge_context_remove: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_context_add: unsafe extern "C" fn(isize, isize, isize) -> isize,
        tezedge_context_find: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_context_mem_tree: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_context_mem: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_context_list: unsafe extern "C" fn(isize, isize, isize, isize) -> isize,
        tezedge_context_get_tree: unsafe extern "C" fn(isize) -> isize,
        tezedge_context_set_tree: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_context_empty: unsafe extern "C" fn(isize) -> isize,
        tezedge_context_dump: unsafe extern "C" fn(isize, isize, isize) -> isize,
        tezedge_context_restore: unsafe extern "C" fn(isize, isize, isize, isize) -> isize,
    );

    pub fn initialize_tezedge_tree_callbacks(
        tezedge_tree_hash: unsafe extern "C" fn(isize) -> isize,
        tezedge_tree_find_tree: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_tree_add_tree: unsafe extern "C" fn(isize, isize, isize) -> isize,
        tezedge_tree_remove: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_tree_add: unsafe extern "C" fn(isize, isize, isize) -> isize,
        tezedge_tree_find: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_tree_mem_tree: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_tree_mem: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_tree_list: unsafe extern "C" fn(isize, isize, isize, isize) -> isize,
        tezedge_tree_walker_make: unsafe extern "C" fn(isize, isize, isize, isize) -> isize,
        tezedge_tree_walker_next: unsafe extern "C" fn(isize) -> isize,
        tezedge_tree_empty: unsafe extern "C" fn(isize) -> isize,
        tezedge_tree_to_value: unsafe extern "C" fn(isize) -> isize,
        tezedge_tree_of_value: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_tree_is_empty: unsafe extern "C" fn(isize) -> isize,
        tezedge_tree_equal: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_tree_kind: unsafe extern "C" fn(isize) -> isize,
    );

    pub fn initialize_tezedge_index_callbacks(
        tezedge_index_patch_context_get: unsafe extern "C" fn(isize) -> isize,
        tezedge_index_checkout: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_index_exists: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_index_close: unsafe extern "C" fn(isize) -> isize,
        tezedge_index_block_applied: unsafe extern "C" fn(isize, isize, isize) -> isize,
        tezedge_index_init: unsafe extern "C" fn(isize, isize) -> isize,
        tezedge_index_latest_context_hashes: unsafe extern "C" fn(isize, isize) -> isize,
    );

    pub fn initialize_tezedge_timing_callbacks(
        tezedge_timing_set_block: unsafe extern "C" fn(isize) -> isize,
        tezedge_timing_set_protocol: unsafe extern "C" fn(isize) -> isize,
        tezedge_timing_checkout: unsafe extern "C" fn(isize, f64, f64) -> isize,
        tezedge_timing_set_operation: unsafe extern "C" fn(isize) -> isize,
        tezedge_timing_commit: unsafe extern "C" fn(isize, f64, f64) -> isize,
        tezedge_timing_context_action: unsafe extern "C" fn(isize, isize, f64, f64) -> isize,
        tezedge_timing_init: unsafe extern "C" fn(isize) -> isize,
    );
}

/// This function does nothing. It exists to force cargo to link libtezos to crates
/// that depend on tezos-sys.
////
/// To do so, define a `pub` function in the crate that requires libtezos linking that
/// calls this function.
///
/// The problem is that cargo seems to not link in stuff that doesn't get called
/// (but I have only experienced these issues when running the tests with ASAN enabled, and not always).
/// What is done with this function is to add a call from the crates that require libtezos to be linked
/// (the call does nothing), that way cargo sees that the crate is used which forces it to be linked.
///
/// It doesn't seem to be required under normal compilation conditions, but when
/// running the tests with the address sanitizer enabled, linking fails without
/// this extra step.
pub fn force_libtezos_linking() {}
