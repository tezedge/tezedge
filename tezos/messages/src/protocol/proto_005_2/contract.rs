// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::mem::size_of;
use std::sync::Arc;
use std::collections::HashMap;
use std::string::ToString;

use serde::{Deserialize, Serialize};
use getset::Getters;
use failure::bail;

use crate::base::signature_public_key_hash::SignaturePublicKeyHash;
use crate::base::signature_public_key::SignaturePublicKey;
use crate::protocol::{ToRpcResultJsonMap, UniversalValue, ToRpcResultJsonList};
use crate::{ts_to_rfc3339, is_rfc3339};

use tezos_encoding::{
    encoding::{Encoding, Field, HasEncoding, Tag, TagMap},
    types::BigInt,
};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};

#[derive(Serialize, Debug, Clone)]
pub struct Contract {
    pub balance: BigInt,
    pub delegate: Option<SignaturePublicKeyHash>,
    pub script: Option<Script>,
    pub counter: Option<Counter>,
}

impl Contract {
    pub fn new(
        balance: BigInt,
        delegate: Option<SignaturePublicKeyHash>,
        script: Option<Script>,
        counter: Option<Counter>,
    ) -> Self {
        Self {
            balance,
            delegate,
            script,
            counter,
        }
    }
}

impl ToRpcResultJsonMap for Contract {
    fn as_result_map(&self) -> Result<HashMap<&'static str, UniversalValue>, failure::Error> {
        let mut ret: HashMap<&'static str, UniversalValue> = Default::default();

        ret.insert("balance", UniversalValue::big_num(self.balance.clone()));

        if let Some(s) = &self.script {
            ret.insert("script", UniversalValue::Map(s.as_result_map()?));
        }

        if let Some(c) = &self.counter {
            ret.insert("counter", UniversalValue::big_num(c.counter.clone()));
        }

        if let Some(d) = &self.delegate {
            ret.insert("delegate", UniversalValue::string(d.to_string()));
        }

        Ok(ret)
    }
}

#[derive(Serialize, Debug, Clone, Getters)]
pub struct Script {
    code: Code,
    storage: Storage,
}

impl Script {
    pub fn new(
        code: Code,
        storage: Storage,
    ) -> Self {
        Self {
            code,
            storage,
        }
    }
}

impl ToRpcResultJsonMap for Script {
    fn as_result_map(&self) -> Result<HashMap<&'static str, UniversalValue>, failure::Error> {
        let mut ret: HashMap<&'static str, UniversalValue> = Default::default();

        let mut code_simplified = self.code.code.simplify();
        let mut storage_simplified = self.storage.storage.simplify();

        if let Some(storage_ref) = get_component_mut(&mut code_simplified, &"storage") {
            sort_annots(storage_ref);
        }

        if let Some(parameter_ref) = get_component_mut(&mut code_simplified, &"parameter") {
            // Note: this is a brute force approach to move the annots down one level in the nested JSON
            move_param_annots(parameter_ref);

            sort_annots(parameter_ref);
        }

        if let Some(code_ref) = get_component_mut(&mut code_simplified, &"code") {
            process_code_timestamps(code_ref)?;
        }

        let mut code_list = code_simplified.clone().as_result_list()?;
        
        // after deserialization, the storage and the parameter values "prims" are sometimes swapped in the array (ocaml node behavior)
        if let UniversalValue::List(values) = code_list {
            let mut modified = values.clone();
            if modified.len() > 0 {
                if let UniversalValue::Map(code_map) = &*modified[0] {
                    if code_map.get("prim") != Some(&UniversalValue::String("parameter".to_string())) {
                        modified.swap(0, 1);
                    }
                }
            }
            code_list = UniversalValue::List(modified);
        }
        
        if let Some(type_struct) = get_storage(code_simplified) {
            parse_types(&type_struct, &mut storage_simplified)?;
        }

        ret.insert("code", code_list);
        ret.insert("storage", storage_simplified.as_result_list()?);
        // println!("Serialized storage: {:?}", self.storage.cache_reader().get());
        // println!("Serialized code: {:?}", self.code.cache_reader().get());
        Ok(ret)
    } 
}

fn move_param_annots(code_param: &mut MichelsonJsonElement) {
    if let Some(prim) = &code_param.prim {
        if prim != "parameter" {
            // TODO: handle error
            println!("Only parameter component allowed");
        }
    }
    
    if let Some(args) = &mut code_param.args {
        if let Some(annots) = &mut code_param.annots {
            args[0].annots = Some(annots.to_vec()); 
            code_param.annots = None;
        }
    }
}

fn sort_annots(type_struct: &mut MichelsonJsonElement) {
    if let Some(annots) = &mut type_struct.annots {
        let mut sortable = annots.clone();
        sortable.sort();
        sortable.reverse();
        type_struct.annots = Some(sortable);
    }
    
    if let Some(nested) = &mut type_struct.nested {
        for nest in nested {
            sort_annots(&mut *nest);
        }
    }

    if let Some(args) = &mut type_struct.args {
        for arg in args {
            sort_annots(&mut *arg);
        }
    }
}

fn process_code_timestamps(code_code: &mut MichelsonJsonElement) -> Result<(), failure::Error>{    
    if let Some(prim) = &code_code.prim {
        if prim == "PUSH" {
            if let Some(args) = &mut code_code.args {
                let type_struct = args[0].clone();
                parse_types(&type_struct, &mut args[1])?
            }
        }
    }

    if let Some(nested) = &mut code_code.nested {
        for nest in nested {
            process_code_timestamps(&mut *nest)?
        }
    }

    if let Some(args) = &mut code_code.args {
        for arg in args {
            process_code_timestamps(&mut *arg)?
        }
    }

    Ok(())
}

fn parse_types(type_struct: &MichelsonJsonElement, value_struct: &mut MichelsonJsonElement) -> Result<(), failure::Error> {
    match (type_struct.prim.as_ref(), value_struct.prim.as_ref()) {
        (Some(code_prim), Some(storage_prim)) => {
            if (code_prim.to_lowercase() == storage_prim.to_lowercase()) || (code_prim == "option" && storage_prim == "Some") {
                if let Some(args) = &type_struct.args {
                    if let Some(storage_args) = &mut value_struct.args {
                        let mut counter = 0;
                        for arg in args {
                            parse_types(&arg.clone(), &mut *storage_args[counter])?;
                            counter += 1;
                        }
                    }
                }
                Ok(())
            } else if code_prim == "or" {
                if storage_prim == "Left" {
                    if let Some(args) = &type_struct.args {
                        if let Some(storage_args) = &mut value_struct.args {
                            parse_types(&*args[0], &mut *storage_args[0])?
                        }
                    }
                } else if storage_prim == "Right" {
                    if let Some(args) = &type_struct.args {
                        if let Some(storage_args) = &mut value_struct.args {
                            parse_types(&*args[1], &mut *storage_args[0])?
                        }
                    }
                }
                Ok(())
            } else {
                // println!("Prims do not match!");
                // println!("Code storage prim: {} Storage prim: {}", code_prim, storage_prim);
                Ok(())
            }
        }
        (Some(code_prim), None) => {
            if code_prim == "map" {
                if let Some(storage_nested) = &mut value_struct.nested {
                    if let Some(args) = &type_struct.args {
                        for elem in storage_nested {
                            if let Some(element_args) = &mut elem.args {
                                let mut counter = 0;
                                for arg in args {
                                    parse_types(&arg.clone(), &mut *element_args[counter])?;
                                    counter += 1;
                                }
                            }
                        }
                    }
                }
            }
            if code_prim == "list" || code_prim == "set" {
                if let Some(storage_nested) = &mut value_struct.nested {
                    if let Some(args) = &type_struct.args {
                        for mut elem in storage_nested {
                            parse_types(&*args[0], &mut elem)?
                        }
                    }
                }
            }
            if code_prim == "timestamp" {
                // println!("Storage element: {:?}", value_struct);
                if let Some(num_string) = &value_struct.int {
                    // this unwrap shouldn't panic, as the smart contract validation (at deploying time) should handle overflows
                    // TODO: investigtate, or just handle the Error, just to be safe
                    let num_i64: i64 = num_string.parse()?;
                    value_struct.string = Some(ts_to_rfc3339(num_i64));
                    value_struct.int = None;
                } else if let Some(num_string) = &value_struct.string {
                    // TODO handle errors
                    if !is_rfc3339(num_string) {
                        let num_i64: i64 = num_string.parse()?;
                        value_struct.string = Some(ts_to_rfc3339(num_i64));
                    }
                } else {
                    // TODO handle errors
                    println!("Error in \"timestamp\" ");
                }
            }
            if code_prim == "key_hash" {
                if let Some(bytes) = &value_struct.bytes {
                    let mut padded = bytes.clone();
                    padded.insert(0, 0x00); 

                    let res = SignaturePublicKeyHash::from_tagged_hex_string(&hex::encode(padded))?;
                    value_struct.string = Some(res.to_string());
                    value_struct.bytes = None;
                }
            }
            if code_prim == "address" {
                if let Some(bytes) = &value_struct.bytes {
                    let address_part;
                    let mut address_hook = "".to_string(); 
                    if bytes.len() > 22 {
                        address_part = bytes[..22].to_vec();
                        address_hook = String::from_utf8_lossy(&bytes[22..]).to_string();
                    } else {
                        address_part = bytes.to_vec();
                    }
                    let res = SignaturePublicKeyHash::from_tagged_hex_string(&hex::encode(address_part))?;
                    if address_hook == "" {
                        value_struct.string = Some(res.to_string());
                    } else {
                        value_struct.string = Some(format!("{}%{}", res.to_string(), address_hook));
                    }
                    value_struct.bytes = None;
                }
            }
            if code_prim == "key" {
                if let Some(bytes) = &value_struct.bytes {
                    let res = SignaturePublicKey::from_tagged_bytes(bytes.to_vec())?;
                    value_struct.string = Some(res.to_string());
                    value_struct.bytes = None;
                }
            }
            Ok(())
            // if code_prim == "mutez" {
            //     // println!("Storage part: {:?}", value_struct);

            // }
        }
        _ => bail!("Shouldn't have happened")
    }
}

fn get_storage(code: MichelsonJsonElement) -> Option<MichelsonJsonElement> { 
    if let Some(nested) = code.nested {
        for elem in nested {
            if elem.prim == Some("storage".to_string()) {
                if let Some(args) = elem.args {
                    return Some(*args[0].clone())
                }
            }
        }
    }
    None
}

fn get_component_mut<'a>(code: &'a mut MichelsonJsonElement, component: &str) -> Option<&'a mut MichelsonJsonElement> { 
    if let Some(nested) = &mut code.nested {
        for elem in nested {
            if elem.prim == Some(component.to_string()) {
                return Some(elem)
            }
        }
    }
    None
}

#[derive(Serialize, Deserialize, Debug, Clone, Getters)]
pub struct Code {
    #[get = "pub"]
    code: Box<MichelsonExpression>,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

#[derive(Serialize, Deserialize, Debug, Clone, Getters)]
pub struct Storage {
    // TODO: merge with Code
    #[get = "pub"]
    storage: Box<MichelsonExpression>,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl CachedData for Storage {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

impl HasEncoding for Storage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("storage", Encoding::dynamic(Encoding::Lazy(Arc::new(MichelsonExpression::encoding))))
        ])
    }
}

impl CachedData for Code {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

impl HasEncoding for Code {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("code", Encoding::dynamic(Encoding::Lazy(Arc::new(MichelsonExpression::encoding))))
        ])
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Getters)]
pub struct Counter {
    // TODO: merge with Code
    #[get = "pub"]
    counter: BigInt,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl CachedData for Counter {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

impl HasEncoding for Counter {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("counter", Encoding::Z)
        ])
    }
}


#[derive(Serialize, Deserialize, Debug, Clone, Getters)]
pub struct Balance {
    // TODO: merge with Code
    #[get = "pub"]
    balance: BigInt,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl CachedData for Balance {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

impl HasEncoding for Balance {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("balance", Encoding::Mutez)
        ])
    }
}


#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialEq, Eq,PartialOrd)]
pub struct MichelsonJsonElement {
    int: Option<String>,
    string: Option<String>,
    prim: Option<String>,
    args: Option<Vec<Box<MichelsonJsonElement>>>,
    annots: Option<Vec<String>>,
    bytes: Option<Vec<u8>>,
    nested: Option<Vec<Box<MichelsonJsonElement>>>,
}

pub type RpcJsonMapVector = Vec<HashMap<&'static str, UniversalValue>>;

pub struct RpcMichelsonTuple(Option<MichelsonJsonElement>, Option<Vec<MichelsonJsonElement>>);

impl ToRpcResultJsonMap for MichelsonJsonElement {
    fn as_result_map(&self) -> Result<HashMap<&'static str, UniversalValue>, failure::Error> {
        let mut ret: HashMap<&'static str, UniversalValue> = Default::default();
        
        if let Some(s) = &self.int {
            ret.insert("int", UniversalValue::string(s.clone()));
        }
        if let Some(s) = &self.string {
            ret.insert("string", UniversalValue::string(s.clone()));
        }
        if let Some(s) = &self.prim {
            ret.insert("prim", UniversalValue::string(s.clone()));
        }
        if let Some(s) = &self.args {
            let mut val_list: Vec<Box<UniversalValue>> = Default::default();
            for val in s {
                val_list.push(Box::new(val.as_result_list()?))
            }
            ret.insert("args", UniversalValue::List(val_list));
        }
        if let Some(s) = &self.annots {
            ret.insert("annots", UniversalValue::string_list(s.clone()));
        }

        if let Some(s) = &self.bytes {
            ret.insert("bytes", UniversalValue::string(hex::encode(s)));
        }
        Ok(ret)
    }
}

impl ToRpcResultJsonList for MichelsonJsonElement {
    fn as_result_list(&self) -> Result<UniversalValue, failure::Error> {
        if let Some(s) = &self.nested {
            let mut values: Vec<Box<UniversalValue>> = Default::default();
            for val in s {
                values.push(Box::new(val.as_result_list()?));
            }
            Ok(UniversalValue::List(values))
        } else {
            Ok(UniversalValue::Map(self.as_result_map()?))
        }
    }
}

impl MichelsonJsonElement {
    pub fn new(int: Option<String>, string: Option<String>, prim: Option<String>, args: Option<Vec<Box<MichelsonJsonElement>>>, annots: Option<Vec<String>>, bytes: Option<Vec<u8>>, nested: Option<Vec<Box<MichelsonJsonElement>>>) -> Self {
        Self {
            int,
            string,
            prim,
            args,
            annots,
            bytes,
            nested,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MichelsonExpression {
    Int(MichelsonExpInt),
    String(MichelsonExpString),
    Array(Vec<Box<MichelsonExpression>>),
    Primitive(MichelsonExpPrimitive),
    PrimitiveWithAnotations(Box<MichelsonExpPrimitiveWithAnotations>),
    PrimitiveWithSingleArgument(Box<MichelsonExpPrimitiveWithSingleArgument>),
    PrimitiveWithTwoArguments(Box<MichelsonExpPrimitiveWithTwoArguments>),
    PrimitiveWithSingleArgumentAndAnotations(Box<MichelsonExpPrimitiveWithSingleArgumentAndAnotations>),
    PrimitiveWihtTwoArgumentsAndAnotations(Box<MichelsonExpPrimitiveWithTwoArgumentsAndAnotations>),
    PrimitiveWithNArguments(Box<MichelsonExpPrimitiveWithNArguments>),
    /// holding a tz1 address for example
    Bytes(MichelsonExpBytes),
}

impl MichelsonExpression {
    pub fn simplify(&self) -> MichelsonJsonElement {
        match self {
            Self::Int(int_exp) => MichelsonJsonElement::new(
                Some(int_exp.int.0.to_str_radix(10)), 
                None, 
                None, 
                None,
                None,
                None,
                None, 
            ),
            Self::String(string_exp) => MichelsonJsonElement::new(
                None, 
                Some(string_exp.string.clone()), 
                None, 
                None,
                None,
                None,
                None, 
            ),
            Self::Primitive(prim_exp) => MichelsonJsonElement::new(
                None, 
                None, 
                Some(prim_exp.prim.as_custom_named_variant().to_string()), 
                None,
                None,
                None,
                None, 
            ),
            Self::PrimitiveWithAnotations(exp) => MichelsonJsonElement::new(
                None, 
                None, 
                Some(exp.prim.as_custom_named_variant().to_string()), 
                None,
                Some(exp.annots.split(" ").map(|s| s.to_string()).collect()),
                None,
                None, 
            ),
            Self::PrimitiveWithSingleArgument(exp) => MichelsonJsonElement::new(
                None, 
                None, 
                Some(exp.prim.as_custom_named_variant().to_string()), 
                Some(vec![Box::new(exp.args.simplify())]),
                None,
                None,
                None, 
            ),
            Self::PrimitiveWithSingleArgumentAndAnotations(exp) => MichelsonJsonElement::new(
                None, 
                None, 
                Some(exp.prim.as_custom_named_variant().to_string()), 
                Some(vec![Box::new(exp.args.simplify())]),
                Some(exp.annots.split(" ").map(|s| s.to_string()).collect()),
                None,
                None, 
            ),
            Self::Array(exp) => MichelsonJsonElement::new(
                None, 
                None, 
                None,
                None,
                None,
                None,
                Some(exp.iter().map(|elem| Box::new(elem.simplify())).collect()), 
            ),
            Self::PrimitiveWithTwoArguments(exp) => MichelsonJsonElement::new(
                None, 
                None, 
                Some(exp.prim.as_custom_named_variant().to_string()), 
                Some(exp.args.iter().map(|arg| Box::new(arg.simplify())).collect()), 
                None,
                None,
                None, 
            ),
            Self::PrimitiveWihtTwoArgumentsAndAnotations(exp) => MichelsonJsonElement::new(
                None, 
                None, 
                Some(exp.prim.as_custom_named_variant().to_string()), 
                Some(exp.args.iter().map(|arg| Box::new(arg.simplify())).collect()), 
                Some(exp.annots.split(" ").map(|s| s.to_string()).collect()),
                None,
                None, 
            ),
            Self::PrimitiveWithNArguments(exp) => MichelsonJsonElement::new(
                None, 
                None, 
                Some(exp.prim.as_custom_named_variant().to_string()), 
                Some(exp.args.iter().map(|arg| Box::new(arg.simplify())).collect()), 
                None,
                None,
                None, 
            ),
            Self::Bytes(exp) => MichelsonJsonElement::new(
                None, 
                None, 
                None, 
                None, 
                None,
                Some(exp.bytes.clone()),
                None,
            )
        }
    }
}


impl HasEncoding for MichelsonExpression {
    fn encoding() -> Encoding {
        Encoding::Tags(
            size_of::<u8>(),
            TagMap::new(&[
                Tag::new(0x00, "Int", MichelsonExpInt::encoding()),
                Tag::new(0x01, "String", MichelsonExpString::encoding()),
                Tag::new(0x02, "Array", Encoding::dynamic(Encoding::list(Encoding::Lazy(Arc::new(MichelsonExpression::encoding))))),
                Tag::new(0x03, "Primitive", MichelsonExpPrimitive::encoding()),
                Tag::new(0x04, "PrimitiveWithAnotations", MichelsonExpPrimitiveWithAnotations::encoding()),
                Tag::new(0x05, "PrimitiveWithSingleArgument", MichelsonExpPrimitiveWithSingleArgument::encoding()),
                Tag::new(0x06, "PrimitiveWithSingleArgumentAndAnotations", MichelsonExpPrimitiveWithSingleArgumentAndAnotations::encoding()),
                Tag::new(0x07, "PrimitiveWithTwoArguments", MichelsonExpPrimitiveWithTwoArguments::encoding()),
                Tag::new(0x08, "PrimitiveWihtTwoArgumentsAndAnotations",  MichelsonExpPrimitiveWithTwoArgumentsAndAnotations::encoding()),
                Tag::new(0x09, "PrimitiveWithNArguments", MichelsonExpPrimitiveWithNArguments::encoding()),
                Tag::new(0x0a, "Bytes", MichelsonExpBytes::encoding()),
            ])
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpInt {
    int: BigInt,
}

impl HasEncoding for MichelsonExpInt {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("int", Encoding::Z),
        ])
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpString {
    string: String,
}

impl HasEncoding for MichelsonExpString {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("string", Encoding::String),
        ])
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpPrimitive {
    prim: MichelsonPrimitive,
}

impl HasEncoding for MichelsonExpPrimitive {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("prim", Encoding::Tags(size_of::<u8>(), michelson_primitive_tag_map())),
        ])
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpPrimitiveWithAnotations {
    prim: MichelsonPrimitive,
    // TODO transform to array
    annots: String,
}

impl HasEncoding for MichelsonExpPrimitiveWithAnotations {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("prim", Encoding::Tags(size_of::<u8>(), michelson_primitive_tag_map())),
            Field::new("annots", Encoding::String),
        ])
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpPrimitiveWithSingleArgument {
    prim: MichelsonPrimitive,
    // TODO transform to array with this only element
    args: Box<MichelsonExpression>,
}

impl HasEncoding for MichelsonExpPrimitiveWithSingleArgument {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("prim", Encoding::Tags(size_of::<u8>(), michelson_primitive_tag_map())),
            Field::new("args", Encoding::Lazy(Arc::new(MichelsonExpression::encoding))),
        ])
    }
}

// MichelsonExpPrimitiveWithTwoArguments
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpPrimitiveWithTwoArguments {
    prim: MichelsonPrimitive,
    args: Vec<Box<MichelsonExpression>>,
}

impl HasEncoding for MichelsonExpPrimitiveWithTwoArguments {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("prim", Encoding::Tags(size_of::<u8>(), michelson_primitive_tag_map())),
            Field::new("args", Encoding::array(2, Encoding::Lazy(Arc::new(MichelsonExpression::encoding)))),
        ])
    }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpPrimitiveWithSingleArgumentAndAnotations {
    prim: MichelsonPrimitive,
    args: Box<MichelsonExpression>,
    annots: String,
}

impl HasEncoding for MichelsonExpPrimitiveWithSingleArgumentAndAnotations {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("prim", Encoding::Tags(size_of::<u8>(), michelson_primitive_tag_map())),
            Field::new("args", Encoding::Lazy(Arc::new(MichelsonExpression::encoding))),
            Field::new("annots", Encoding::String),
        ])
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpPrimitiveWithTwoArgumentsAndAnotations {
    prim: MichelsonPrimitive,
    args: Vec<Box<MichelsonExpression>>,
    // TODO transform to array
    annots: String,
}

impl HasEncoding for MichelsonExpPrimitiveWithTwoArgumentsAndAnotations {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("prim", Encoding::Tags(size_of::<u8>(), michelson_primitive_tag_map())),
            Field::new("args", Encoding::array(2, Encoding::Lazy(Arc::new(MichelsonExpression::encoding)))),
            Field::new("annots", Encoding::String),
        ])
    }
}

// MichelsonExpPrimitiveWithNArguments

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpPrimitiveWithNArguments {
    prim: MichelsonPrimitive,
    // placeholder: Vec<u8>,
    args: Vec<Box<MichelsonExpression>>,
    annots: String,
}

impl HasEncoding for MichelsonExpPrimitiveWithNArguments {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("prim", Encoding::Tags(size_of::<u8>(), michelson_primitive_tag_map())),
            // Field::new("placeholder", Encoding::sized(2, Encoding::Bytes)),
            Field::new("args", Encoding::dynamic(Encoding::list(Encoding::Lazy(Arc::new(MichelsonExpression::encoding))))),
            Field::new("annots", Encoding::String),
        ])
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpBytes {
    bytes: Vec<u8>,
}

impl HasEncoding for MichelsonExpBytes {
    fn encoding() -> Encoding {
        // Encoding::dynamic(Encoding::Bytes)
        Encoding::Obj(vec![
            Field::new("bytes", Encoding::dynamic(Encoding::Bytes)),
        ])
    }
}

pub fn michelson_primitive_tag_map() -> TagMap {
    let primitive_vec = vec![
        MichelsonPrimitive::parameter,
        MichelsonPrimitive::storage,
        MichelsonPrimitive::code,
        MichelsonPrimitive::False,
        MichelsonPrimitive::Elt,
        MichelsonPrimitive::Left,
        MichelsonPrimitive::None,
        MichelsonPrimitive::Pair,
        MichelsonPrimitive::Right,
        MichelsonPrimitive::Some,
        MichelsonPrimitive::True,
        MichelsonPrimitive::Unit,
        MichelsonPrimitive::PACK,
        MichelsonPrimitive::UNPACK,
        MichelsonPrimitive::BLAKE2B,
        MichelsonPrimitive::SHA256,
        MichelsonPrimitive::SHA512,
        MichelsonPrimitive::ABS,
        MichelsonPrimitive::ADD,
        MichelsonPrimitive::AMOUNT,
        MichelsonPrimitive::AND,
        MichelsonPrimitive::BALANCE,
        MichelsonPrimitive::CAR,
        MichelsonPrimitive::CDR,
        MichelsonPrimitive::CHECK_SIGNATURE,
        MichelsonPrimitive::COMPARE,
        MichelsonPrimitive::CONCAT,
        MichelsonPrimitive::CONS,
        MichelsonPrimitive::CREATE_ACCOUNT,
        MichelsonPrimitive::CREATE_CONTRACT,
        MichelsonPrimitive::IMPLICIT_ACCOUNT,
        MichelsonPrimitive::DIP,
        MichelsonPrimitive::DROP,
        MichelsonPrimitive::DUP,
        MichelsonPrimitive::EDIV,
        MichelsonPrimitive::EMPTY_MAP,
        MichelsonPrimitive::EMPTY_SET,
        MichelsonPrimitive::EQ,
        MichelsonPrimitive::EXEC,
        MichelsonPrimitive::FAILWITH,
        MichelsonPrimitive::GE,
        MichelsonPrimitive::GET,
        MichelsonPrimitive::GT,
        MichelsonPrimitive::HASH_KEY,
        MichelsonPrimitive::IF,
        MichelsonPrimitive::IF_CONS,
        MichelsonPrimitive::IF_LEFT,
        MichelsonPrimitive::IF_NONE,
        MichelsonPrimitive::INT,
        MichelsonPrimitive::LAMBDA,
        MichelsonPrimitive::LE,
        MichelsonPrimitive::LEFT,
        MichelsonPrimitive::LOOP,
        MichelsonPrimitive::LSL,
        MichelsonPrimitive::LSR,
        MichelsonPrimitive::LT,
        MichelsonPrimitive::MAP,
        MichelsonPrimitive::MEM,
        MichelsonPrimitive::MUL,
        MichelsonPrimitive::NEG,
        MichelsonPrimitive::NEQ,
        MichelsonPrimitive::NIL,
        MichelsonPrimitive::NONE,
        MichelsonPrimitive::NOT,
        MichelsonPrimitive::NOW,
        MichelsonPrimitive::OR,
        MichelsonPrimitive::PAIR,
        MichelsonPrimitive::PUSH,
        MichelsonPrimitive::RIGHT,
        MichelsonPrimitive::SIZE,
        MichelsonPrimitive::SOME,
        MichelsonPrimitive::SOURCE,
        MichelsonPrimitive::SENDER,
        MichelsonPrimitive::SELF,
        MichelsonPrimitive::STEPS_TO_QUOTA,
        MichelsonPrimitive::SUB,
        MichelsonPrimitive::SWAP,
        MichelsonPrimitive::TRANSFER_TOKENS,
        MichelsonPrimitive::SET_DELEGATE,
        MichelsonPrimitive::UNIT,
        MichelsonPrimitive::UPDATE,
        MichelsonPrimitive::XOR,
        MichelsonPrimitive::ITER,
        MichelsonPrimitive::LOOP_LEFT,
        MichelsonPrimitive::ADDRESS,
        MichelsonPrimitive::CONTRACT,
        MichelsonPrimitive::ISNAT,
        MichelsonPrimitive::CAST,
        MichelsonPrimitive::RENAME,
        MichelsonPrimitive::bool,
        MichelsonPrimitive::contract,
        MichelsonPrimitive::int,
        MichelsonPrimitive::key,
        MichelsonPrimitive::key_hash,
        MichelsonPrimitive::lambda,
        MichelsonPrimitive::list,
        MichelsonPrimitive::map,
        MichelsonPrimitive::big_map,
        MichelsonPrimitive::nat,
        MichelsonPrimitive::option,
        MichelsonPrimitive::or,
        MichelsonPrimitive::pair,
        MichelsonPrimitive::set,
        MichelsonPrimitive::signature,
        MichelsonPrimitive::string,
        MichelsonPrimitive::bytes,
        MichelsonPrimitive::mutez,
        MichelsonPrimitive::timestamp,
        MichelsonPrimitive::unit,
        MichelsonPrimitive::operation,
        MichelsonPrimitive::address,
        MichelsonPrimitive::SLICE,
        MichelsonPrimitive::DIG,
        MichelsonPrimitive::DUG,
        MichelsonPrimitive::EMPTY_BIG_MAP,
        MichelsonPrimitive::APPLY,
        MichelsonPrimitive::chain_id,
        MichelsonPrimitive::CHAIN_ID,
        ];
        
        // let mut primitive_tags: TagMap = Default::default();
        // let mut tag_hash_map: HashMap<u16, &'static str> = Default::default();
        let mut tag_vec: Vec<Tag> = Default::default();

        let mut counter: u16 = 0;
        for element in primitive_vec {
            tag_vec.push(Tag::new(counter, element.as_custom_named_variant(), Encoding::Unit));
            counter += 1;
        }
        TagMap::new(&tag_vec)
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum MichelsonPrimitive{
    parameter,
    storage,
    code,
    False,
    Elt,
    Left,
    None,
    Pair,
    Right,
    Some,
    True,
    Unit,
    PACK,
    UNPACK,
    BLAKE2B,
    SHA256,
    SHA512,
    ABS,
    ADD,
    AMOUNT,
    AND,
    BALANCE,
    CAR,
    CDR,
    CHECK_SIGNATURE,
    COMPARE,
    CONCAT,
    CONS,
    CREATE_ACCOUNT,
    CREATE_CONTRACT,
    IMPLICIT_ACCOUNT,
    DIP,
    DROP,
    DUP,
    EDIV,
    EMPTY_MAP,
    EMPTY_SET,
    EQ,
    EXEC,
    FAILWITH,
    GE,
    GET,
    GT,
    HASH_KEY,
    IF,
    IF_CONS,
    IF_LEFT,
    IF_NONE,
    INT,
    LAMBDA,
    LE,
    LEFT,
    LOOP,
    LSL,
    LSR,
    LT,
    MAP,
    MEM,
    MUL,
    NEG,
    NEQ,
    NIL,
    NONE,
    NOT,
    NOW,
    OR,
    PAIR,
    PUSH,
    RIGHT,
    SIZE,
    SOME,
    SOURCE,
    SENDER,
    SELF,
    STEPS_TO_QUOTA,
    SUB,
    SWAP,
    TRANSFER_TOKENS,
    SET_DELEGATE,
    UNIT,
    UPDATE,
    XOR,
    ITER,
    LOOP_LEFT,
    ADDRESS,
    CONTRACT,
    ISNAT,
    CAST,
    RENAME,
    bool,
    contract,
    int,
    key,
    key_hash,
    lambda,
    list,
    map,
    big_map,
    nat,
    option,
    or,
    pair,
    set,
    signature,
    string,
    bytes,
    mutez,
    timestamp,
    unit,
    operation,
    address,
    SLICE,
    DIG,
    DUG,
    EMPTY_BIG_MAP,
    APPLY,
    chain_id,
    CHAIN_ID,
}

impl MichelsonPrimitive {
    pub fn as_custom_named_variant(&self) -> &'static str {
        match self {
            MichelsonPrimitive::parameter => "parameter",
            MichelsonPrimitive::storage => "storage",
            MichelsonPrimitive::code => "code",
            MichelsonPrimitive::False => "False",
            MichelsonPrimitive::Elt => "Elt",
            MichelsonPrimitive::Left => "Left",
            MichelsonPrimitive::None => "None",
            MichelsonPrimitive::Pair => "Pair",
            MichelsonPrimitive::Right => "Right",
            MichelsonPrimitive::Some => "Some",
            MichelsonPrimitive::True => "True",
            MichelsonPrimitive::Unit => "Unit",
            MichelsonPrimitive::PACK => "PACK",
            MichelsonPrimitive::UNPACK => "UNPACK",
            MichelsonPrimitive::BLAKE2B => "BLAKE2B",
            MichelsonPrimitive::SHA256 => "SHA256",
            MichelsonPrimitive::SHA512 => "SHA512",
            MichelsonPrimitive::ABS => "ABS",
            MichelsonPrimitive::ADD => "ADD",
            MichelsonPrimitive::AMOUNT => "AMOUNT",
            MichelsonPrimitive::AND => "AND",
            MichelsonPrimitive::BALANCE => "BALANCE",
            MichelsonPrimitive::CAR => "CAR",
            MichelsonPrimitive::CDR => "CDR",
            MichelsonPrimitive::CHAIN_ID => "CHAIN_ID",
            MichelsonPrimitive::CHECK_SIGNATURE => "CHECK_SIGNATURE",
            MichelsonPrimitive::COMPARE => "COMPARE",
            MichelsonPrimitive::CONCAT => "CONCAT",
            MichelsonPrimitive::CONS => "CONS",
            MichelsonPrimitive::CREATE_ACCOUNT => "CREATE_ACCOUNT",
            MichelsonPrimitive::CREATE_CONTRACT => "CREATE_CONTRACT",
            MichelsonPrimitive::IMPLICIT_ACCOUNT => "IMPLICIT_ACCOUNT",
            MichelsonPrimitive::DIP => "DIP",
            MichelsonPrimitive::DROP => "DROP",
            MichelsonPrimitive::DUP => "DUP",
            MichelsonPrimitive::EDIV => "EDIV",
            MichelsonPrimitive::EMPTY_BIG_MAP => "EMPTY_BIG_MAP",
            MichelsonPrimitive::EMPTY_MAP => "EMPTY_MAP",
            MichelsonPrimitive::EMPTY_SET => "EMPTY_SET",
            MichelsonPrimitive::EQ => "EQ",
            MichelsonPrimitive::EXEC => "EXEC",
            MichelsonPrimitive::APPLY => "APPLY",
            MichelsonPrimitive::FAILWITH => "FAILWITH",
            MichelsonPrimitive::GE => "GE",
            MichelsonPrimitive::GET => "GET",
            MichelsonPrimitive::GT => "GT",
            MichelsonPrimitive::HASH_KEY => "HASH_KEY",
            MichelsonPrimitive::IF => "IF",
            MichelsonPrimitive::IF_CONS => "IF_CONS",
            MichelsonPrimitive::IF_LEFT => "IF_LEFT",
            MichelsonPrimitive::IF_NONE => "IF_NONE",
            MichelsonPrimitive::INT => "INT",
            MichelsonPrimitive::LAMBDA => "LAMBDA",
            MichelsonPrimitive::LE => "LE",
            MichelsonPrimitive::LEFT => "LEFT",
            MichelsonPrimitive::LOOP => "LOOP",
            MichelsonPrimitive::LSL => "LSL",
            MichelsonPrimitive::LSR => "LSR",
            MichelsonPrimitive::LT => "LT",
            MichelsonPrimitive::MAP => "MAP",
            MichelsonPrimitive::MEM => "MEM",
            MichelsonPrimitive::MUL => "MUL",
            MichelsonPrimitive::NEG => "NEG",
            MichelsonPrimitive::NEQ => "NEQ",
            MichelsonPrimitive::NIL => "NIL",
            MichelsonPrimitive::NONE => "NONE",
            MichelsonPrimitive::NOT => "NOT",
            MichelsonPrimitive::NOW => "NOW",
            MichelsonPrimitive::OR => "OR",
            MichelsonPrimitive::PAIR => "PAIR",
            MichelsonPrimitive::PUSH => "PUSH",
            MichelsonPrimitive::RIGHT => "RIGHT",
            MichelsonPrimitive::SIZE => "SIZE",
            MichelsonPrimitive::SOME => "SOME",
            MichelsonPrimitive::SOURCE => "SOURCE",
            MichelsonPrimitive::SENDER => "SENDER",
            MichelsonPrimitive::SELF => "SELF",
            MichelsonPrimitive::SLICE => "SLICE",
            MichelsonPrimitive::STEPS_TO_QUOTA => "STEPS_TO_QUOTA",
            MichelsonPrimitive::SUB => "SUB",
            MichelsonPrimitive::SWAP => "SWAP",
            MichelsonPrimitive::TRANSFER_TOKENS => "TRANSFER_TOKENS",
            MichelsonPrimitive::SET_DELEGATE => "SET_DELEGATE",
            MichelsonPrimitive::UNIT => "UNIT",
            MichelsonPrimitive::UPDATE => "UPDATE",
            MichelsonPrimitive::XOR => "XOR",
            MichelsonPrimitive::ITER => "ITER",
            MichelsonPrimitive::LOOP_LEFT => "LOOP_LEFT",
            MichelsonPrimitive::ADDRESS => "ADDRESS",
            MichelsonPrimitive::CONTRACT => "CONTRACT",
            MichelsonPrimitive::ISNAT => "ISNAT",
            MichelsonPrimitive::CAST => "CAST",
            MichelsonPrimitive::RENAME => "RENAME",
            MichelsonPrimitive::DIG => "DIG",
            MichelsonPrimitive::DUG => "DUG",
            MichelsonPrimitive::bool => "bool",
            MichelsonPrimitive::contract => "contract",
            MichelsonPrimitive::int => "int",
            MichelsonPrimitive::key => "key",
            MichelsonPrimitive::key_hash => "key_hash",
            MichelsonPrimitive::lambda => "lambda",
            MichelsonPrimitive::list => "list",
            MichelsonPrimitive::map => "map",
            MichelsonPrimitive::big_map => "big_map",
            MichelsonPrimitive::nat => "nat",
            MichelsonPrimitive::option => "option",
            MichelsonPrimitive::or => "or",
            MichelsonPrimitive::pair => "pair",
            MichelsonPrimitive::set => "set",
            MichelsonPrimitive::signature => "signature",
            MichelsonPrimitive::string => "string",
            MichelsonPrimitive::bytes => "bytes",
            MichelsonPrimitive::mutez => "mutez",
            MichelsonPrimitive::timestamp => "timestamp",
            MichelsonPrimitive::unit => "unit",
            MichelsonPrimitive::operation => "operation",
            MichelsonPrimitive::address => "address",
            MichelsonPrimitive::chain_id => "chain_id",
        }
    }
}