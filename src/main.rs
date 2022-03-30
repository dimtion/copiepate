use std::io::Write;
use std::{io::Read, path::PathBuf, process::exit};

use anyhow::anyhow;
use anyhow::Result;
use clipboard::{ClipboardContext, ClipboardProvider};
use etcetera::base_strategy::{self, BaseStrategy};
use serde_derive::{Deserialize, Serialize};
use simple_logger::SimpleLogger;
use structopt::StructOpt;

// TODO: move opts and opts parsing to a proper module
// TODO: client/server specific configuration
// TODO: json output
// TODO: auto-configuration on startup
// TODO: review error handling (especially server-side)
// TODO: add code coverage

const DEFAULT_ADDRESS: &str = "127.0.0.1";
const DEFAULT_PORT: &str = "2323";

const DEFAULT_CONFIG_DIR: &str = "copiepate";
const DEFAULT_CONFIG_FILENAME: &str = "config.toml";

/// Unsecure key used for unsecure mode. WARNING: using this key is as if the message
/// was sent as plaintext over the network.
pub const DEFAULT_INSECURE_KEY: &[u8; copiepate::KEY_SIZE] = b"_WARNING_UNSECURE_KEY_PLAINTEXT_";

#[derive(Debug, StructOpt, Deserialize, Serialize)]
#[structopt(
    name = "copiepate",
    about = "Send a paste event from a client over the network to a server.",
    version = "0.2.0"
)]
struct Opt {
    #[structopt(
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
        help = "Sets the level of verbosity. Copiepate will log service messages on stderr.
Increase log level to have more information.",
        parse(from_occurrences)
    )]
    verbosity: u64,

    #[structopt(
        short = "-k",
        long = "--insecure",
        help = "Do not encrypt message over the network.
WARNING: anybody might be able to read the messages."
    )]
    insecure: bool,

    #[structopt(
        long = "--secret",
        help = "32 bits base64 encoded secret to use to contact the server.
Must be the same between client and server. If `--insecure` is set, will be discarded"
    )]
    #[serde(skip_serializing_if = "Option::is_none")]
    secret: Option<String>,

    #[structopt(
        long = "--tee",
        help = "[Client only] With `--tee`, copiepate will behave like the tee built-in and redirect the stdin to stdout."
    )]
    tee: bool,

    #[structopt(
        long = "--exec",
        help = "[Server only] Shell to command to execute when receiving a new message.
Received message will be passed as stdin to the invoked command."
    )]
    #[serde(skip_serializing_if = "Option::is_none")]
    exec: Option<String>,
}

fn get_address(opt: &Opt) -> Result<String> {
    Ok(format!(
        "{}:{}",
        opt.address
            .as_ref()
            .ok_or_else(|| anyhow!("Missing address"))?,
        opt.port.as_ref().ok_or_else(|| anyhow!("Missing port"))?,
    ))
}

fn get_key(opt: &Opt) -> Result<Vec<u8>> {
    let secret = if opt.insecure {
        Ok(DEFAULT_INSECURE_KEY.to_vec())
    } else {
        match &opt.secret {
            None => Err(anyhow!("No secret provided.")),
            Some(k) => Ok(base64::decode(k.clone())?),
        }
    };

    secret.and_then(|s| match s.len() {
        copiepate::KEY_SIZE => Ok(s),
        _ => Err(anyhow!(
            "Decoded secret must have a length of {} bytes.",
            copiepate::KEY_SIZE
        )),
    })
}

fn get_log_level(verbosity: u64) -> log::LevelFilter {
    match verbosity {
        0 => log::LevelFilter::Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    }
}

fn load_config(opt: &Opt) -> Result<Opt> {
    match &opt.config_file {
        None => (),
        Some(filename) => {
            if !filename.exists() {
                return Err(anyhow!("Configuration file {:?} does not exist.", filename));
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
                Err(e) => log::warn!("Failed to load configuration file: {}", e),
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

fn tee(message: &[u8]) -> Result<()> {
    let mut stdout = std::io::stdout();
    stdout.write_all(message)?;
    stdout.flush()?;
    Ok(())
}

fn main() {
    let opt = Opt::from_args();
    create_logger(&opt);

    let config = match load_config(&opt) {
        Ok(c) => c,
        Err(e) => {
            log::error!(
                "Failed to load configuration.
Error: {}",
                e
            );
            exit(1);
        }
    };
    log::trace!("Configuration: {:#?}", &config);

    let address = get_address(&config).expect("Failed to load server address");
    let key = match get_key(&config) {
        Ok(k) => k,
        Err(e) => {
            log::error!(
                "Failed to load secret.

You must specify a valid Base64 secret, either in the configuration file or by passing the --secret option.
More information: https://github.com/dimtion/copiepate#setup-and-installation

Error: {} ", e);
            exit(1);
        }
    };

    if config.server_mode {
        let mut clipboard_ctx =
            ClipboardProvider::new().expect("Failed to load clipboard provider");
        let mut server = copiepate::server::ServerBuilder::<ClipboardContext>::default()
            .address(&address)
            .clipboard_ctx(&mut clipboard_ctx)
            .key(&key)
            .exec_command(config.exec)
            .build()
            .expect("Failed setting up copiepate server");
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

        let mut client = copiepate::client::Client::new(&address, &key);

        if config.tee {
            tee(&message).expect("Failed to write to stdout");
            // Empty stderr line to have a separation between tee-ed message and service message
            eprintln!();
        }

        match client.send(&message) {
            Ok(_) => {
                log::info!("Message sent successfully");
            }
            Err(e) => {
                log::error!("Failed to send message: {}", e);
                exit(1);
            }
        }
    }
}
