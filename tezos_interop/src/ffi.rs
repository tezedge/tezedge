use ocaml::{Array, Str, Tuple};

use crate::runtime;
use crate::runtime::OcamlError;

pub fn init_storage(storage_data_dir: String) -> Result<(String, String, String), OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("init_storage").expect("function 'init_storage' is not registered");
        match ocaml_function.call_exn::<Str>(storage_data_dir.as_str().into()) {
            Ok(result) => {
                let ocaml_result: Tuple = result.into();
                let chain_id: Str = ocaml_result.get(0).unwrap().into();
                let genesis_block_header_hash: Str = ocaml_result.get(1).unwrap().into();
                let current_block_header_hash: Str = ocaml_result.get(2).unwrap().into();
                (
                    chain_id.as_str().to_string(),
                    genesis_block_header_hash.as_str().to_string(),
                    current_block_header_hash.as_str().to_string()
                )
            }
            Err(e) => {
                panic!("Storage in directory '{}' initialization failed! Reason: {:?}", storage_data_dir, e)
            }
        }
    })
}

pub fn get_current_block_header(chain_id: String) -> Result<String, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("get_current_block_header").expect("function 'get_current_block_header' is not registered");
        let block_header: Str = ocaml_function.call::<Str>(chain_id.as_str().into()).unwrap().into();
        if block_header.is_empty() {
            panic!("No current block header is set, at least genesis should be set!")
        } else {
            block_header.as_str().to_string()
        }
    })
}

pub fn get_block_header(block_header_hash: String) -> Result<Option<String>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("get_block_header").expect("function 'get_block_header' is not registered");
        let block_header: Str = ocaml_function.call::<Str>(block_header_hash.as_str().into()).unwrap().into();
        if block_header.is_empty() {
            None
        } else {
            Some(block_header.as_str().to_string())
        }
    })
}

pub fn apply_block(block_header_hash: String, block_header: String, operations: Vec<Vec<String>>) -> Result<String, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("apply_block").expect("function 'apply_block' is not registered");
        let ocaml_result: Str = ocaml_function.call3::<Str, Str, Array>(
            block_header_hash.as_str().into(),
            block_header.as_str().into(),
            operations_to_ocaml_array(operations),
        ).unwrap().into();
        ocaml_result.as_str().to_string()
    })
}

fn operations_to_ocaml_array(operations: Vec<Vec<String>>) -> Array {
    let mut operations_for_ocaml = Array::new(operations.len());

    operations.iter()
        .enumerate()
        .for_each(|(ops_idx, ops)| {
            let mut ops_array = Array::new(ops.len());
            ops.iter()
                .enumerate()
                .for_each(|(op_idx, op)| {
                    ops_array
                        .set(op_idx, Str::from(op.as_str()).into())
                        .expect("Failed to add operation to Array!");
                });
            operations_for_ocaml
                .set(ops_idx, ops_array.into())
                .expect("Failed to add operations to Array!");
        });

    operations_for_ocaml
}