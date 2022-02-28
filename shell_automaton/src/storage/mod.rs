// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod storage_state;

pub use storage_state::*;

pub mod request;

pub mod blocks;
pub mod state_snapshot;

macro_rules! kv_state {
    ($key:path, $value:path, $should_cache:expr) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
        pub struct State {
            pub cache: std::collections::HashMap<$key, Request>,
        }

        impl State {
            pub(super) fn should_cache(key: &$key, value: &$value) -> bool {
                ($should_cache)(key, value)
            }
        }

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub enum Request {
            Pending,
            Ready($value),
            NotFound,
            Error(crate::service::storage_service::StorageError),
        }
    };
}

macro_rules! kv_actions {
    ($key:path, $value:path, $get_action:ident, $ok_action:ident, $err_action:ident) => {
        //#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct $get_action {
            pub key: $key,
        }

        impl $get_action {
            pub fn new<K>(key: K) -> Self
            where
                K: Into<$key>,
            {
                Self { key: key.into() }
            }
        }

        impl crate::EnablingCondition<crate::State> for $get_action {
            fn is_enabled(&self, state: &crate::State) -> bool {
                let _ = state;
                true
            }
        }

        //#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct $ok_action {
            pub key: $key,
            pub value: $value,
        }

        impl crate::EnablingCondition<crate::State> for $ok_action {
            fn is_enabled(&self, state: &crate::State) -> bool {
                let _ = state;
                true
            }
        }

        //#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct $err_action {
            pub key: $key,
            pub error: Error,
        }

        impl crate::EnablingCondition<crate::State> for $err_action {
            fn is_enabled(&self, state: &crate::State) -> bool {
                let _ = state;
                true
            }
        }

        #[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
        #[derive(
            Debug, Clone, serde::Serialize, serde::Deserialize, derive_more::From, thiserror::Error,
        )]
        pub enum Error {
            #[error("Storage error: `{0}`")]
            Storage(crate::service::storage_service::StorageError),
            #[error("Value not found")]
            NotFound,
        }
    };
}

macro_rules! kv_reducer {
    ($state:ident, $get:ident, $get_action:ident, $ok:ident, $ok_action:ident, $err:ident, $err_action:ident) => {
        pub fn reducer(state: &mut crate::State, action: &redux_rs::ActionWithMeta<crate::Action>) {
            use crate::Action;
            let cache = &mut state.storage.$state.cache;
            match &action.action {
                Action::$get($get_action { key }) => {
                    if !cache.contains_key(key) {
                        cache.insert(key.clone(), Request::Pending);
                    }
                }
                Action::$ok($ok_action { key, value }) if State::should_cache(key, value) => {
                    cache
                        .get_mut(key)
                        .map(|cached| *cached = Request::Ready(value.clone()));
                }
                Action::$ok($ok_action { key, value: _ }) => {
                    cache.remove(key);
                }
                Action::$err($err_action { key, error: _ }) => {
                    cache.remove(key);
                }
                // TODO
                _ => (),
            }
        }
    };
}

macro_rules! kv_effects {
    ($request:ident, $success:ident, $error:ident, $get:ident, $get_action:ident, $ok:ident, $ok_action:ident, $err:ident, $err_action:ident) => {
        pub fn effects<S>(
            store: &mut redux_rs::Store<crate::State, S, crate::Action>,
            action: &redux_rs::ActionWithMeta<crate::Action>,
        ) where
            S: crate::Service,
        {
            use crate::service::storage_service::*;
            use crate::storage::request::*;
            use crate::Action;
            match &action.action {
                Action::$get($get_action { key }) => store.dispatch(StorageRequestCreateAction {
                    payload: StorageRequestPayload::$request(key.clone()),
                }),
                Action::StorageRequestSuccess(StorageRequestSuccessAction {
                    result: StorageResponseSuccess::$success(key, Some(value)),
                    ..
                }) => store.dispatch($ok_action {
                    key: key.clone(),
                    value: value.clone(),
                }),
                Action::StorageRequestSuccess(StorageRequestSuccessAction {
                    result: StorageResponseSuccess::$success(key, None),
                    ..
                }) => store.dispatch($err_action {
                    key: key.clone(),
                    error: Error::NotFound,
                }),
                Action::StorageRequestError(StorageRequestErrorAction {
                    error: StorageResponseError::$error(key, error),
                    ..
                }) => store.dispatch($err_action {
                    key: key.clone(),
                    error: error.clone().into(),
                }),
                _ => true,
            };
        }
    };
}

macro_rules! kv_state_machine {
    ($state_name:ident, $storage_req:ident, $storage_success:ident, $storage_error:ident, $key:path, $value:path, $should_cache:expr, $get:ident, $get_action:ident, $ok:ident, $ok_action:ident, $err:ident, $err_action:ident) => {
        kv_state!($key, $value, $should_cache);
        kv_actions!($key, $value, $get_action, $ok_action, $err_action);
        kv_reducer!(
            $state_name,
            $get,
            $get_action,
            $ok,
            $ok_action,
            $err,
            $err_action
        );
        kv_effects!(
            $storage_req,
            $storage_success,
            $storage_error,
            $get,
            $get_action,
            $ok,
            $ok_action,
            $err,
            $err_action
        );
    };
}

pub mod kv_block_meta {
    use crypto::hash::BlockHash;
    use storage::block_meta_storage::Meta;

    kv_state_machine!(
        block_meta,
        BlockMetaGet,
        BlockMetaGetSuccess,
        BlockMetaGetError,
        BlockHash,
        Meta,
        |_hash, _meta| true,
        StorageBlockMetaGet,
        StorageBlockMetaGetAction,
        StorageBlockMetaOk,
        StorageBlockMetaOkAction,
        StorageBlockMetaError,
        StorageBlockMetaErrorAction
    );
}

pub mod kv_block_additional_data {
    use crypto::hash::BlockHash;
    use storage::block_meta_storage::BlockAdditionalData;

    kv_state_machine!(
        block_additional_data,
        BlockAdditionalDataGet,
        BlockAdditionalDataGetSuccess,
        BlockAdditionalDataGetError,
        BlockHash,
        BlockAdditionalData,
        |_hash, _additinal_data| false,
        StorageBlockAdditionalDataGet,
        StorageBlockAdditionalDataGetAction,
        StorageBlockAdditionalDataOk,
        StorageBlockAdditionalDataOkAction,
        StorageBlockAdditionalDataError,
        StorageBlockAdditionalDataErrorAction
    );
}

pub mod kv_block_header {
    use crypto::hash::BlockHash;
    use tezos_messages::p2p::encoding::block_header::BlockHeader;

    kv_state_machine!(
        block_header,
        BlockHeaderGet,
        BlockHeaderGetSuccess,
        BlockHeaderGetError,
        BlockHash,
        BlockHeader,
        |_hash, _header| false,
        StorageBlockHeaderGet,
        StorageBlockHeaderGetAction,
        StorageBlockHeaderOk,
        StorageBlockHeaderOkAction,
        StorageBlockHeaderError,
        StorageBlockHeaderErrorAction
    );
}

pub mod kv_constants {
    use crypto::hash::ProtocolHash;

    kv_state_machine!(
        constants,
        ConstantsGet,
        ConstantsGetSuccess,
        ConstantsGetError,
        ProtocolHash,
        String,
        |_key, _value| false,
        StorageConstantsGet,
        StorageConstantsGetAction,
        StorageConstantsOk,
        StorageConstantsOkAction,
        StorageConstantsError,
        StorageConstantsErrorAction
    );
}

pub mod kv_cycle_meta {
    use std::{convert::TryFrom, num::ParseIntError};

    use storage::cycle_storage::CycleData;

    pub type Cycle = i32;

    #[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    #[serde(into = "String", try_from = "String")]
    pub struct CycleKey(pub Cycle);

    impl From<i32> for CycleKey {
        fn from(source: i32) -> Self {
            Self(source)
        }
    }

    impl From<CycleKey> for i32 {
        fn from(source: CycleKey) -> Self {
            source.0
        }
    }

    impl From<CycleKey> for String {
        fn from(source: CycleKey) -> Self {
            source.0.to_string()
        }
    }

    impl TryFrom<String> for CycleKey {
        type Error = ParseIntError;

        fn try_from(source: String) -> Result<Self, Self::Error> {
            source.parse().map(Self)
        }
    }

    kv_state_machine!(
        cycle_data,
        CycleMetaGet,
        CycleMetaGetSuccess,
        CycleMetaGetError,
        CycleKey,
        CycleData,
        |_cycle, _data| false,
        StorageCycleMetaGet,
        StorageCycleMetaGetAction,
        StorageCycleMetaOk,
        StorageCycleMetaOkAction,
        StorageCycleMetaError,
        StorageCycleMetaErrorAction
    );
}

pub mod kv_cycle_eras {
    use crypto::hash::ProtocolHash;
    use storage::cycle_eras_storage::CycleErasData;

    kv_state_machine!(
        cycle_eras,
        CycleErasGet,
        CycleErasGetSuccess,
        CycleErasGetError,
        ProtocolHash,
        CycleErasData,
        |_proto, _eras| false,
        StorageCycleErasGet,
        StorageCycleErasGetAction,
        StorageCycleErasOk,
        StorageCycleErasOkAction,
        StorageCycleErasError,
        StorageCycleErasErrorAction
    );
}

pub mod kv_operations {
    use crypto::hash::BlockHash;
    use tezos_messages::p2p::encoding::operation::Operation;

    kv_state_machine!(
        operation,
        OperationsGet,
        OperationsGetSuccess,
        OperationsGetError,
        BlockHash,
        Vec<Operation>,
        |_hash, _ops| false,
        StorageOperationsGet,
        StorageOperationsGetAction,
        StorageOperationsOk,
        StorageOperationsOkAction,
        StorageOperationsError,
        StorageOperationsErrorAction
    );
}
