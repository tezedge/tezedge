// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module provides rpc services and exposes also protocol rpc services.

pub mod base_services;
pub mod context;
pub mod dev_services;
pub mod mempool_services;
pub mod protocol;
// pub mod stats_services;
pub mod stream_services;

pub mod cache_warm_up {
    use crate::server::RpcServiceEnvironment;
    use crypto::hash::ChainId;
    use std::sync::Arc;
    use storage::BlockHeaderWithHash;

    pub async fn warm_up_rpc_cache(
        chain_id: ChainId,
        block: Arc<BlockHeaderWithHash>,
        env: Arc<RpcServiceEnvironment>,
    ) {
        // async -> sync calls: goes first because other calls re-use the cached result
        let get_additional_data = async {
            crate::services::base_services::get_additional_data_or_fail(
                &chain_id,
                &block.hash,
                env.persistent_storage(),
            )
        };
        let get_raw_block_header_with_hash = async {
            crate::services::base_services::get_raw_block_header_with_hash(
                &chain_id,
                &block.hash,
                env.persistent_storage(),
            )
        };
        let get_block_with_json_data = async {
            crate::services::base_services::get_block_with_json_data(
                &chain_id,
                &block.hash,
                env.persistent_storage(),
            )
        };

        let _ = tokio::join!(
            get_additional_data,
            get_raw_block_header_with_hash,
            get_block_with_json_data
        );

        // Async calls
        let get_block_metadata =
            crate::services::base_services::get_block_metadata(&chain_id, &block.hash, &env);
        let get_block = crate::services::base_services::get_block(&chain_id, &block.hash, &env);
        let get_block_operations_metadata =
            crate::services::base_services::get_block_operations_metadata(
                chain_id.clone(),
                &block.hash,
                &env,
            );
        let get_block_operation_hashes = crate::services::base_services::get_block_operation_hashes(
            chain_id.clone(),
            &block.hash,
            &env,
        );
        let get_block_header = crate::services::base_services::get_block_header(
            chain_id.clone(),
            block.hash.clone(),
            &env.persistent_storage(),
        );
        let get_block_protocols = async {
            crate::services::base_services::get_block_protocols(
                &chain_id,
                &block.hash,
                &env.persistent_storage(),
            )
        };
        let live_blocks = async {
            crate::services::base_services::live_blocks(&chain_id, block.hash.clone(), &env)
        };
        let get_block_shell_header = async {
            crate::services::base_services::get_block_shell_header_or_fail(
                &chain_id,
                block.hash.clone(),
                &env.persistent_storage(),
            )
        };

        let _ = tokio::join!(
            get_block_metadata,
            get_block,
            get_block_operations_metadata,
            get_block_operation_hashes,
            get_block_header,
            get_block_protocols,
            live_blocks,
            get_block_shell_header,
        );
    }
}

#[cfg(test)]
mod tests {
    use crypto::hash::{BlockHash, ChainId, ProtocolHash};
    use std::convert::{TryFrom, TryInto};
    use storage::tests_common::TmpStorage;
    use storage::{BlockAdditionalData, BlockMetaStorage, BlockMetaStorageReader};

    #[test]
    fn test_do_not_cache_error() {
        // prepare storage
        let tmp_storage = TmpStorage::create_to_out_dir("__do_not_cache_optional_storage")
            .expect("failed to create storage");
        let block_meta_storage = BlockMetaStorage::new(tmp_storage.storage());
        let chain_id = ChainId::try_from("NetXgtSLGNJvNye").unwrap();

        // prepare block data
        let block_hash: BlockHash = vec![0; 32].try_into().expect("failed to create BlockHash");
        let metadata = BlockAdditionalData::new(
            17,
            16,
            ProtocolHash::from_base58_check("PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb")
                .expect("failed to create protocol_hash"),
            ProtocolHash::from_base58_check("PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb")
                .expect("failed to create protocol_hash"),
            None,
            None,
            None,
        );

        // check no data set to db
        assert!(matches!(
            block_meta_storage.get_additional_data(&block_hash),
            Ok(None)
        ));

        // check rpc call
        assert!(super::base_services::get_additional_data_or_fail(
            &chain_id,
            &block_hash,
            tmp_storage.storage()
        )
        .is_err());
        assert!(super::base_services::get_additional_data_or_fail(
            &chain_id,
            &block_hash,
            tmp_storage.storage()
        )
        .is_err());

        // store metadata
        block_meta_storage
            .put_block_additional_data(&block_hash, &metadata)
            .expect("failed to save metadata");

        // check stored
        let stored = block_meta_storage
            .get_additional_data(&block_hash)
            .expect("failed to get metadata");
        assert!(stored.is_some());

        // check rpc once more
        let rpc_result = super::base_services::get_additional_data_or_fail(
            &chain_id,
            &block_hash,
            tmp_storage.storage(),
        );
        assert!(rpc_result.is_ok());

        // check the same result
        let stored = stored.unwrap();
        let rpc_result = rpc_result.unwrap();
        assert_eq!(stored.protocol_hash, rpc_result.protocol_hash);
        assert_eq!(stored.next_protocol_hash, rpc_result.next_protocol_hash);
        assert_eq!(stored.max_operations_ttl(), rpc_result.max_operations_ttl());
        assert_eq!(
            stored.last_allowed_fork_level(),
            rpc_result.last_allowed_fork_level()
        );
        assert_eq!(
            stored.block_metadata_hash(),
            rpc_result.block_metadata_hash()
        );
        assert_eq!(stored.ops_metadata_hash(), rpc_result.ops_metadata_hash());
        assert_eq!(
            stored.ops_metadata_hashes(),
            rpc_result.ops_metadata_hashes()
        );
    }
}
