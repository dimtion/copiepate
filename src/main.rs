use std::{io::Read, path::PathBuf, process::exit};

use clipboard::{ClipboardContext, ClipboardProvider};
use etcetera::base_strategy::{self, BaseStrategy};
use serde_derive::{Deserialize, Serialize};
use simple_logger::SimpleLogger;
use std::error::Error;
use structopt::StructOpt;

// TODO: move opts and opts parsing to a proper module
// TODO: add encryption configuration in config file
// TODO: add option to execute another command
// TODO: find a nice solution to avoid "eating std in" when necessary (network
//       issue, paste-bin lost on server side etc...
// TODO: notification/alert on server side when received event?
// TODO: client/server specific configuration

const DEFAULT_ADDRESS: &str = "127.0.0.1";
const DEFAULT_PORT: &str = "2323";

const DEFAULT_CONFIG_DIR: &str = "copiepate";
const DEFAULT_CONFIG_FILENAME: &str = "config.toml";

#[derive(Debug, StructOpt, Deserialize, Serialize)]
#[structopt(
    name = "copiepate",
    about = "Send a paste event from a client over the network to a server.",
    version = "0.2.0"
)]
struct Opt {
    #[structopt(
        short,
        long = "config",
        help = "Configuration file. Default configuration location depends on OS.
~/.config/copiepate/config.toml for XDG-compatible OSes.",
        parse(from_os_str)
    )]
    #[serde(skip_serializing_if = "Option::is_none")]
    config_file: Option<PathBuf>,

    #[structopt(
        short = "s",
        long = "server",
        help = "Start copiepate server that listen for copy events."
    )]
    server_mode: bool,

    #[structopt(
        short = "a",
        long = "address",
        help = "Server ip address in client mode, or server bind address in server mode."
    )]
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,

    #[structopt(short = "p", long = "port", help = "Server listen port.")]
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<String>,

    #[structopt(
        short = "v",
        long = "verbosity",
        help = "Sets the level of verbosity",
        parse(from_occurrences)
    )]
    verbosity: u64,
}

fn get_address(opt: &Opt) -> Result<String, ()> {
    Ok(format!(
        "{}:{}",
        opt.address.as_ref().ok_or(())?,
        opt.port.as_ref().ok_or(())?
    ))
}

fn get_log_level(verbosity: u64) -> log::LevelFilter {
    match verbosity {
        0 => log::LevelFilter::Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    }
}

fn load_config(opt: &Opt) -> Result<Opt, Box<dyn Error>> {
    match &opt.config_file {
        None => (),
        Some(filename) => {
            if !filename.exists() {
                panic!("Configuration file {:?} does not exist.", filename);
            }
        }
    }

    let config_filename = opt.config_file.clone().unwrap_or({
        let strategy = base_strategy::choose_base_strategy()?;
        strategy
            .config_dir()
            .join(DEFAULT_CONFIG_DIR)
            .join(DEFAULT_CONFIG_FILENAME)
    });

    let mut settings = config::Config::default();

    settings
        .set_default("config_file", config_filename.to_string_lossy().to_string())?
        .set_default("server_mode", false)?
        .set_default("address", DEFAULT_ADDRESS)?
        .set_default("port", DEFAULT_PORT)?;

    log::info!(target: "server", "Loading configuration file: {:?}", &config_filename);
    config_filename
        .exists()
        .then(|| {
            match settings.merge(config::File::from(config_filename.as_path())) {
                Ok(_) => (),
                Err(e) => log::warn!("Error loading config file: {}", e),
            };
        })
        .unwrap_or_else(|| {
            log::warn!(target: "server", "No configuration file. Using default values.");
        });

    settings.merge(config::Config::try_from(opt)?)?;

    Ok(settings.try_into()?)
}

fn create_logger(opt: &Opt) {
    let mut logger = SimpleLogger::new()
        .with_colors(true)
        .with_level(get_log_level(opt.verbosity));

    if opt.server_mode {
        logger = logger.with_module_level("client", log::LevelFilter::Off);
    } else {
        logger = logger.with_module_level("server", log::LevelFilter::Off);
    }

    logger.init().unwrap();
}

fn main() {
    let opt = Opt::from_args();
    create_logger(&opt);
    let config = load_config(&opt).expect("Unable to load configuration");
    log::trace!("Configuration: {:#?}", &config);

    let address = get_address(&config).expect("Failed to load server address");

    if config.server_mode {
        let mut clipboard_ctx =
            ClipboardProvider::new().expect("Unable to load clipboard provider");
        let mut server = copiepate::server::Server::<ClipboardContext> {
            address: &address,
            clipboard_ctx: &mut clipboard_ctx,
        };
        match server.start() {
            Ok(_) => (),
            Err(e) => {
                log::error!("Failed to start server: {}", e);
                exit(1);
            }
        }
    } else {
        let mut message = Vec::new();
        let mut stdin = std::io::stdin();
        stdin.read_to_end(&mut message).unwrap();

        let client = copiepate::client::Client { address: &address };

        match client.send_message(&message) {
            Ok(_) => (),
            Err(e) => {
                log::error!("Failed to send message: {}", e);
                exit(1);
            }
        }
    }
}
