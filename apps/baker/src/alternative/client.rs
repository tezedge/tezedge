// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{io, str, sync::mpsc, thread, time::Duration};

use chrono::{DateTime, ParseError, Utc};
use derive_more::From;
use reqwest::{blocking::Client, StatusCode, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

use crypto::hash::{
    BlockHash, BlockPayloadHash, ChainId, ContextHash, OperationHash, OperationListListHash,
    ProtocolHash, Signature,
};
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_messages::{
    p2p::{binary_message::BinaryRead, encoding::operation::DecodedOperation},
    protocol::proto_012::operation::FullHeader,
};

use super::event::{Block, Constants, Event, OperationSimple, ProtocolBlockHeader};

pub const PROTOCOL: &'static str = "Psithaca2MLRFYargivpo7YvUr7wUDqyxrdhC5CQq78mRvimz6A";

#[derive(Clone)]
pub struct RpcClient {
    tx: mpsc::Sender<Result<Event, RpcError>>,
    endpoint: Url,
    inner: Client,
}

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("{inner}, url: {url}, body: {body}")]
    WithBody {
        url: Url,
        body: String,
        inner: RpcErrorInner,
    },
    #[error("{inner}, url: {url}")]
    WithContext { url: Url, inner: RpcErrorInner },
    #[error("{_0}")]
    Less(RpcErrorInner),
}

#[derive(Debug, Error, From)]
pub enum RpcErrorInner {
    #[error("reqwest: {_0}")]
    Reqwest(reqwest::Error),
    #[error("serde_json: {_0}")]
    SerdeJson(serde_json::Error),
    #[error("io: {_0}")]
    Io(io::Error),
    #[error("hex: {_0}")]
    Hex(hex::FromHexError),
    #[error("nom: {_0}")]
    Nom(BinaryReaderError),
    #[error("utf8: {_0}")]
    Utf8(str::Utf8Error),
    #[error("chrono: {_0}")]
    Chrono(ParseError),
    #[error("node: {_0}")]
    NodeError(String, StatusCode),
}

impl RpcClient {
    pub fn new(endpoint: Url, tx: mpsc::Sender<Result<Event, RpcError>>) -> Self {
        RpcClient {
            tx,
            endpoint,
            inner: Client::new(),
        }
    }

    pub fn get_chain_id(&self) -> Result<ChainId, RpcError> {
        let url = self
            .endpoint
            .join("chains/main/chain_id")
            .expect("valid constant url");
        self.single_response_blocking(&url, None, None)
            .map_err(|inner| RpcError::WithContext { url, inner })
    }

    /// nothing to do until bootstrapped, so let's wait synchronously
    pub fn wait_bootstrapped(&self) -> Result<BlockHash, RpcError> {
        let url = self
            .endpoint
            .join("monitor/bootstrapped")
            .expect("valid constant url");

        #[derive(Deserialize)]
        struct Bootstrapped {
            block: BlockHash,
            #[allow(dead_code)]
            timestamp: String,
        }
        let Bootstrapped { block, .. } = self
            .single_response_blocking(&url, None, None)
            .map_err(|inner| RpcError::WithContext { url, inner })?;

        Ok(block)
    }

    pub fn get_block(&self, block_hash: &BlockHash) -> Result<Option<Block>, RpcError> {
        #[derive(Deserialize)]
        struct Protocols {
            protocol: ProtocolHash,
            next_protocol: ProtocolHash,
        }

        let s = format!("chains/main/blocks/{}/protocols", block_hash);
        let url = self.endpoint.join(&s).expect("valid url");
        let Protocols {
            protocol,
            next_protocol,
        } = self
            .single_response_blocking(&url, None, None)
            .map_err(|inner| RpcError::WithContext { url, inner })?;
        if next_protocol.to_base58_check() != PROTOCOL {
            return Ok(None);
        }

        #[derive(Deserialize)]
        struct BlockHeaderJson {
            hash: BlockHash,
            level: i32,
            proto: u8,
            predecessor: BlockHash,
            timestamp: String,
            validation_pass: u8,
            operations_hash: OperationListListHash,
            fitness: Vec<String>,
            context: ContextHash,
            payload_hash: BlockPayloadHash,
            payload_round: i32,
        }

        let s = format!("chains/main/blocks/{}/header", block_hash);
        let url = self.endpoint.join(&s).expect("valid url");
        let header = self
            .single_response_blocking::<BlockHeaderJson>(&url, None, None)
            .map_err(|inner| RpcError::WithContext { url, inner })?;

        let transition = protocol != next_protocol;
        let operations = if !transition {
            let s = format!("chains/main/blocks/{}/operations", header.hash);
            let url = self.endpoint.join(&s).expect("valid url");
            self.single_response_blocking(&url, None, None)
                .map_err(|inner| RpcError::WithContext { url, inner })?
        } else {
            vec![]
        };

        let s = format!("chains/main/blocks/head/helpers/validators");
        let mut url = self.endpoint.join(&s).expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("level", &(header.level + 1).to_string());
        let validators = self
            .single_response_blocking(&url, None, None)
            .map_err(|inner| RpcError::WithContext { url, inner })?;

        Ok(Some(Block {
            hash: header.hash,
            level: header.level,
            proto: header.proto,
            predecessor: header.predecessor,
            timestamp: header.timestamp,
            validation_pass: header.validation_pass,
            operations_hash: header.operations_hash,
            fitness: header.fitness,
            context: header.context,
            payload_hash: header.payload_hash,
            payload_round: header.payload_round,

            transition,
            operations,
            validators,
        }))
    }

    pub fn get_constants(&self) -> Result<Constants, RpcError> {
        let url = self
            .endpoint
            .join("chains/main/blocks/head/context/constants")
            .expect("valid constant url");
        self.single_response_blocking(&url, None, None)
            .map_err(|inner| RpcError::WithContext { url, inner })
    }

    pub fn monitor_heads(&self, chain_id: &ChainId) -> Result<(), RpcError> {
        let s = format!("monitor/heads/{chain_id}");
        let mut url = self.endpoint.join(&s).expect("valid constant url");
        url.query_pairs_mut().append_pair("next_protocol", PROTOCOL);

        #[derive(Deserialize, Serialize, Debug, Clone)]
        struct BlockHeaderJsonGeneric {
            hash: BlockHash,
            level: i32,
            proto: u8,
            predecessor: BlockHash,
            timestamp: String,
            validation_pass: u8,
            operations_hash: OperationListListHash,
            fitness: Vec<String>,
            context: ContextHash,
            protocol_data: String,
        }

        let this = self.clone();
        self.multiple_responses::<BlockHeaderJsonGeneric, _>(&url, None, move |header| {
            let s = format!("chains/main/blocks/{}/operations", header.hash);
            let url = this.endpoint.join(&s).expect("valid url");
            let operations = this.single_response_blocking(&url, None, None)?;
            let s = format!("chains/main/blocks/head/helpers/validators");
            let mut url = this.endpoint.join(&s).expect("valid constant url");
            url.query_pairs_mut()
                .append_pair("level", &(header.level + 1).to_string());
            let validators = this.single_response_blocking(&url, None, None)?;

            let protocol_data_bytes = hex::decode(header.protocol_data)?;
            let protocol_header = ProtocolBlockHeader::from_bytes(&protocol_data_bytes)?;

            Ok(Event::Block(Block {
                hash: header.hash,
                level: header.level,
                proto: header.proto,
                predecessor: header.predecessor,
                timestamp: header.timestamp,
                validation_pass: header.validation_pass,
                operations_hash: header.operations_hash,
                fitness: header.fitness,
                context: header.context,
                payload_hash: protocol_header.payload_hash,
                payload_round: protocol_header.payload_round,

                transition: false,
                operations,
                validators,
            }))
        })
        .map_err(|inner| RpcError::WithContext { url, inner })
    }

    pub fn monitor_operations(&self) -> Result<(), RpcError> {
        let mut url = self
            .endpoint
            .join("chains/main/mempool/monitor_operations")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("applied", "yes")
            .append_pair("refused", "no")
            .append_pair("outdated", "no")
            .append_pair("branch_refused", "no")
            .append_pair("branch_delayed", "yes");
        self.multiple_responses(&url, None, move |operations| {
            Ok(Event::Operations(operations))
        })
        .map_err(|inner| RpcError::WithContext { url, inner })
    }

    pub fn inject_operation(
        &self,
        chain_id: &ChainId,
        op_hex: String,
    ) -> Result<OperationHash, RpcError> {
        let mut url = self
            .endpoint
            .join("injection/operation")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("chain", &chain_id.to_base58_check());
        let body = format!("{op_hex:?}");
        self.single_response_blocking::<OperationHash>(&url, Some(body.clone()), None)
            .map_err(|inner| RpcError::WithBody { url, body, inner })
    }

    pub fn preapply_block(
        &self,
        protocol_header: ProtocolBlockHeader,
        predecessor_hash: BlockHash,
        timestamp: i64,
        mut operations: [Vec<OperationSimple>; 4],
    ) -> Result<(FullHeader, Vec<Vec<DecodedOperation>>), RpcError> {
        #[derive(Serialize)]
        struct BlockData {
            protocol_data: serde_json::Value,
            operations: [Vec<OperationSimple>; 4],
        }

        #[derive(Deserialize)]
        pub struct ShellBlockShortHeader {
            level: i32,
            proto: u8,
            predecessor: BlockHash,
            timestamp: String,
            validation_pass: u8,
            operations_hash: OperationListListHash,
            fitness: Vec<String>,
            context: ContextHash,
        }

        #[derive(Deserialize)]
        struct PreapplyResponse {
            shell_header: ShellBlockShortHeader,
            operations: Vec<serde_json::Value>,
        }

        let mut protocol_data = serde_json::to_value(&protocol_header)
            .map_err(Into::into)
            .map_err(RpcError::Less)?;
        let protocol_block_header_obj = protocol_data
            .as_object_mut()
            .expect("`ProtocolBlockHeader` is a structure");
        let proof_of_work_str = hex::encode(&protocol_header.proof_of_work_nonce);
        protocol_block_header_obj.insert(
            "proof_of_work_nonce".to_string(),
            serde_json::Value::String(proof_of_work_str),
        );
        protocol_block_header_obj.insert(
            "protocol".to_string(),
            serde_json::Value::String(PROTOCOL.to_string()),
        );

        for i in 0..4 {
            for op in &mut operations[i] {
                op.hash = None;
                for content in &mut op.contents {
                    if let Some(content_obj) = content.as_object_mut() {
                        content_obj.remove("metadata");
                    }
                }
            }
        }
        let block_data = BlockData {
            protocol_data,
            operations,
        };

        let s = format!("chains/main/blocks/{predecessor_hash}/helpers/preapply/block");
        let mut url = self.endpoint.join(&s).expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("timestamp", &timestamp.to_string());
        let body = serde_json::to_string(&block_data)
            .map_err(Into::into)
            .map_err(RpcError::Less)?;
        let PreapplyResponse {
            shell_header,
            operations,
        } = self
            .single_response_blocking(&url, Some(body.clone()), None)
            .map_err(|inner| RpcError::WithBody {
                url: url.clone(),
                body,
                inner,
            })?;

        let ShellBlockShortHeader {
            level,
            proto,
            predecessor,
            timestamp,
            validation_pass,
            operations_hash,
            fitness,
            context,
        } = shell_header;
        let timestamp = timestamp
            .parse::<DateTime<Utc>>()
            .map_err(Into::into)
            .map_err(|inner| RpcError::WithContext {
                url: url.clone(),
                inner,
            })?
            .timestamp()
            .into();
        let ProtocolBlockHeader {
            payload_hash,
            payload_round,
            proof_of_work_nonce,
            seed_nonce_hash,
            liquidity_baking_escape_vote,
            ..
        } = protocol_header;
        let full_block_header = FullHeader {
            level,
            proto,
            predecessor,
            timestamp,
            validation_pass,
            operations_hash,
            fitness: {
                let mut v = vec![];
                for fitness_str in fitness {
                    let url = url.clone();
                    let item = hex::decode(fitness_str)
                        .map_err(Into::into)
                        .map_err(|inner| RpcError::WithContext { url, inner })?;
                    v.push(item);
                }
                v.into()
            },
            context,
            payload_hash,
            payload_round,
            proof_of_work_nonce,
            seed_nonce_hash,
            liquidity_baking_escape_vote,
            signature: Signature(vec![0; 64]),
        };
        let operations = operations
            .into_iter()
            .filter_map(|mut v| {
                let applied = v.as_object_mut()?.remove("applied")?;
                serde_json::from_value(applied).ok()
            })
            .collect();

        Ok((full_block_header, operations))
    }

    pub fn inject_block(
        &self,
        data: String,
        operations: Vec<Vec<DecodedOperation>>,
    ) -> Result<BlockHash, RpcError> {
        #[derive(Serialize)]
        struct BlockData {
            data: String,
            operations: Vec<Vec<DecodedOperation>>,
        }
        let block_data = BlockData { data, operations };
        let url = self
            .endpoint
            .join("injection/block")
            .expect("valid constant url");
        let body = serde_json::to_string(&block_data)
            .map_err(Into::into)
            .map_err(RpcError::Less)?;
        self.single_response_blocking(&url, Some(body), None)
            .map_err(|inner| RpcError::WithContext { url, inner })
    }

    fn multiple_responses<T, F>(
        &self,
        url: &Url,
        timeout: Option<Duration>,
        wrapper: F,
    ) -> Result<(), RpcErrorInner>
    where
        T: DeserializeOwned,
        F: Fn(T) -> Result<Event, RpcErrorInner> + Send + 'static,
    {
        let request = self.inner.get(url.clone());
        let request = if let Some(timeout) = timeout {
            request.timeout(timeout)
        } else {
            request
        };

        let url = url.clone();
        let response = request.send()?;
        let tx = self.tx.clone();
        let handle = thread::spawn(move || {
            let status = response.status();

            if status.is_success() {
                let mut deserializer =
                    serde_json::Deserializer::from_reader(response).into_iter::<T>();
                while let Some(v) = deserializer.next() {
                    let url = url.clone();
                    let v = v
                        .map_err(Into::into)
                        .and_then(|v| wrapper(v))
                        .map_err(|inner| RpcError::WithContext { url, inner });
                    let _ = tx.send(v);
                }
            } else {
                let status = response.status();
                let mut response = response;
                match Self::read_error(&mut response, status) {
                    Ok(()) => unreachable!(),
                    Err(inner) => {
                        let _ = tx.send(Err(RpcError::WithContext { url, inner }));
                    }
                }
            }
        });

        // TODO: join
        let _ = handle;

        Ok(())
    }

    fn single_response_blocking<T>(
        &self,
        url: &Url,
        body: Option<String>,
        timeout: Option<Duration>,
    ) -> Result<T, RpcErrorInner>
    where
        T: DeserializeOwned,
    {
        let mut retry = 3;
        let mut response = loop {
            let request = match &body {
                Some(ref body) => self.inner.post(url.clone()).body(body.clone()),
                None => self.inner.get(url.clone()),
            };
            let request = if let Some(timeout) = timeout {
                request.timeout(timeout)
            } else {
                request
            };
            match request.send() {
                Ok(v) => break v,
                Err(err) => {
                    if retry == 0 {
                        return Err(err.into());
                    } else {
                        retry -= 1;
                    }
                }
            }
        };
        if response.status().is_success() {
            serde_json::from_reader::<_, T>(response).map_err(Into::into)
        } else {
            let status = response.status();
            Self::read_error(&mut response, status)?;
            unreachable!()
        }
    }

    // it may be string without quotes, it is invalid json, let's read it manually
    fn read_error(response: &mut impl io::Read, status: StatusCode) -> Result<(), RpcErrorInner> {
        let mut buf = [0; 0x1000];
        io::Read::read(response, &mut buf)?;
        let err = str::from_utf8(&buf)?.trim_end_matches('\0');
        Err(RpcErrorInner::NodeError(err.to_string(), status))
    }
}
