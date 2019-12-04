extern crate reqwest; //sudo apt-get install libssl-dev
extern crate serde_json;

use serde_json::Value;
use std::collections::HashMap;
use std::mem::discriminant;
use failure::Error;


pub fn call_rpc_json(uri: String) -> Result<HashMap<String, Value>, Error> {
    let json = reqwest::get(&uri)?.json()?;
    println!("resp for uri {} is {:?}", uri, json);
    Ok(json)
}

pub fn call_rpc_ssl_json(uri: String) -> Result<HashMap<String, Value>, Error> {
    let mut res = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?
        .get(&uri)
        .send()?;
    let json: HashMap<String, Value> = res.json()?;
    println!("resp for uri {} is {:?}", uri, json);
    Ok(json)
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