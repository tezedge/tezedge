use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, RwLock};

use failure::Fail;
use serde::Deserialize;
use warp::reject;

use super::TEZOS_CLIENT_DIR;

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
    ) -> Result<(), reject::Rejection> {
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
        )?;

        // remove the file after activation
        fs::remove_file("./protocol_parameters.json")
            .map_err(|err| reject::custom(TezosClientRunnerError::IOError { reason: err }))?;

        Ok(())
    }

    /// Bake a block with the bootstrap1 account
    pub fn bake_block(&self, request: BakeRequest) -> Result<(), reject::Rejection> {
        let alias = if let Some(wallet) = self.wallets.get(&request.alias) {
            &wallet.alias
        } else {
            return Err(TezosClientRunnerError::NonexistantWallet.into());
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
        )?;

        Ok(())
    }

    /// Initialize the accounts in the tezos-client
    pub fn init_client_data(
        &mut self,
        requested_wallets: SandboxWallets,
    ) -> Result<(), reject::Rejection> {
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
            )?;
            self.wallets.insert(wallet.alias.clone(), wallet);
        }

        Ok(())
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
    fn run_client(&self, args: Vec<&str>) -> Result<(), reject::Rejection> {
        let _ = Command::new(&self.executable_path)
            .args(args)
            .spawn()
            .map_err(|err| reject::custom(TezosClientRunnerError::IOError { reason: err }))?
            .wait();
        Ok(())
    }
}
