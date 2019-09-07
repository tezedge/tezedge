//        TODO: dopisat docs poriadne, ked sa utrasie rozhranie

//        TODO: doriesit logovanie v ruste
//        TODO: setup storu pre testy
//        TODO: spravit impl a async vrstvu

//        TODO: domysliet ci treba async na rozhrani, ak hej, tak rozdeli/zaobalit ffi.rs cez ffi_impl.rs
//        TODO: domysliet na rozhrani aby nesli stringy ale realne strukturi

//        TODO: block_header_hash String - zmenit na BlockHash strukturu vsade
//        TODO: operations VecVecString - zmenit na realnu strukturu: OperationsListList alebo len Operations - este uvidime
//        TODO: vyskusat ci sa neda Array zmenit na List a ci to zafunguje potom aj v list ocaml?

//        TODO: error handling pre apply_block

use log::warn;
use ocaml::{Array, Str};

use crate::runtime;
use crate::runtime::OcamlResult;

pub fn get_block_header(block_header_hash: String) -> OcamlResult<Option<String>> {
    runtime::spawn(move || {
        let ocaml_function = ocaml::named_value("get_block_header").expect("function 'get_block_header' is not registered");
        match ocaml_function.call_exn::<Str>(block_header_hash.as_str().into()) {
            Ok(result) => {
                let ocaml_result: Str = result.into();
                Some(ocaml_result.as_str().to_string())
            }
            Err(e) => {
                warn!("No block header found. Reason: {:?}", e);
                None
            }
        }
    })
}

pub fn apply_block(block_header_hash: String, operations: Vec<Vec<String>>) -> OcamlResult<String> {
    runtime::spawn(move || {
        let ocaml_function = ocaml::named_value("apply_block").expect("function 'apply_block' is not registered");
        let ocaml_result: Str = ocaml_function.call2::<Str, Array>(
            block_header_hash.as_str().into(),
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