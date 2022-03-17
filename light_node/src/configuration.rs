// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::ffi::OsString;
use std::fmt::Display;
use std::{fs, env};
use std::io::{self, BufRead};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use std::{collections::HashMap, collections::HashSet, fmt::Debug};

use clap::{ArgEnum, Parser, Subcommand};
use slog::Logger;

use crypto::hash::BlockHash;
use logging::config::{FileLoggerConfig, LogFormat, LoggerType, NoDrainError, SlogConfig};
use shell::shell_automaton_manager::P2p;
use shell::PeerConnectionThreshold;
use storage::database::tezedge_database::TezedgeDatabaseBackendConfiguration;
use storage::initializer::{DbsRocksDbTableInitializer, RocksDbConfig};
use storage::BlockReference;
use tezedge_actor_system::actors::ActorReference;
use tezos_api::environment::{self, TezosEnvironmentConfiguration};
use tezos_api::environment::{TezosEnvironment, ZcashParams};
use tezos_context_api::{
    ContextKvStoreConfiguration, PatchContext, SupportedContextKeyValueStore,
    TezosContextIrminStorageConfiguration, TezosContextStorageConfiguration,
    TezosContextTezEdgeStorageConfiguration, TezosContextTezedgeOnDiskBackendOptions,
};

#[derive(Clone, Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(args_override_self = true)]
struct CliArgs {
    /// Validate configuration and generated identity, than just stops application
    #[clap(long)]
    validate_cfg_identity_and_stop: bool,

    /// Configuration file with start-up arguments (same format as cli arguments)
    #[clap(long, global = true, parse(from_os_str), validator = validate_path_exists)]
    config_file: Option<PathBuf>,

    /// Context storage to use
    #[clap(long, global = true, arg_enum, parse(try_from_str), default_value_t = TezosContextStorageChoice::Irmin)]
    tezos_context_storage: TezosContextStorageChoice,

    /// A directory for Tezos OCaml runtime storage (context/store)
    #[clap(long, global = true, validator = validate_directory_exists_create, parse(from_os_str), default_value = "/tmp/tezedge")]
    tezos_data_dir: PathBuf,

    /// Enable or not the integrity check on persistent tezedge context
    #[clap(long)]
    context_integrity_check: bool,

    /// Path to the json identity file with peer-id, public-key, secret-key and pow-stamp.
    /// In case it starts with ./ or ../, it is relative path to the current dir, otherwise to the --tezos-data-dir
    #[clap(long, global = true, default_value = "./light_node/etc/tezedge/identity.json")]
    identity_file: PathBuf,

    /// Expected power of identity for node. It is used to generate new identity
    #[clap(long, parse(try_from_str), default_value_t = 26.0)]
    identity_expected_pow: f64,

    /// Path to bootstrap database directory.
    /// In case it starts with ./ or ../, it is relative path to the current dir, otherwise to the --tezos-data-dir
    #[clap(long, global = true, default_value = "bootstrap_db")]
    bootstrap_db_path: PathBuf,

    /// Path to context-stats database directory.
    /// In case it starts with ./ or ../, it is relative path to the current dir, otherwise to the --tezos-data-dir
    #[clap(long, validator = validate_path_exists)]
    context_stats_db_path: Option<PathBuf>,

    /// Panic if the context initialization of application took longer than this number of seconds
    #[clap(long, parse(try_from_str), default_value_t = Storage::DEFAULT_INITIALIZE_CONTEXT_TIMEOUT_IN_SECONDS)]
    initialize_context_timeout_in_secs: u64,

    /// Panic if the chain manager first initialization of application took longer than this number of seconds
    #[clap(long, parse(try_from_str), default_value_t = Environment::DEFAULT_INITIALIZE_CHAIN_MANAGER_TIMEOUT_IN_SECONDS)]
    initialize_chain_manager_timeout_in_secs: u64,

    /// Max number of threads used by database configuration. If not specified, then number of threads equal to CPU cores.
    #[clap(long, parse(try_from_str))]
    db_cfg_max_threads: Option<usize>,

    /// A peers for dns lookup to get the peers to bootstrap the network from. Peers are delimited by a colon.
    /// Used according to --network parameter see TezosEnvironment
    #[clap(long, parse(try_from_str), group = "bootstrap_lookup")]
    bootstrap_lookup_address: Vec<String>,

    /// Disables dns lookup to get the peers to bootstrap the network from
    #[clap(long, group = "bootstrap_lookup")]
    disable_bootstrap_lookup: bool,

    /// Set the logger target. Possible values: terminal, file
    #[clap(long, parse(try_from_str), default_value = "terminal")]
    log: Vec<String>,

    /// Path to the log file. 
    /// In case it starts with ./ or ../, it is relative path to the current dir, otherwise to the --tezos-data-dir
    #[clap(long, default_value = Logging::DEFAULT_FILE_LOGGER_PATH)]
    log_file: PathBuf,

    /// Used for log file rotation, if actual log file reaches this size-in-bytes, it will be rotated to '*.0.gz, .1.gz, ...'
    #[clap(long, parse(try_from_str), default_value_t = Logging::DEFAULT_FILE_LOGGER_ROTATE_IF_SIZE_IN_BYTES)]
    log_rotate_if_size_in_bytes: u64,

    /// Used for log file rotation, how many rotated files do we want to keep '*.0.gz, .1.gz, ...'
    #[clap(long, parse(try_from_str), default_value_t = Logging::DEFAULT_FILE_LOGGER_KEEP_NUMBER_OF_ROTATED_FILE)]
    log_rotate_keep_logs_number: u16,

    /// Set output format of the log
    #[clap(long, parse(try_from_str), default_value_t = LogFormatArg::Simple)]
    log_format: LogFormatArg,

    /// Set logging level
    #[clap(long, arg_enum, parse(try_from_str), default_value_t = LogLevelArg::Info)]
    log_level: LogLevelArg,

    /// Flag for turn on/off logging in Tezos OCaml runtime
    #[clap(long)]
    ocaml_log_enabled: bool,

    /// Enable or disable mempool
    #[clap(long)]
    disable_mempool: bool,

    /// Enable or disable blocks prechecking
    #[clap(long)]
    disable_block_precheck: bool,

    /// Enable or disable prechecking of endorsements
    #[clap(long)]
    disable_endorsements_precheck: bool,

    /// Enable or disable retries when block fails to apply
    #[clap(long)]
    disable_apply_retry: bool,

    /// Enable or disable peer graylisting
    #[clap(long)]
    disable_peer_graylist: bool,

    /// Enable or disable private node. Use peers to set IP addresses of the peers you want to connect to
    #[clap(long, group = "bootstrap_lookup", requires = "peers")]
    private_node: bool,

    /// Tezos network to connect to
    #[clap(long, arg_enum)]
    network: TezosEnvironment,

    /// Path to a JSON file defining a custom network using the same format used by Octez
    #[clap(long, validator = validate_path_exists)]
    custom_network_file: Option<PathBuf>,

    /// Socket listening port for p2p for communication with tezos world
    #[clap(long, parse(try_from_str), default_value_t = 9732)]
    p2p_port: u16,

    /// Rust server RPC port for communication with rust node
    #[clap(long, parse(try_from_str), default_value_t = 18732)]
    rpc_port: u16,

    /// Flag for enable/disable test chain switching for block applying
    #[clap(long)]
    enable_testchain: bool,

    /// Websocket address where various node metrics and statistics are available
    #[clap(long, parse(try_from_str))]
    websocket_address: Option<SocketAddr>,

    /// Websocket max number of allowed concurrent connection
    #[clap(long, parse(try_from_str), default_value_t = Rpc::DEFAULT_WEBSOCKET_MAX_CONNECTIONS)]
    websocket_max_connections: u16,

    /// Peers to bootstrap the network from.
    #[clap(long, parse(try_from_str))]
    peers: Vec<SocketAddr>,

    /// Minimal number of peers to connect to
    #[clap(long, parse(try_from_str), default_value_t = 20)]
    peer_thresh_low: usize,

    /// Maximal number of peers to connect to
    #[clap(long, parse(try_from_str), default_value_t = 30)]
    peer_thresh_high: usize,

    /// Set the synchronizations threshol
    #[clap(long, parse(try_from_str))]
    synchronization_thresh: Option<usize>,

    /// Path to a tezos protocol runner executable
    #[clap(long, global = true, validator = validate_path_exists, default_value = "./target/release/protocol-runner")]
    protocol_runner: PathBuf,

    /// Path to a init file for sapling-spend.params
    #[clap(long, global = true, validator = validate_path_exists, default_value = Ffi::DEFAULT_ZCASH_PARAM_SAPLING_SPEND_FILE_PATH)]
    init_sapling_spend_params_file: PathBuf,

    /// Path to a init file for sapling-output.params
    #[clap(long, global = true, validator = validate_path_exists, default_value = Ffi::DEFAULT_ZCASH_PARAM_SAPLING_OUTPUT_FILE_PATH)]
    init_sapling_output_params_file: PathBuf,

    /// Number of threads spawned by a tokio thread pool. If value is zero, then number of threads equal to CPU cores is spawned.
    #[clap(long, parse(try_from_str), default_value_t = 0)]
    tokio_threads: usize,

    /// Number of threads spawned by a riker (actor system) thread pool. If value is zero, then number of threads equal to CPU cores is spawned.
    #[clap(long, parse(try_from_str), default_value_t = 0)]
    riker_threads: usize,

    /// Options fo main database backend
    #[clap(long, arg_enum, default_value_t = Storage::DEFAULT_MAINDB)]
    maindb_backend: TezedgeDatabaseBackendConfiguration,

    /// Choose the TezEdge context storage backend
    #[clap(long, arg_enum, default_value_t = Storage::DEFAULT_CONTEXT_KV_STORE_BACKEND)]
    context_kv_store: SupportedContextKeyValueStore,

    /// Activate the computation of tree hashes when applying context actions
    #[clap(long)]
    compute_context_action_tree_hashes: bool,

    /// Enable recording/persisting shell automaton state snapshots.
    #[clap(long)]
    record_shell_automaton_state_snapshots: bool,

    /// Enable recording/persisting shell automaton actions.
    #[clap(long)]
    record_shell_automaton_actions: bool,

    #[clap(long, validator = validate_path_exists)]
    sandbox_patch_context_json_file: Option<PathBuf>,

    #[clap(subcommand)]
    command: Option<Commands>,

    /// Override node's current head level in order to bootstrap from that block.
    #[clap(long, parse(try_from_str))]
    current_head_level_override: Option<i32>
}

impl CliArgs {
    fn set_tezos_data_dir(&mut self, tezos_data_dir: PathBuf) {
        self.tezos_data_dir = tezos_data_dir;
    }
}

#[derive(Clone, Debug, Subcommand)]
pub enum Commands {
    /// Replay a specific part of the chain
    Replay {
        /// Block from which we start the replay
        #[clap(long, parse(try_from_str))]
        from_block: Option<BlockHash>,

        /// Replay until this block
        #[clap(long, parse(try_from_str))]
        to_block: BlockHash,

        /// A directory for the replay
        #[clap(long, validator = validate_directory_exists_create, default_value = "/tmp/replay")]
        target_path: PathBuf,

        /// Panic if the block application took longer than this number of milliseconds
        #[clap(long, parse(try_from_str))]
        fail_above: Option<u64>,
    },
    /// Create a full snapshot from the node data
    Snapshot {
        // TODO: replicate the vlidation
        /// Block to snapshot (by hash, level or offset (~<NUM>) from head)
        #[clap(long)]
        block: Option<String>,

        /// Directory where the snapshot will be created
        #[clap(long, validator = validate_directory_exists_create, default_value = "/tmp/tezedge-snapshot")]
        target_path: PathBuf,
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum LogFormatArg {
    Simple,
    Json,
}

impl Display for LogFormatArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogFormatArg::Simple => write!(f, "simple"),
            LogFormatArg::Json => write!(f, "json"),
        }
    }
}

impl FromStr for LogFormatArg {
    type Err = InvalidVariant;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "simple" => Ok(LogFormatArg::Simple),
            "json" => Ok(LogFormatArg::Json),
            _ => Err(InvalidVariant(format!("Invalid log format: {}", s)))
        }
    }
}

impl From<LogFormatArg> for LogFormat {
    fn from(format: LogFormatArg) -> Self {
        match format {
            LogFormatArg::Simple => LogFormat::Simple,
            LogFormatArg::Json => LogFormat::Json,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum LogLevelArg {
    Critical,
    Error,
    Warning,
    Info,
    Debug,
    Trace,
}

impl From<LogLevelArg> for slog::Level {
    fn from(val: LogLevelArg) -> Self {
        match val {
            LogLevelArg::Critical => slog::Level::Critical,
            LogLevelArg::Error => slog::Level::Error,
            LogLevelArg::Warning => slog::Level::Warning,
            LogLevelArg::Info => slog::Level::Info,
            LogLevelArg::Debug => slog::Level::Debug,
            LogLevelArg::Trace => slog::Level::Trace,
        }
    }
}

fn validate_directory_exists_create(s: &str) -> Result<(), String> {
    let dir = Path::new(&s);
    if dir.exists() {
        if dir.is_dir() {
            Ok(())
        } else {
            Err(format!("Required dir '{}' exists, but is not a directory!", s))
        }
    } else {
        // Directory does not exists, try to create it
        if let Err(e) = fs::create_dir_all(dir) {
            Err(format!("Unable to create directory '{}': {} ", s, e))
        } else {
            Ok(())
        }
    }
}

fn validate_path_exists(s: &str) -> Result<(), String> {
    if Path::new(s).exists() {
        Ok(())
    } else {
        Err(format!("File not found at '{}'", s))
    }
}

#[derive(Debug, Clone)]
pub struct Rpc {
    pub listener_port: u16,
    /// Tuple of :
    ///     SocketAddr
    ///     u16 - max_number_of_websocket_connections
    pub websocket_cfg: Option<(SocketAddr, u16)>,
}

impl Rpc {
    const DEFAULT_WEBSOCKET_MAX_CONNECTIONS: u16 = 100;
}

#[derive(Debug, Clone)]
pub struct Logging {
    pub slog: SlogConfig,
    pub ocaml_log_enabled: bool,
}

impl Logging {
    const DEFAULT_FILE_LOGGER_PATH: &'static str = "./tezedge.log";
    const DEFAULT_FILE_LOGGER_ROTATE_IF_SIZE_IN_BYTES: u64 = 10_485_760 * 10; // 100 MB
    const DEFAULT_FILE_LOGGER_KEEP_NUMBER_OF_ROTATED_FILE: u16 = 100; // 100 MB * 100 = 10 GB
}

#[derive(Debug, Clone, thiserror::Error)]
pub struct InvalidVariant(String);

impl Display for InvalidVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum TezosContextStorageChoice {
    Irmin,
    TezEdge,
    Both,
}

impl FromStr for TezosContextStorageChoice {
    type Err = InvalidVariant;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_ref() {
            "both" => Ok(TezosContextStorageChoice::Both),
            "irmin" => Ok(TezosContextStorageChoice::Irmin),
            "tezedge" => Ok(TezosContextStorageChoice::TezEdge),
            _ => Err(InvalidVariant(format!(
                "Invalid context storage name: {}",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Storage {
    pub db: RocksDbConfig<DbsRocksDbTableInitializer>,
    pub db_path: PathBuf,
    pub context_stats_db_path: Option<PathBuf>,
    pub context_storage_configuration: TezosContextStorageConfiguration,
    pub compute_context_action_tree_hashes: bool,
    pub patch_context: Option<PatchContext>,
    pub main_db: TezedgeDatabaseBackendConfiguration,
    pub initialize_context_timeout: Duration,
}

impl Storage {
    const STORAGES_COUNT: usize = 3;
    const MINIMAL_THREAD_COUNT: usize = 1;

    const DB_STORAGE_VERSION: i64 = 21;

    const LRU_CACHE_SIZE_96MB: usize = 96 * 1024 * 1024;

    const DEFAULT_CONTEXT_KV_STORE_BACKEND: SupportedContextKeyValueStore = SupportedContextKeyValueStore::OnDisk;

    const DEFAULT_MAINDB: TezedgeDatabaseBackendConfiguration = TezedgeDatabaseBackendConfiguration::RocksDB;

    const DEFAULT_INITIALIZE_CONTEXT_TIMEOUT_IN_SECONDS: u64 = 15;
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub identity_json_file_path: PathBuf,
    pub expected_pow: f64,
}

#[derive(Debug, Clone)]
pub struct Ffi {
    pub protocol_runner: PathBuf,
    pub zcash_param: ZcashParams,
}

impl Ffi {
    pub const DEFAULT_ZCASH_PARAM_SAPLING_SPEND_FILE_PATH: &'static str =
        "tezos/sys/lib_tezos/artifacts/sapling-spend.params";
    pub const DEFAULT_ZCASH_PARAM_SAPLING_OUTPUT_FILE_PATH: &'static str =
        "tezos/sys/lib_tezos/artifacts/sapling-output.params";
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub p2p: P2p,
    pub rpc: Rpc,
    pub logging: Logging,
    pub storage: Storage,
    pub identity: Identity,
    pub ffi: Ffi,
    pub sub_command: Option<Commands>,

    pub tezos_network: TezosEnvironment,
    pub tezos_network_config: TezosEnvironmentConfiguration,

    pub enable_testchain: bool,
    pub tokio_threads: usize,
    pub riker_threads: usize,

    /// This flag is used, just for to stop node immediatelly after generate identity,
    /// to prevent and initialize actors and create data (except identity)
    pub validate_cfg_identity_and_stop: bool,

    pub initialize_chain_manager_timeout: Duration,
}

impl Environment {
    const DEFAULT_INITIALIZE_CHAIN_MANAGER_TIMEOUT_IN_SECONDS: u64 = 10;
}

impl slog::Value for Environment {
    fn serialize(
        &self,
        _record: &slog::Record,
        _: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_arguments("p2p", &format_args!("{:?}", self.p2p))?;
        serializer.emit_arguments("rpc", &format_args!("{:?}", self.rpc))?;
        serializer.emit_arguments("logging", &format_args!("{:?}", self.logging))?;
        serializer.emit_arguments("storage", &format_args!("{:?}", self.storage))?;
        serializer.emit_arguments("identity", &format_args!("{:?}", self.identity))?;
        serializer.emit_arguments("ffi", &format_args!("{:?}", self.ffi))?;
        // serializer.emit_arguments("replay", &format_args!("{:?}", self.replay))?;
        serializer.emit_arguments(
            "initialize_chain_manager_timeout",
            &format_args!("{:?}", self.initialize_chain_manager_timeout),
        )?;
        serializer.emit_arguments(
            "enable_testchain",
            &format_args!("{:?}", self.enable_testchain),
        )?;
        serializer.emit_arguments("tokio_threads", &format_args!("{:?}", self.tokio_threads))?;
        serializer.emit_arguments("riker_threads", &format_args!("{:?}", self.riker_threads))?;
        serializer.emit_arguments(
            "validate_cfg_identity_and_stop",
            &format_args!("{:?}", self.validate_cfg_identity_and_stop),
        )?;
        serializer.emit_arguments(
            "tezos_network_config",
            &format_args!("{:?}", self.tezos_network_config),
        )?;
        serializer.emit_arguments("tezos_network", &format_args!("{:?}", self.tezos_network))
    }
}

pub fn parse_snapshot_block(block: &Option<String>) -> Option<BlockReference> {
    block.as_ref().map(|b| {
        if let Ok(block_hash) = b.parse::<BlockHash>() {
            return BlockReference::BlockHash(block_hash);
        }

        // ~num is an offset from HEAD
        if let Some(maybe_num) = b.strip_prefix('~') {
            if let Ok(offset) = maybe_num.parse::<u32>() {
                return BlockReference::OffsetFromHead(offset);
            }
        }

        if let Ok(level) = b.parse::<u32>() {
            return BlockReference::Level(level);
        }

        panic!(
            "Provided value cannot be converted to BlockHash, level of offset from head"
        );
    })
}

fn resolve_tezos_network_config(
    tezos_network: &TezosEnvironment,
    custom_network_file: Option<PathBuf>,
) -> (TezosEnvironment, TezosEnvironmentConfiguration) {
    if matches!(tezos_network, TezosEnvironment::Custom) {
        // If a custom network file has been provided, parse it and set the custom network
        if let Some(custom_network_file) = custom_network_file {
            (
                *tezos_network,
                TezosEnvironmentConfiguration::try_from_config_file(custom_network_file)
                    .expect("Failed to parse tezos network configuration"),
            )
        } else {
            panic!("Missing `--custom-network-file` argument with custom network configuration for selected network `{:?}`", tezos_network)
        }
    } else {
        // check in defaults
        if let Some(tezos_network_config) = environment::default_networks().get(tezos_network) {
            (*tezos_network, tezos_network_config.clone())
        } else {
            panic!(
                "Missing default configuration for selected network `{:?}`",
                tezos_network
            )
        }
    }
}

// Returns final path. In case:
//      1. path is relative -> final_path = tezos_data_dir / path
//      2. path is absolute -> final_path = path
pub fn get_final_path(tezos_data_dir: &Path, path: PathBuf) -> PathBuf {
    let mut final_path: PathBuf;

    // path is absolute or relative to the current dir -> start with ./ or ../
    if path.is_absolute() || path.starts_with(".") {
        final_path = path
    }
    // otherwise path is relative to the tezos-data-dir
    else {
        final_path = tezos_data_dir.to_path_buf();
        final_path.push(path);
    }

    // Tries to create final_path parent dir, if non-existing
    if let Some(parent_dir) = final_path.parent() {
        if !parent_dir.exists() {
            if let Err(e) = fs::create_dir_all(parent_dir) {
                panic!("Unable to create required dir '{:?}': {} ", parent_dir, e);
            }
        }
    }

    final_path
}

// Parses config file and returns vector of OsString representing all argument strings from file
// All lines that are empty or begin with "#" or "//" are ignored
pub fn parse_config(config_path: PathBuf) -> Vec<OsString> {
    let file = fs::File::open(&config_path)
        .unwrap_or_else(|_| panic!("Unable to open config file at: {:?}", config_path));
    let reader = io::BufReader::new(file);

    let mut args: Vec<OsString> = vec![];

    let mut line_num = 0;
    for line_result in reader.lines() {
        let mut line = line_result.unwrap_or_else(|_| {
            panic!(
                "Unable to read line: {:?} from config file at: {:?}",
                line_num, config_path
            )
        });
        line = line.trim().to_string();

        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }

        args.push(OsString::from(line));
        line_num += 1;
    }

    args
}

impl Environment {
    pub fn from_args() -> Self {
        let mut cli_args = CliArgs::parse();

        // TODO: implement this with the new arg parsing
        // First, get cli arguments and find out only if config-file arg is provided
        // If config-file argument is present, read all parameters from config-file and merge it with cli arguments
        let temp_args = cli_args.clone();
        if let Some(config_file) = temp_args.config_file {

            let mut merged_args = parse_config(config_file);

            let mut cli_args_os = env::args_os();
            if let Some(bin) = cli_args_os.next() {
                merged_args.insert(0, bin);
            }
            merged_args.extend(cli_args_os);

            // args = cli_args.get_matches_from(merged_args);
            cli_args = CliArgs::parse_from(merged_args);
        }
        // Otherwise use only cli arguments that are already parsed
        else {
            cli_args = temp_args;
        }

        // Replay corner case
        if let Some(Commands::Replay { target_path, .. }) = &cli_args.command {
            let options = fs_extra::dir::CopyOptions {
                content_only: true,
                overwrite: true,
                ..fs_extra::dir::CopyOptions::default()
            };
    
            fs_extra::dir::copy(cli_args.tezos_data_dir.as_path(), target_path.as_path(), &options).unwrap();
            
            let new_path = target_path.clone();
            cli_args.set_tezos_data_dir(new_path);
        }

        let (tezos_network, tezos_network_config): (
            TezosEnvironment,
            TezosEnvironmentConfiguration,
        ) = resolve_tezos_network_config(&cli_args.network, cli_args.custom_network_file.clone());

        let new_tezos_data_dir = cli_args.command.clone().map(|cmd|{
            if let Commands::Replay { target_path, .. } = cmd {
                let options = fs_extra::dir::CopyOptions {
                    content_only: true,
                    overwrite: true,
                    ..fs_extra::dir::CopyOptions::default()
                };
    
                fs_extra::dir::copy(&cli_args.tezos_data_dir, &target_path, &options).unwrap();
                Some(target_path.clone())
            } else {
                None
            }
        });

        if let Some(Some(new_data_dir)) = new_tezos_data_dir {
            cli_args.set_tezos_data_dir(new_data_dir);
        }

        let log_targets: HashSet<String> = cli_args.log.into_iter().collect();

        let loggers = log_targets
            .iter()
            .map(|name| match name.as_str() {
                "terminal" => LoggerType::TerminalLogger,
                "file" => {
                    LoggerType::FileLogger(FileLoggerConfig::new(
                        get_final_path(&cli_args.tezos_data_dir, cli_args.log_file.clone()),
                        cli_args.log_rotate_if_size_in_bytes,
                        cli_args.log_rotate_keep_logs_number,
                    ))
                }
                unknown_logger_type => {
                    panic!(
                        "Unknown log target {} - supported are: {:?}",
                        unknown_logger_type,
                        LoggerType::possible_values()
                    )
                }
            })
            .collect();

        // Validate that protocol runner binary is correct before starting
        if !cli_args.protocol_runner.exists() {
            panic!(
                "Tezos protocol runner executable not found at '{}'",
                cli_args.protocol_runner.to_string_lossy(),
            )
        }

        let bootstrap_addresses = if cli_args.bootstrap_lookup_address.is_empty() && cli_args.peers.is_empty() && !cli_args.private_node {
            tezos_network_config.bootstrap_lookup_addresses.clone()
        } else {
            cli_args.bootstrap_lookup_address.clone()
        };

        let tezos_data_dir = cli_args.tezos_data_dir;

        Environment {
            p2p: crate::configuration::P2p {
                listener_port: cli_args.p2p_port,
                listener_address: format!("0.0.0.0:{}", cli_args.p2p_port)
                    .parse::<SocketAddr>()
                    .expect("Failed to parse listener address"),
                disable_bootstrap_lookup: cli_args.disable_bootstrap_lookup,
                disable_peer_graylist: cli_args.disable_peer_graylist,
                bootstrap_lookup_addresses: bootstrap_addresses
                    .iter()
                    .map(|addr| {
                        environment::parse_bootstrap_addr_port(addr, crate::configuration::P2p::DEFAULT_P2P_PORT_FOR_LOOKUP)
                        .unwrap_or_else(|_| {
                                        panic!(
                                            "Was expecting 'ADDR' or 'ADDR:PORT', invalid value: {}",
                                            addr
                                        )
                                    })
                    }).collect(),
                bootstrap_peers: cli_args.peers,
                peer_threshold: PeerConnectionThreshold::try_new(
                    cli_args.peer_thresh_low,
                    cli_args.peer_thresh_high,
                    cli_args.synchronization_thresh,
                )
                .expect("Invalid threashold range"),
                private_node: cli_args.private_node,
                disable_mempool: cli_args.disable_mempool,
                disable_block_precheck: cli_args.disable_block_precheck,
                disable_endorsements_precheck: cli_args.disable_endorsements_precheck,
                randomness_seed: None,
                record_shell_automaton_state_snapshots: cli_args.record_shell_automaton_state_snapshots,
                record_shell_automaton_actions: cli_args.record_shell_automaton_actions,
                current_head_level_override: cli_args.current_head_level_override,
            },
            rpc: crate::configuration::Rpc {
                listener_port: cli_args.rpc_port,
                websocket_cfg: cli_args.websocket_address.map(|address| {
                    let max_connections = cli_args.websocket_max_connections;
                    (address, max_connections)
                }),
            },
            logging: crate::configuration::Logging {
                slog: SlogConfig {
                    level: cli_args.log_level.into(),
                    format: cli_args.log_format.into(),
                    log: loggers,
                },
                ocaml_log_enabled: cli_args.ocaml_log_enabled,
            },
            storage: {
                let db_path = get_final_path(&tezos_data_dir, cli_args.bootstrap_db_path);

                let context_stats_db_path = cli_args.context_stats_db_path.map(|path| {
                    get_final_path(&tezos_data_dir, path)
                });

                let db_threads_count = cli_args.db_cfg_max_threads.map(|value| {
                    std::cmp::min(
                        Storage::MINIMAL_THREAD_COUNT,
                        value / Storage::STORAGES_COUNT,
                    )
                });

                let db = RocksDbConfig {
                    cache_size: Storage::LRU_CACHE_SIZE_96MB,
                    expected_db_version: Storage::DB_STORAGE_VERSION,
                    db_path: db_path.join("db"),
                    columns: DbsRocksDbTableInitializer,
                    threads: db_threads_count,
                };

                let context_kv_store = match cli_args.context_kv_store {
                    SupportedContextKeyValueStore::InMem => ContextKvStoreConfiguration::InMem(
                        TezosContextTezedgeOnDiskBackendOptions {
                            base_path: get_final_path(&tezos_data_dir, "context".into())
                                .into_os_string()
                                .into_string()
                                .unwrap(),
                            startup_check: cli_args.context_integrity_check,
                        },
                    ),
                    SupportedContextKeyValueStore::OnDisk => {
                        ContextKvStoreConfiguration::OnDisk(
                            TezosContextTezedgeOnDiskBackendOptions {
                                base_path: get_final_path(&tezos_data_dir, "context".into())
                                    .into_os_string()
                                    .into_string()
                                    .unwrap(),
                                startup_check: cli_args.context_integrity_check,
                            },
                        )
                    }
                };

                // TODO - TE-261: can this conversion be made prettier without `to_string_lossy`?
                // Path for the socket that will be used for IPC access to the context
                let context_ipc_socket_path =
                    async_ipc::temp_sock().to_string_lossy().as_ref().to_owned();

                let context_storage_configuration = match cli_args.tezos_context_storage {
                    TezosContextStorageChoice::TezEdge => {
                        TezosContextStorageConfiguration::TezEdgeOnly(
                            TezosContextTezEdgeStorageConfiguration {
                                backend: context_kv_store,
                                ipc_socket_path: Some(context_ipc_socket_path),
                            },
                        )
                    }
                    TezosContextStorageChoice::Irmin => {
                        TezosContextStorageConfiguration::IrminOnly(
                            TezosContextIrminStorageConfiguration {
                                data_dir: tezos_data_dir
                                    .to_str()
                                    .expect("Invalid tezos_data_dir value")
                                    .to_string(),
                            },
                        )
                    }
                    TezosContextStorageChoice::Both => TezosContextStorageConfiguration::Both(
                        TezosContextIrminStorageConfiguration {
                            data_dir: tezos_data_dir
                                .to_str()
                                .expect("Invalid tezos_data_dir value")
                                .to_string(),
                        },
                        TezosContextTezEdgeStorageConfiguration {
                            backend: context_kv_store,
                            ipc_socket_path: Some(context_ipc_socket_path),
                        },
                    ),
                };

                crate::configuration::Storage {
                    db,
                    context_storage_configuration,
                    main_db: cli_args.maindb_backend,
                    db_path,
                    context_stats_db_path,
                    compute_context_action_tree_hashes: cli_args.compute_context_action_tree_hashes,
                    patch_context: {
                        match cli_args.sandbox_patch_context_json_file {
                            Some(path) => {
                                let path = get_final_path(&tezos_data_dir, path);
                                match fs::read_to_string(&path) {
                                    Ok(content) => {
                                        // validate valid json
                                        if let Err(e) = serde_json::from_str::<
                                            HashMap<String, serde_json::Value>,
                                        >(
                                            &content
                                        ) {
                                            panic!(
                                                "Invalid json file: {}, reason: {}",
                                                path.as_path().display(),
                                                e
                                            );
                                        }
                                        Some(PatchContext {
                                            key: "sandbox_parameter".to_string(),
                                            json: content,
                                        })
                                    }
                                    Err(e) => panic!("Cannot read file, reason: {}", e),
                                }
                            }
                            None => {
                                // check default configuration, if any
                                tezos_network_config
                                    .patch_context_genesis_parameters
                                    .clone()
                            }
                        }
                    },
                    initialize_context_timeout: std::time::Duration::from_secs(
                        cli_args.initialize_context_timeout_in_secs,
                    ),
                }
            },
            identity: crate::configuration::Identity {
                identity_json_file_path: {
                    get_final_path(&tezos_data_dir, cli_args.identity_file)
                },
                expected_pow: cli_args.identity_expected_pow,
            },
            ffi: Ffi {
                protocol_runner: cli_args.protocol_runner,
                zcash_param: ZcashParams {
                    init_sapling_spend_params_file: cli_args.init_sapling_spend_params_file,
                    init_sapling_output_params_file: cli_args.init_sapling_output_params_file,
                },
            },
            tokio_threads: cli_args.tokio_threads,
            riker_threads: cli_args.riker_threads,
            tezos_network,
            tezos_network_config,
            enable_testchain: cli_args.enable_testchain,
            validate_cfg_identity_and_stop: cli_args.validate_cfg_identity_and_stop,
            initialize_chain_manager_timeout: std::time::Duration::from_secs(
                cli_args.initialize_chain_manager_timeout_in_secs,
            ),
            sub_command: cli_args.command
        }
    }

    pub fn create_logger(&self) -> Result<Logger, NoDrainError> {
        self.logging.slog.create_logger()
    }
}
