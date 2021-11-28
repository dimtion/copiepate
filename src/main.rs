use std::{io::Read, process::exit};

use clap::{App, Arg, ArgMatches};
use clipboard::{ClipboardContext, ClipboardProvider};
use log::error;

const DEFAULT_ADDRESS: &str = "127.0.0.1";
const DEFAULT_PORT: &str = "2323";

fn get_address(matches: &ArgMatches) -> String {
    let address = matches.value_of("address").unwrap();
    let port = matches.value_of("port").unwrap();

    format!("{}:{}", address, port)
}

fn get_log_level(matches: &ArgMatches) -> log::Level {
    match matches.occurrences_of("verbosity") {
        0 => log::Level::Info,
        1 => log::Level::Debug,
        _ => log::Level::Trace,
    }
}

fn init_args<'a>() -> ArgMatches<'a> {
    App::new("copiepate")
        .version("0.1.0")
        .author("Loïc C. <loic.carr@gmail.com>")
        .about("Send a paste event from a client over the network to a server")
        .arg(
            Arg::with_name("server")
                .short("s")
                .long("server")
                .help("Starts server daemon"),
        )
        .arg(
            Arg::with_name("address")
                .short("a")
                .long("address")
                .takes_value(true)
                .default_value(DEFAULT_ADDRESS)
                .help("Server ip address"),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .takes_value(true)
                .default_value(DEFAULT_PORT)
                .help("Server port"),
        )
        .arg(
            Arg::with_name("verbosity")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .get_matches()
}

fn main() {
    let matches = init_args();
    simple_logger::init_with_level(get_log_level(&matches)).unwrap();
    let address = get_address(&matches);

    if matches.is_present("server") {
        let mut clipboard_ctx = ClipboardProvider::new().unwrap();
        let mut server = copiepate::server::Server::<ClipboardContext> {
            address: &address,
            clipboard_ctx: &mut clipboard_ctx,
        };
        match server.start() {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to start server: {}", e);
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
                error!("Failed to send message: {}", e);
                exit(1);
            }
        }
    }
}
