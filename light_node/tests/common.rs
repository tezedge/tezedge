extern crate reqwest; //sudo apt-get install libssl-dev
extern crate serde_json;

use serde_json::Value;
use std::mem::discriminant;
use failure::Error;

// configuration
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

pub fn get_config() -> Result<Value, Error> {
    let cfg_file = PathBuf::from("./etc/tezedge/integ_tests.json");
    let mut file = File::open(cfg_file)?;
    let mut data = String::new();
    file.read_to_string(&mut data)?;

    let cfg_json: Value = serde_json::from_str(&data)?;
    Ok(cfg_json)
}

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

pub fn call_rpc(url: &str) -> Result<reqwest::Response, reqwest::Error> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true).build()?
        .get(url).send()
}

pub fn call_rpc_json(url: &str) -> Result<Value, Error> {
    let mut res = reqwest::Client::builder()
        .danger_accept_invalid_certs(true).build()?
        .get(url).send()?;
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

// compare json structures of a single layer Object(Map<String, Value>)
pub fn json_structure_match( json1: &Value, json2: &Value) -> bool {
    match (json1, json2) {
        (Value::Object(j1), Value::Object(j2)) => 
            j1.len() == j2.len() && j1.keys().all(|k| j2.contains_key(k) 
                && discriminant(unpack_value_option(j1.get(k))) == discriminant(unpack_value_option(j2.get(k)))),
        _ => false
    }
}