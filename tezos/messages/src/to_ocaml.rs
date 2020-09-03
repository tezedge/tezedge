// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::p2p::encoding::prelude::BlockHeader;
use crate::p2p::encoding::prelude::Operation;
use crypto::hash::{BlockHash, ContextHash, OperationListListHash};
use znfe::{
    ocaml, ocaml_alloc, ocaml_frame, to_ocaml, Intnat, OCaml, OCamlAllocResult, OCamlAllocToken,
    OCamlBytes, OCamlInt32, OCamlInt64, OCamlList, ToOCaml,
};

struct BlockHeaderShellHeader {}
struct OperationShellHeader {}
struct OCamlOperationListListHash {}
struct OCamlBlockHash {}
struct OCamlContextHash {}

ocaml! {
    alloc fn alloc_block_header_shell_header(
        level: OCamlInt32,
        proto_level: Intnat,
        predecessor: OCamlBlockHash,
        timestamp: OCamlInt64,
        validation_passes: Intnat,
        operations_hash: OCamlOperationListListHash,
        fitness: OCamlList<OCamlBytes>,
        context: OCamlContextHash,
    ) -> BlockHeaderShellHeader;

    alloc fn alloc_block_header(
        shell: BlockHeaderShellHeader,
        protocol_data: OCamlBytes,
    ) -> BlockHeader;

    alloc fn alloc_operation_shell_header(
        branch: OCamlBlockHash,
    ) -> OperationShellHeader;

    alloc fn alloc_operation(
        shell: OperationShellHeader,
        proto: OCamlBytes,
    ) -> Operation;

    alloc fn alloc_operation_list_list_hash(
        hash: OCamlBytes,
    ) -> OCamlOperationListListHash;

    alloc fn alloc_block_hash(
        hash: OCamlBytes,
    ) -> OCamlBlockHash;

    alloc fn alloc_context_hash(
        hash: OCamlBytes,
    ) -> OCamlContextHash;
}

unsafe impl ToOCaml<OCamlOperationListListHash> for OperationListListHash {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<OCamlOperationListListHash> {
        ocaml_frame!(gc, {
            let hash = to_ocaml!(gc, self);
            unsafe { alloc_operation_list_list_hash(token, hash) }
        })
    }
}

unsafe impl ToOCaml<OCamlBlockHash> for BlockHash {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<OCamlBlockHash> {
        ocaml_frame!(gc, {
            let hash = to_ocaml!(gc, self);
            unsafe { alloc_block_hash(token, hash) }
        })
    }
}

unsafe impl ToOCaml<OCamlContextHash> for ContextHash {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<OCamlContextHash> {
        ocaml_frame!(gc, {
            let hash = to_ocaml!(gc, self);
            unsafe { alloc_context_hash(token, hash) }
        })
    }
}

unsafe impl ToOCaml<BlockHeader> for BlockHeader {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<BlockHeader> {
        ocaml_frame!(gc, {
            let ref level = to_ocaml!(gc, self.level()).keep(gc);
            let proto_level = OCaml::of_int(self.proto() as i64);
            let ref predecessor = to_ocaml!(gc, self.predecessor()).keep(gc);
            let ref timestamp = to_ocaml!(gc, self.timestamp()).keep(gc);
            let validation_passes = OCaml::of_int(self.validation_pass() as i64);
            let ref operations_hash = to_ocaml!(gc, self.operations_hash()).keep(gc);
            let ref fitness = to_ocaml!(gc, self.fitness()).keep(gc);
            let ref context = to_ocaml!(gc, self.context()).keep(gc);
            let ref protocol_data = to_ocaml!(gc, self.protocol_data()).keep(gc);
            let shell_header = unsafe {
                ocaml_alloc!(alloc_block_header_shell_header(
                    gc,
                    gc.get(level),
                    proto_level,
                    gc.get(predecessor),
                    gc.get(timestamp),
                    validation_passes,
                    gc.get(operations_hash),
                    gc.get(fitness),
                    gc.get(context),
                ))
            };
            unsafe { alloc_block_header(token, shell_header, gc.get(protocol_data)) }
        })
    }
}

unsafe impl ToOCaml<Operation> for Operation {
    fn to_ocaml(&self, token: OCamlAllocToken) -> OCamlAllocResult<Operation> {
        ocaml_frame!(gc, {
            let ref branch = to_ocaml!(gc, self.branch()).keep(gc);
            let ref proto = to_ocaml!(gc, self.data()).keep(gc);
            let shell = unsafe { ocaml_alloc!(alloc_operation_shell_header(gc, gc.get(branch))) };
            unsafe { alloc_operation(token, shell, gc.get(proto)) }
        })
    }
}
