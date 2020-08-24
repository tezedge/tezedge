use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use failure::Fail;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use wait_timeout::ChildExt;

#[derive(Debug, Fail)]
pub enum LightNodeRunnerError {
    /// Already running error.
    #[fail(display = "light node is already running")]
    NodeAlreadyRunning,

    /// Already running error.
    #[fail(display = "light node is not running")]
    NodeNotRunnig,

    /// IO Error.
    #[fail(display = "IO error during process creation")]
    IOError { reason: std::io::Error },

    /// Json argument parsing error.
    #[fail(display = "Json argument parsing error")]
    JsonParsingError,
}

impl From<std::io::Error> for LightNodeRunnerError {
    fn from(err: std::io::Error) -> LightNodeRunnerError {
        LightNodeRunnerError::IOError { reason: err }
    }
}

/// Thread safe reference to a shared Runner
pub type LightNodeRunnerRef = Arc<RwLock<LightNodeRunner>>;

/// Struct that holds info about the running child process
pub struct LightNodeRunner {
    executable_path: PathBuf,
    _name: String,
    process: Option<Child>,
    // TODO: TE-213 Launch multiple nodes
}

impl LightNodeRunner {
    const PROCESS_WAIT_TIMEOUT: Duration = Duration::from_secs(4);

    pub fn new(name: &str, executable_path: PathBuf) -> Self {
        Self {
            executable_path,
            _name: name.to_string(),
            process: None,
        }
    }

    /// Spawn a light-node child process
    pub fn spawn(&mut self, cfg: serde_json::Value) -> Result<(), LightNodeRunnerError> {
        if self.is_running() {
            Err(LightNodeRunnerError::NodeAlreadyRunning)
        } else {
            let process = Command::new(&self.executable_path)
                .args(Self::construct_args(cfg)?)
                .spawn()?;
            self.process = Some(process);
            Ok(())
        }
    }

    /// Shut down the light-node
    pub fn shut_down(&mut self) -> Result<(), LightNodeRunnerError> {
        if self.is_running() {
            let process = self.process.as_mut().unwrap();
            // kill with SIGINT (ctr-c)
            match signal::kill(Pid::from_raw(process.id() as i32), Signal::SIGINT) {
                Ok(()) => {
                    self.process = None;
                    Ok(())
                }
                // if for some reason, the SIGINT fails to end the process, kill it with SIGKILL
                Err(_) => {
                    Self::terminate_ref(process);
                    Ok(())
                }
            }
        } else {
            Err(LightNodeRunnerError::NodeNotRunnig)
        }
    }

    fn terminate_ref(process: &mut Child) {
        match process.wait_timeout(Self::PROCESS_WAIT_TIMEOUT).unwrap() {
            Some(_) => (),
            None => {
                // child hasn't exited yet
                let _ = process.kill();
            }
        };
    }

    fn is_running(&mut self) -> bool {
        if let Some(process) = &mut self.process {
            match process.try_wait() {
                Ok(None) => true,
                _ => false,
            }
        } else {
            false
        }
    }

    /// function to construct a vector with all the passed (via RPC) arguments
    fn construct_args(cfg: serde_json::Value) -> Result<Vec<String>, LightNodeRunnerError> {
        let mut args: Vec<String> = Vec::new();

        if let Some(arg_map) = cfg.as_object() {
            for (key, value) in arg_map {
                args.push(format!("--{}", key.replace("_", "-").replace("\"", "")));
                let str_val = value.to_string().replace("\"", "");

                // options are defined as a key with an empty string
                if !str_val.is_empty() {
                    args.push(str_val);
                }
            }
            Ok(args)
        } else {
            Err(LightNodeRunnerError::JsonParsingError)
        }
    }
}
