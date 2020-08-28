use std::path::PathBuf;
use std::process::Command;
use std::fs;
use std::io::Write;

use failure::Fail;
use serde::Deserialize;

#[derive(Debug, Fail)]
pub enum TezosClientRunnerError {
    /// IO Error.
    #[fail(display = "IO error during process creation")]
    IOError { reason: std::io::Error },

    /// Path Error
    #[fail(display = "Json argument parsing error")]
    PathError,

    /// Protocol parameters json error
    #[fail(display = "Error while deserializing parameters json")]
    ProtocolParameterError,

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

#[derive(Clone)]
pub struct TezosClientRunner {
    pub name: String,
    pub executable_path: PathBuf,
    pub base_dir_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TezosProtcolActivationParameters {
    timestamp: String,
    protocol_hash: String,
    protocol_parameters: serde_json::Value,

    #[serde(skip)]
    accounts: Vec<Vec<String>>
}

// hardcoded bootstrap accounts
pub const SANDBOX_ACCOUNTS: &str = r#"
    [
      [
        "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
        "4000000000000"
      ],
      [
        "edpktzNbDAUjUk697W7gYg2CRuBQjyPxbEg8dLccYYwKSKvkPvjtV9",
        "4000000000000"
      ],
      [
        "edpkuTXkJDGcFd5nh6VvMz8phXxU3Bi7h6hqgywNFi1vZTfQNnS1RV",
        "4000000000000"
      ],
      [
        "edpkuFrRoDSEbJYgxRtLx2ps82UdaYc1WwfS9sE11yhauZt5DgCHbU",
        "4000000000000"
      ],
      [
        "edpkv8EUUH68jmo3f7Um5PezmfGrRF24gnfLpH3sVNwJnV5bVCxL2n",
        "4000000000000"
      ]
    ]
  "#;

impl TezosClientRunner {
    pub fn new(name: &str, executable_path: PathBuf, base_dir_path: PathBuf) -> Self {
        Self {
            name: name.to_string(),
            executable_path,
            base_dir_path,
        }
    }

    /// Activate a protocol with the provided parameters
    pub fn activate_protocol(&self, mut activation_parameters: TezosProtcolActivationParameters) -> Result<(), TezosClientRunnerError> {
        let base_dir = if let Some(path) = self.base_dir_path.to_str() {
            path
        } else {
            return Err(TezosClientRunnerError::PathError);
        };

        // create a temporary file, the tezos-client requires the parameters to be passed in a .json file
        // we won't use tempfile becouse we need the path to the file
        let mut file = fs::File::create("protocol_parameters.json")?;
        
        // get as mutable object, so we can insert the hardcoded bootstrap accounts
        // Note: we can include it in the request from the FE
        let params = if let Some(params) = activation_parameters.protocol_parameters.as_object_mut() {
            params
        } else {
            return Err(TezosClientRunnerError::ProtocolParameterError)
        };

        // serialize the harcoded accounts as json array and include it in protocol_parameters
        let sandbox_accounts = serde_json::from_str(SANDBOX_ACCOUNTS)?;
        params.insert("bootstrap_accounts".to_string(), sandbox_accounts);

        // write to a file for the tezos-client
        writeln!(file, "{}", activation_parameters.protocol_parameters);

        self.run_client(
            [
                "--base-dir",
                base_dir,
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
        fs::remove_file("./protocol_parameters.json")?;

        Ok(())
    }

    /// Bake a block with the bootstrap1 account
    pub fn bake_block(&self) -> Result<(), TezosClientRunnerError> {
        let base_dir = if let Some(path) = self.base_dir_path.to_str() {
            path
        } else {
            return Err(TezosClientRunnerError::PathError);
        };

        self.run_client(
            [
                "--base-dir",
                base_dir,
                "-A",
                "localhost",
                "-P",
                "18732",
                "bake",
                "for",
                "bootstrap1",
            ]
            .to_vec(),
        )?;

        Ok(())
    }

    /// Initialize the accounts in the tezos-client
    pub fn init_client_data(&self) -> Result<(), TezosClientRunnerError> {
        let base_dir = if let Some(path) = self.base_dir_path.to_str() {
            println!("PATH: {}", path);
            path
        } else {
            return Err(TezosClientRunnerError::PathError);
        };
        println!("Base dir: {}", base_dir);

        self.run_client(
            [
                "--base-dir",
                base_dir,
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
        self.run_client(
            [
                "--base-dir",
                base_dir,
                "-A",
                "localhost",
                "-P",
                "18732",
                "import",
                "secret",
                "key",
                "bootstrap1",
                "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
            ]
            .to_vec(),
        )?;
        self.run_client(
            [
                "--base-dir",
                base_dir,
                "-A",
                "localhost",
                "-P",
                "18732",
                "import",
                "secret",
                "key",
                "bootstrap2",
                "unencrypted:edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo",
            ]
            .to_vec(),
        )?;
        Ok(())
    }

    /// Cleanup the tezos-client directory
    pub fn cleanup(&self) -> Result<(), TezosClientRunnerError> {
        let base_dir = if let Some(path) = self.base_dir_path.to_str() {
            path
        } else {
            return Err(TezosClientRunnerError::PathError);
        };

        fs::remove_dir_all(base_dir)?;
        fs::create_dir(base_dir)?;

        Ok(())
    }

    /// Private method to run the tezos-client as a subprocess and wait for its completion
    fn run_client(&self, args: Vec<&str>) -> Result<(), TezosClientRunnerError> {
        let _ = Command::new(&self.executable_path)
            .args(args)
            .spawn()?
            .wait();
        Ok(())
    }
}
