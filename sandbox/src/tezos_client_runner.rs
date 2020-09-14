use std::collections::HashMap;
use std::fs;
use std::io::{Write, BufReader, Read};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, RwLock};

use failure::Fail;
use serde::{Deserialize, Serialize};
use warp::reject;
use warp::http::StatusCode;
use slog::{info, warn, Logger};
use itertools::Itertools;

use super::TEZOS_CLIENT_DIR;
use crate::handlers::ErrorMessage;

#[derive(Debug, Fail)]
pub enum TezosClientRunnerError {
    /// IO Error.
    #[fail(display = "IO error during process creation")]
    IOError { reason: std::io::Error },

    /// Protocol parameters json error
    #[fail(display = "Error while deserializing parameters json")]
    ProtocolParameterError,

    /// Wallet does not exists error
    #[fail(display = "Alias does not exists among the known wallets")]
    NonexistantWallet,

    /// Wallet already exists error
    #[fail(display = "Alias already exists among the known wallets")]
    WalletAlreadyExistsError,

    /// Serde Error.
    #[fail(display = "Error in serde")]
    SerdeError { reason: serde_json::Error },

    /// Call Error.
    #[fail(display = "Tezos-client call error")]
    CallError { message: ErrorMessage },
}

impl From<std::io::Error> for TezosClientRunnerError {
    fn from(err: std::io::Error) -> TezosClientRunnerError {
        TezosClientRunnerError::IOError { reason: err }
    }
}

impl From<serde_json::Error> for TezosClientRunnerError {
    fn from(err: serde_json::Error) -> TezosClientRunnerError {
        TezosClientRunnerError::SerdeError { reason: err }
    }
}

impl From<TezosClientRunnerError> for reject::Rejection {
    fn from(err: TezosClientRunnerError) -> reject::Rejection {
        reject::custom(err)
    }
}

impl reject::Reject for TezosClientRunnerError {}

/// Type alias for a vecotr of Wallets
pub type SandboxWallets = Vec<Wallet>;

/// Structure holding data used by tezos client
#[derive(Clone, Debug, Deserialize)]
pub struct Wallet {
    alias: String,
    public_key_hash: String,
    public_key: String,
    secret_key: String,
    initial_balance: String,
}

/// The json body incoming with the bake request containing the alias for the wallet to bake with
#[derive(Clone, Debug, Deserialize)]
pub struct BakeRequest {
    alias: String,
}

#[derive(Serialize)]
pub struct TezosClientErrorReply {
    pub message: String,
    pub field_name: String,
}

/// Thread-safe reference to the client runner
pub type TezosClientRunnerRef = Arc<RwLock<TezosClientRunner>>;

/// Structure holding data to use a tezos-client binary
#[derive(Clone)]
pub struct TezosClientRunner {
    pub name: String,
    pub executable_path: PathBuf,
    pub base_dir_path: PathBuf,
    pub wallets: HashMap<String, Wallet>,
}

/// A structure holding all the required parameters to activate an economic protocol
#[derive(Clone, Debug, Deserialize)]
pub struct TezosProtcolActivationParameters {
    timestamp: String,
    protocol_hash: String,
    protocol_parameters: serde_json::Value,
}

#[derive(Serialize, Clone)]
pub struct TezosClientReply {
    pub output: String,
    pub error: String,
}

impl TezosClientReply {
    pub fn new(output: String, error: String) -> Self {
        Self {
            output,
            error,
        }
    }
}

impl Default for TezosClientReply {
    fn default() -> TezosClientReply {
        TezosClientReply::new(String::new(), String::new())
    }
}

impl TezosClientRunner {
    pub fn new(name: &str, executable_path: PathBuf, base_dir_path: PathBuf) -> Self {
        Self {
            name: name.to_string(),
            executable_path,
            base_dir_path,
            wallets: HashMap::new(),
        }
    }

    /// Activate a protocol with the provided parameters
    pub fn activate_protocol(
        &self,
        mut activation_parameters: TezosProtcolActivationParameters,
    ) -> Result<TezosClientReply, reject::Rejection> {
        let mut client_output: TezosClientReply = Default::default();

        // create a temporary file, the tezos-client requires the parameters to be passed in a .json file
        let mut file = fs::File::create("protocol_parameters.json")
            .map_err(|err| reject::custom(TezosClientRunnerError::IOError { reason: err }))?;

        // get as mutable object, so we can insert the hardcoded bootstrap accounts
        let params = if let Some(params) = activation_parameters.protocol_parameters.as_object_mut()
        {
            params
        } else {
            return Err(TezosClientRunnerError::ProtocolParameterError.into());
        };

        let wallet_activation: Vec<[String; 2]> = self
            .wallets
            .clone()
            .into_iter()
            .map(|(_, w)| [w.public_key, w.initial_balance])
            .collect();

        // serialize the harcoded accounts as json array and include it in protocol_parameters
        let sandbox_accounts = serde_json::json!(wallet_activation);
        params.insert("bootstrap_accounts".to_string(), sandbox_accounts);

        // write to a file for the tezos-client
        writeln!(file, "{}", activation_parameters.protocol_parameters)
            .map_err(|err| reject::custom(TezosClientRunnerError::IOError { reason: err }))?;

        self.run_client(
            [
                "--base-dir",
                TEZOS_CLIENT_DIR,
                "-A",
                "localhost",
                "-P",
                "18732",
                "--block",
                "genesis",
                "activate",
                "protocol",
                &activation_parameters.protocol_hash,
                "with",
                "fitness",
                "1",
                "and",
                "key",
                "activator",
                "and",
                "parameters",
                "./protocol_parameters.json",
                "--timestamp",
                &activation_parameters.timestamp,
            ]
            .to_vec(),
            &mut client_output,
        )?;

        // remove the file after activation
        fs::remove_file("./protocol_parameters.json")
            .map_err(|err| reject::custom(TezosClientRunnerError::IOError { reason: err }))?;

        Ok(client_output)
    }

    /// Bake a block with the bootstrap1 account
    pub fn bake_block(&self, request: Option<BakeRequest>) -> Result<TezosClientReply, reject::Rejection> {
        let mut client_output: TezosClientReply = Default::default();

        let alias = if let Some(request) = request {
            if let Some(wallet) = self.wallets.get(&request.alias) {
                &wallet.alias
            } else {
                return Err(TezosClientRunnerError::NonexistantWallet.into());
            }
        } else {
            // if there is no wallet provided in the request (GET) set the alias to be an arbitrary wallet
            if let Some(wallet) = self.wallets.values().next() {
                &wallet.alias
            } else {
                return Err(TezosClientRunnerError::NonexistantWallet.into());
            }
        };

        self.run_client(
            [
                "--base-dir",
                TEZOS_CLIENT_DIR,
                "-A",
                "localhost",
                "-P",
                "18732",
                "bake",
                "for",
                &alias,
            ]
            .to_vec(),
            &mut client_output,
        )?;

        Ok(client_output)
    }

    /// Initialize the accounts in the tezos-client
    pub fn init_client_data(
        &mut self,
        requested_wallets: SandboxWallets,
    ) -> Result<TezosClientReply, reject::Rejection> {
        let mut client_output: TezosClientReply = Default::default();

        self.run_client(
            [
                "--base-dir",
                TEZOS_CLIENT_DIR,
                "-A",
                "localhost",
                "-P",
                "18732",
                "import",
                "secret",
                "key",
                "activator",
                "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6",
            ]
            .to_vec(),
            &mut client_output,
        )?;

        for wallet in requested_wallets {
            self.run_client(
                [
                    "--base-dir",
                    TEZOS_CLIENT_DIR,
                    "-A",
                    "localhost",
                    "-P",
                    "18732",
                    "import",
                    "secret",
                    "key",
                    &wallet.alias,
                    &format!("unencrypted:{}", &wallet.secret_key),
                ]
                .to_vec(),
                &mut client_output,
            )?;
            self.wallets.insert(wallet.alias.clone(), wallet);
        }

        Ok(client_output)
    }

    /// Cleanup the tezos-client directory
    pub fn cleanup(&self) -> Result<(), reject::Rejection> {
        fs::remove_dir_all(TEZOS_CLIENT_DIR)
            .map_err(|err| reject::custom(TezosClientRunnerError::IOError { reason: err }))?;
        fs::create_dir(TEZOS_CLIENT_DIR)
            .map_err(|err| reject::custom(TezosClientRunnerError::IOError { reason: err }))?;

        Ok(())
    }

    /// Private method to run the tezos-client as a subprocess and wait for its completion
    fn run_client(&self, args: Vec<&str>, client_output: &mut TezosClientReply) -> Result<(), reject::Rejection> {
        let output = Command::new(&self.executable_path)
            .args(args)
            .output()
            .map_err(|err| reject::custom(TezosClientRunnerError::IOError { reason: err }))?;
        let _ = BufReader::new(output.stdout.as_slice()).read_to_string(&mut client_output.output);
        let _ = BufReader::new(output.stderr.as_slice()).read_to_string(&mut client_output.error);

        Ok(())
    }
}

/// Construct a reply using the output from the tezos-client
pub fn reply_with_client_output(reply: TezosClientReply, log: &Logger) -> Result<impl warp::Reply, reject::Rejection> {
    if reply.error.is_empty() {
        info!(log, "Successfull tezos-client call: {}", reply.output);
        Ok(StatusCode::OK)
    } else {
        if reply.output.is_empty() {
            // error
            if let Some((field_name, message)) = extract_field_name_and_message_ocaml(&reply) {
                Err(TezosClientRunnerError::CallError { message: ErrorMessage::validation(500, message, field_name)}.into())
            } else {
                // generic
                warn!(log, "GENERIC ERROR in tezos-client, log: {}", reply.error);
                Err(TezosClientRunnerError::CallError { message: ErrorMessage::generic(500, "Unexpexted error in tezos-client call".to_string())}.into())
            }
        } else {
            // this is just a warning
            warn!(log, "Succesfull call with a warning: {} -> WARNING: {}", reply.output, reply.error);
            Ok(StatusCode::OK)
        }
    }
}

/// Parse the returned error string from the tezos client
fn extract_field_name_and_message_ocaml(reply: &TezosClientReply) -> Option<(String, String)>{
    let parsed_message = reply.error.replace("\\", "").split("\"").filter(|s| s.contains("Invalid protocol_parameters")).join("").replace(" n{ ", "").replace("{", "");

    // extract the field name depending on the parsed error
    let field_name = if parsed_message.contains("Missing object field") {
        Some(parsed_message.split_whitespace().last().unwrap_or("").to_string())
    } else if parsed_message.contains("/") {
        Some(parsed_message.split_whitespace().filter(|s| s.contains("/")).join("").replace("/", "").replace(",", ""))
    } else {
        None
    };

    if let Some(field_name) = field_name {
        // simply remove the field name from the error message
        let message = parsed_message.replace(&field_name, "").replace("At /, ", "").trim().to_string();
        Some((field_name, message))
    } else {
        None
    }
}
