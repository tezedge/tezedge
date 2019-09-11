pub fn init_storage(storage_data_dir: String) -> (String, String) {
    ("".to_string(), "".to_string())
}

pub fn get_current_block_header(chain_id: String) -> String {
    "".to_string()
}

pub fn get_block_header(block_header_hash: String) -> Option<String> {
    Some("".to_string())
}

pub fn apply_block(block_header_hash: String, block_header: String, operations: Vec<Vec<String>>) -> String {
    "".to_string()
}