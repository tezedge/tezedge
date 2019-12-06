extern crate reqwest; //sudo apt-get install libssl-dev
extern crate serde_json;

use serde_json::Value;
use std::mem::discriminant;
use failure::Error;


// pub fn call_rpc_json(uri: String, ssl: bool) -> Result<Value, Error> {
//     if ssl {
//         call_rpc_json_ssl(uri)
//     } else {
//         call_rpc_json_nossl(uri)
//     }
// }

// this is not supporting ssl
// pub fn call_rpc_json_nossl(uri: String) -> Result<Value, Error> {
//     let json = reqwest::get(&uri)?.json()?;
//     println!("resp for uri {} is {:?}", uri, json);
//     Ok(json)
// }

pub fn call_rpc_json(uri: String) -> Result<Value, Error> {
    let mut res = reqwest::Client::builder()
        .danger_accept_invalid_certs(true).build()?
        .get(&uri).send()?;
    let json: Value = res.json()?;
    //println!("resp for uri {} is {:?}", uri, json);
    Ok(json)
}

fn unpack_value_option(opt: Option<&Value>) -> &Value {
    match opt {
        Some(v) => v,
        None => &Value::Null
    }
}

// compare json structures of a single layer Object
pub fn json_structure_match( json1: &Value, json2: &Value) -> bool {
    match (json1, json2) {
        (Value::Object(j1), Value::Object(j2)) => 
            j1.len() == j2.len() && j1.keys().all(|k| j2.contains_key(k) 
                && discriminant(unpack_value_option(j1.get(k))) == discriminant(unpack_value_option(j2.get(k)))),
        _ => false
    }
}