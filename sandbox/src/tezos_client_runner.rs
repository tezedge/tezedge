use std::path::PathBuf;
use std::process::Command;
use std::fs;

use failure::Fail;

#[derive(Debug, Fail)]
pub enum TezosClientRunnerError {
    /// IO Error.
    #[fail(display = "IO error during process creation")]
    IOError { reason: std::io::Error },

    /// Path Error
    #[fail(display = "Json argument parsing error")]
    PathError,
}

impl From<std::io::Error> for TezosClientRunnerError {
    fn from(err: std::io::Error) -> TezosClientRunnerError {
        TezosClientRunnerError::IOError { reason: err }
    }
}

#[derive(Clone)]
pub struct TezosClientRunner {
    pub name: String,
    pub executable_path: PathBuf,
    pub base_dir_path: PathBuf,
}

impl TezosClientRunner {
    pub fn new(name: &str, executable_path: PathBuf, base_dir_path: PathBuf) -> Self {
        Self {
            name: name.to_string(),
            executable_path,
            base_dir_path,
        }
    }

    pub fn activate_protocol(&self) -> Result<(), TezosClientRunnerError> {
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
                "--block",
                "genesis",
                "activate",
                "protocol",
                "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb",
                "with",
                "fitness",
                "1",
                "and",
                "key",
                "activator",
                "and",
                "parameters",
                "./light_node/etc/tezedge_sandbox/006-carthage-protocol-parameters.json",
                "--timestamp",
                "2020-06-24T08:02:48Z",
            ]
            .to_vec(),
        )?;
        Ok(())
    }

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

    fn run_client(&self, args: Vec<&str>) -> Result<(), TezosClientRunnerError> {
        let _ = Command::new(&self.executable_path)
            .args(args)
            .spawn()?
            .wait();
        Ok(())
    }
}
