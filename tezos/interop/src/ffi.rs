use failure::Fail;
use ocaml::{Array1, Error, List, Str, Tag, Tuple, Value};
use serde::{Deserialize, Serialize};

use crate::runtime;
use crate::runtime::OcamlError;

pub type OcamlBytes = Array1<u8>;
pub type RustBytes = Vec<u8>;

pub trait Interchange<T> {
    fn convert_to(&self) -> T;
}

impl Interchange<OcamlBytes> for RustBytes {
    fn convert_to(&self) -> OcamlBytes {
        // convert RustBytes to dedicated struct for ocaml ffi
        // Array1.as_slice dont work here, so we have to copy manually
        // create vs as_slice differs in bigarray::Managed
        let mut array: OcamlBytes = Array1::<u8>::create(self.len());
        for (idx, byte) in self.iter().enumerate() {
            array.data_mut()[idx] = *byte;
        }
        array
    }
}

impl Interchange<RustBytes> for OcamlBytes {
    fn convert_to(&self) -> RustBytes {
        self.data().to_vec()
    }
}

/// Holds configuration for ocaml runtime - e.g. arguments which are passed to ocaml and can be change in runtime
#[derive(Serialize, Deserialize, Debug)]
pub struct OcamlRuntimeConfiguration {
    pub log_enabled: bool
}

#[derive(Serialize, Deserialize, Debug, Fail)]
pub enum OcamlRuntimeConfigurationError {
    #[fail(display = "Change ocaml settings failed, message: {}!", message)]
    ChangeConfigurationError {
        message: String
    }
}

impl From<ocaml::Error> for OcamlRuntimeConfigurationError {
    fn from(error: ocaml::Error) -> Self {
        match error {
            Error::Exception(ffi_error) => {
                OcamlRuntimeConfigurationError::ChangeConfigurationError {
                    message: parse_error_message(ffi_error).unwrap_or_else(|| "unknown".to_string())
                }
            },
            _ => panic!("Ocaml settings failed! Reason: {:?}", error)
        }
    }
}

pub fn change_runtime_configuration(settings: OcamlRuntimeConfiguration) -> Result<Result<(), OcamlRuntimeConfigurationError>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("change_runtime_configuration").expect("function 'change_runtime_configuration' is not registered");
        match ocaml_function.call_exn::<Value>(Value::bool(settings.log_enabled)) {
            Ok(_) => {
                Ok(())
            }
            Err(e) => {
                Err(OcamlRuntimeConfigurationError::from(e))
            }
        }
    })
}

#[derive(Debug)]
pub struct OcamlStorageInitInfo {
    pub chain_id: RustBytes,
    pub genesis_block_header_hash: RustBytes,
    pub genesis_block_header: RustBytes,
    pub current_block_header_hash: RustBytes,
    pub supported_protocol_hashes: Vec<RustBytes>,
}

#[derive(Serialize, Deserialize, Debug, Fail)]
pub enum OcamlStorageInitError {
    #[fail(display = "Ocaml storage init failed, message: {}!", message)]
    InitializeError {
        message: String
    }
}

impl From<ocaml::Error> for OcamlStorageInitError {
    fn from(error: ocaml::Error) -> Self {
        match error {
            Error::Exception(ffi_error) => {
                OcamlStorageInitError::InitializeError {
                    message: parse_error_message(ffi_error).unwrap_or_else(|| "unknown".to_string())
                }
            },
            _ => panic!("Storage initialization failed! Reason: {:?}", error)
        }
    }
}

impl slog::Value for OcamlStorageInitError {
    fn serialize(&self, _record: &slog::Record, key: slog::Key, serializer: &mut dyn slog::Serializer) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

#[derive(Debug)]
pub struct GenesisChain {
    pub time: String,
    pub block: String,
    pub protocol: String,
}

pub fn init_storage(storage_data_dir: String, genesis: &'static GenesisChain) -> Result<Result<OcamlStorageInitInfo, OcamlStorageInitError>, OcamlError> {
    runtime::execute(move || {
        let mut genesis_tuple: Tuple = Tuple::new(3);
        genesis_tuple.set(0, Str::from(genesis.time.as_str()).into()).unwrap();
        genesis_tuple.set(1, Str::from(genesis.block.as_str()).into()).unwrap();
        genesis_tuple.set(2, Str::from(genesis.protocol.as_str()).into()).unwrap();

        let ocaml_function = ocaml::named_value("init_storage").expect("function 'init_storage' is not registered");
        match ocaml_function.call2_exn::<Str, Value>(storage_data_dir.as_str().into(), Value::from(genesis_tuple)) {
            Ok(result) => {
                let ocaml_result: Tuple = result.into();

                let headers: Tuple = ocaml_result.get(0).unwrap().into();

                let chain_id: OcamlBytes = headers.get(0).unwrap().into();
                let genesis_block_header_hash: OcamlBytes = headers.get(1).unwrap().into();
                let genesis_block_header: OcamlBytes = headers.get(2).unwrap().into();
                let current_block_header_hash: OcamlBytes = headers.get(3).unwrap().into();

                // list
                let supported_protocol_hashes: List = ocaml_result.get(1).unwrap().into();
                let supported_protocol_hashes: Vec<RustBytes> = supported_protocol_hashes.to_vec()
                    .iter()
                    .map(|protocol_hash| {
                        let protocol_hash: OcamlBytes = protocol_hash.clone().into();
                        protocol_hash.convert_to()
                    })
                    .collect();

                Ok(OcamlStorageInitInfo {
                    chain_id: chain_id.convert_to(),
                    genesis_block_header_hash: genesis_block_header_hash.convert_to(),
                    genesis_block_header: genesis_block_header.convert_to(),
                    current_block_header_hash: current_block_header_hash.convert_to(),
                    supported_protocol_hashes,
                })
            }
            Err(e) => {
                Err(OcamlStorageInitError::from(e))
            }
        }
    })
}

#[derive(Debug, Fail)]
pub enum BlockHeaderError {
    #[fail(display = "BlockHeader cannot be read from storage: {}!", message)]
    ReadError {
        message: String
    },
    #[fail(display = "BlockHeader was expected, but was not found!")]
    ExpectedButNotFound,
}

impl From<ocaml::Error> for BlockHeaderError {
    fn from(error: ocaml::Error) -> Self {
        match error {
            Error::Exception(ffi_error) => {
                BlockHeaderError::ReadError {
                    message: parse_error_message(ffi_error).unwrap_or_else(|| "unknown".to_string())
                }
            },
            _ => panic!("Storage initialization failed! Reason: {:?}", error)
        }
    }
}

pub fn get_current_block_header(chain_id: RustBytes) -> Result<Result<RustBytes, BlockHeaderError>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("get_current_block_header").expect("function 'get_current_block_header' is not registered");
        match ocaml_function.call_exn::<OcamlBytes>(chain_id.convert_to()) {
            Ok(block_header) => {
                let block_header: OcamlBytes = block_header.into();
                if block_header.is_empty() {
                    Err(BlockHeaderError::ExpectedButNotFound)
                } else {
                    Ok(block_header.convert_to())
                }
            }
            Err(e) => {
                Err(BlockHeaderError::from(e))
            }
        }
    })
}

pub fn get_block_header(chain_id: RustBytes, block_header_hash: RustBytes) -> Result<Result<Option<RustBytes>, BlockHeaderError>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("get_block_header").expect("function 'get_block_header' is not registered");
        match ocaml_function.call2_exn::<OcamlBytes, OcamlBytes>(chain_id.convert_to(), block_header_hash.convert_to()) {
            Ok(block_header) => {
                let block_header: OcamlBytes = block_header.into();
                if block_header.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(block_header.convert_to()))
                }
            }
            Err(e) => {
                Err(BlockHeaderError::from(e))
            }
        }
    })
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApplyBlockResult {
    pub validation_result_message: String
}

#[derive(Serialize, Deserialize, Debug, Fail, PartialEq)]
pub enum ApplyBlockError {
    #[fail(display = "Incomplete operations, exptected: {}, has actual: {}!", expected, actual)]
    IncompleteOperations {
        expected: usize,
        actual: usize,
    },
    #[fail(display = "Failed to apply block - message: {}!", message)]
    FailedToApplyBlock {
        message: String,
    },
    #[fail(display = "Unknown predecessor - try to fetch predecessor at first!")]
    UnknownPredecessor,
    #[fail(display = "Invalid block header data - message: {}!", message)]
    InvalidBlockHeaderData {
        message: String,
    },
    #[fail(display = "Invalid operations data - message: {}!", message)]
    InvalidOperationsData {
        message: String,
    },
}

impl From<ocaml::Error> for ApplyBlockError {
    fn from(error: ocaml::Error) -> Self {
        match error {
            Error::Exception(ffi_error) => {
                match parse_error_message(ffi_error) {
                    None => ApplyBlockError::FailedToApplyBlock {
                        message: "unknown".to_string()
                    },
                    Some(message) => {
                        match message.as_str() {
                            "UnknownPredecessor" => ApplyBlockError::UnknownPredecessor,
                            message => ApplyBlockError::FailedToApplyBlock {
                                message: message.to_string()
                            }
                        }
                    }
                }
            },
            _ => panic!("Unhandled ocaml error occurred for apply block! Error: {:?}", error)
        }
    }
}

pub fn apply_block(
    chain_id: RustBytes,
    block_header_hash: RustBytes,
    block_header: RustBytes,
    operations: Vec<Option<Vec<RustBytes>>>)
    -> Result<Result<ApplyBlockResult, ApplyBlockError>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("apply_block").expect("function 'apply_block' is not registered");

        // convert to ocaml types
        let block_header_tuple: Tuple = block_header_to_ocaml(&block_header_hash, &block_header)?;
        let operations = operations_to_ocaml(&operations);

        // call ffi
        match ocaml_function.call3_exn::<OcamlBytes, Value, List>(
            chain_id.convert_to(),
            block_header_tuple.into(),
            operations,
        ) {
            Ok(validation_result) => {
                let validation_result: Str = validation_result.into();
                Ok(ApplyBlockResult {
                    validation_result_message: validation_result.as_str().to_string()
                })
            },
            Err(e) => {
                Err(ApplyBlockError::from(e))
            }
        }
    })
}

pub fn operations_to_ocaml(operations: &Vec<Option<Vec<RustBytes>>>) -> List {
    let mut operations_for_ocaml = List::new();

    operations.into_iter().rev()
        .for_each(|ops_option| {
            let ops_array = if let Some(ops) = ops_option {
                let mut ops_array = List::new();
                ops.into_iter().rev().for_each(|op| {
                    ops_array.push_hd(Value::from(op.convert_to()));
                });
                ops_array
            } else {
                List::new()
            };
            operations_for_ocaml.push_hd(Value::from(ops_array));
        });

    operations_for_ocaml
}

pub fn block_header_to_ocaml(block_header_hash: &RustBytes, block_header: &RustBytes) -> Result<Tuple, ocaml::Error> {
    let mut block_header_tuple: Tuple = Tuple::new(2);
    block_header_tuple.set(0, Value::from(block_header_hash.convert_to()))?;
    block_header_tuple.set(1, Value::from(block_header.convert_to()))?;
    Ok(block_header_tuple)
}

fn parse_error_message(ffi_error: Value) -> Option<String> {
    if ffi_error.is_block() {
        // for exceptions, in the field 2, there is a message for Failure or Ffi_error
        let error_message = ffi_error.field(1);
        if error_message.tag() == Tag::String {
            let error_message: Str = error_message.into();
            return Some(error_message.as_str().to_string())
        }
    }
    None
}