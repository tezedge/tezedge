use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, RwLock};
use std::thread;
use std::{io::{Read, BufReader}, time::Duration};

use failure::Fail;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use wait_timeout::ChildExt;
use warp::reject;
use itertools::Itertools;

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

    /// Startup Error
    #[fail(display = "Error after light-node process spawned")]
    NodeStartupError { reason: String } ,
}

impl From<std::io::Error> for LightNodeRunnerError {
    fn from(err: std::io::Error) -> LightNodeRunnerError {
        LightNodeRunnerError::IOError { reason: err }
    }
}

impl From<LightNodeRunnerError> for reject::Rejection {
    fn from(err: LightNodeRunnerError) -> reject::Rejection {
        reject::custom(err)
    }
}

impl reject::Reject for LightNodeRunnerError {}

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

    /// Spawn a light-node child process with health check
    pub fn spawn(&mut self, cfg: serde_json::Value) -> Result<(), reject::Rejection> {
        if self.is_running() {
            Err(LightNodeRunnerError::NodeAlreadyRunning.into())
        } else {
            // spawn the process for a "health check" with piped stderr
            let mut process = Command::new(&self.executable_path)
                .args(Self::construct_args(cfg.clone())?)
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|err| LightNodeRunnerError::IOError { reason: err })?;

            // give some time for the process to spawn, a second is sufficient, we only care if it fails to start up
            // and if it does, it fails almost instatnly
            thread::sleep(Duration::from_secs(1));

            // check for live process
            match process.try_wait() {
                // process exited, we need to handle and show the exact error
                Ok(Some(_)) => {
                    let error_msg = handle_stderr(&mut process);
                    return Err(LightNodeRunnerError::NodeStartupError{ reason: error_msg.split("USAGE:").take(1).join("").replace("error:", "").trim().into()}.into());
                }
                // the process started up OK, but we restart it to enable normal logging (stderr won't be piped)
                _ => {
                    match Self::send_sigint(process.id()) {
                        Ok(()) => {
                            self.process = None;
                        }
                        // if for some reason, the SIGINT fails to end the process, kill it with SIGKILL
                        Err(_) => {
                            Self::terminate_ref(&mut process);
                        }
                    }

                    // enable a to shut down gracefully
                    thread::sleep(Duration::from_secs(4));

                    // start the process again
                    let process = Command::new(&self.executable_path)
                        .args(Self::construct_args(cfg)?)
                        .spawn()
                        .map_err(|err| LightNodeRunnerError::IOError { reason: err })?;

                    self.process = Some(process);
                    Ok(())
                }
            }
        }
    }

    /// Shut down the light-node
    pub fn shut_down(&mut self) -> Result<(), reject::Rejection> {
        if self.is_running() {
            let process = self.process.as_mut().unwrap();
            // kill with SIGINT (ctr-c)
            match Self::send_sigint(process.id()) {
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
            Err(LightNodeRunnerError::NodeNotRunnig.into())
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
    fn construct_args(cfg: serde_json::Value) -> Result<Vec<String>, reject::Rejection> {
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
            Err(LightNodeRunnerError::JsonParsingError.into())
        }
    }

    /// Send SIGINT signal to the process with PID, light-node is cheking for this signal and shuts down
    /// gracefully if recieved
    fn send_sigint(pid: u32) -> Result<(), nix::Error>{
        signal::kill(Pid::from_raw(pid as i32), Signal::SIGINT)
    }
}

/// extract data as String form the piped stderr
fn handle_stderr(process: &mut Child) -> String {
    let stderr = process.stderr.take().unwrap();
    let mut extract = String::new();
    let _ = BufReader::new(stderr).read_to_string(&mut extract);

    extract
}