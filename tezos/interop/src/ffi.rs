use failure::Fail;
use ocaml::{Array, Error, Str, Tag, Tuple, Value};

use crate::runtime;
use crate::runtime::OcamlError;

/// Holds configuration for ocaml runtime - e.g. arguments which are passed to ocaml and can be change in runtime
#[derive(Debug)]
pub struct OcamlRuntimeConfiguration {
    pub log_enabled: bool
}

#[derive(Debug, Fail)]
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
    pub chain_id: String,
    pub genesis_block_header_hash: String,
    pub genesis_block_header: String,
    pub current_block_header_hash: String,
}

#[derive(Debug, Fail)]
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

pub fn init_storage(storage_data_dir: String) -> Result<Result<OcamlStorageInitInfo, OcamlStorageInitError>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("init_storage").expect("function 'init_storage' is not registered");
        match ocaml_function.call_exn::<Str>(storage_data_dir.as_str().into()) {
            Ok(result) => {
                let ocaml_result: Tuple = result.into();
                let chain_id: Str = ocaml_result.get(0).unwrap().into();
                let genesis_block_header_hash: Str = ocaml_result.get(1).unwrap().into();
                let genesis_block_header: Str = ocaml_result.get(2).unwrap().into();
                let current_block_header_hash: Str = ocaml_result.get(3).unwrap().into();
                Ok(OcamlStorageInitInfo {
                    chain_id: chain_id.as_str().to_string(),
                    genesis_block_header_hash: genesis_block_header_hash.as_str().to_string(),
                    genesis_block_header: genesis_block_header.as_str().to_string(),
                    current_block_header_hash: current_block_header_hash.as_str().to_string(),
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

pub fn get_current_block_header(chain_id: String) -> Result<Result<String, BlockHeaderError>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("get_current_block_header").expect("function 'get_current_block_header' is not registered");
        match ocaml_function.call_exn::<Str>(chain_id.as_str().into()) {
            Ok(block_header) => {
                let block_header: Str = block_header.into();
                if block_header.is_empty() {
                    Err(BlockHeaderError::ExpectedButNotFound)
                } else {
                    Ok(block_header.as_str().to_string())
                }
            }
            Err(e) => {
                Err(BlockHeaderError::from(e))
            }
        }
    })
}

pub fn get_block_header(block_header_hash: String) -> Result<Result<Option<String>, BlockHeaderError>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("get_block_header").expect("function 'get_block_header' is not registered");
        match ocaml_function.call_exn::<Str>(block_header_hash.as_str().into()) {
            Ok(block_header) => {
                let block_header: Str = block_header.into();
                if block_header.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(block_header.as_str().to_string()))
                }
            }
            Err(e) => {
                Err(BlockHeaderError::from(e))
            }
        }
    })
}

#[derive(Debug)]
pub struct ApplyBlockResult {
    pub validation_result_message: String
}

#[derive(Debug, Fail, PartialEq)]
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
    #[fail(display = "Invalid block header data")]
    InvalidBlockHeaderData,
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

pub fn apply_block(block_header_hash: String, block_header: String, operations: Vec<Option<Vec<String>>>)
                   -> Result<Result<ApplyBlockResult, ApplyBlockError>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("apply_block").expect("function 'apply_block' is not registered");
        match ocaml_function.call3_exn::<Str, Str, Array>(
            block_header_hash.as_str().into(),
            block_header.as_str().into(),
            operations_to_ocaml_array(operations),
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

fn operations_to_ocaml_array(operations: Vec<Option<Vec<String>>>) -> Array {
    let mut operations_for_ocaml = Array::new(operations.len());

    operations.iter()
        .enumerate()
        .for_each(|(ops_idx, ops_option)| {
            let ops_array = if let Some(ops) = ops_option {
                let mut ops_array = Array::new(ops.len());
                ops.iter()
                    .enumerate()
                    .for_each(|(op_idx, op)| {
                        ops_array
                            .set(op_idx, Str::from(op.as_str()).into())
                            .expect("Failed to add operation to Array!");
                    });
                ops_array
            } else {
                Array::new(0)
            };
            operations_for_ocaml
                .set(ops_idx, ops_array.into())
                .expect("Failed to add operations to Array!");
        });

    operations_for_ocaml
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