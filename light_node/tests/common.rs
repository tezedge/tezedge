extern crate reqwest; //sudo apt-get install libssl-dev
extern crate serde_json;

use serde_json::Value;
use std::collections::HashMap;
use std::mem::discriminant;
use failure::Error;


pub fn compare_json_structure_of_rpc(uri1: String, uri2: String) -> Result<bool, Error> {
    let res1 = call_rpc(uri1)?;
    let res2 = call_rpc(uri2)?;

    let json1: HashMap<String, Value> = serde_json::from_str(&res1)?;
    let json2: HashMap<String, Value> = serde_json::from_str(&res2)?;

    //compare structure of json
    Ok(json_structure_match(&json1, &json2))
}

fn call_rpc(request_url: String) -> Result<String, reqwest::Error> {
    let response_string = reqwest::get(&request_url)?.text()?;
    Ok(response_string)
}

fn unpack_value_option(opt: Option<&Value>) -> &Value {
    match opt {
        Some(v) => v,
        None => &Value::Null
    }
}

pub fn json_structure_match( map1: &HashMap<String, Value>, map2: &HashMap<String, Value>) -> bool {
    map1.len() == map2.len() && map1.keys().all(|k| map2.contains_key(k) 
        && discriminant(unpack_value_option(map1.get(k))) == discriminant(unpack_value_option(map2.get(k))))
}