extern crate bytes;
extern crate clap;
extern crate dotenv;
extern crate failure;
extern crate serde_yaml;
extern crate tokio_core;
extern crate tokio_io;

#[macro_use]
extern crate futures;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate telegram_bot;

mod server;

use clap::{App, Arg};
use failure::Error;
use server::Server;
use std::collections::BTreeSet;
use std::env;
use std::fs::File;
use std::path::Path;

fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();

    let matches = App::new("Televery")
        .about("Hassle-free two-step verification")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Specify the config file")
                .default_value("config.yml")
                .takes_value(true),
        )
        .get_matches();

    if matches.occurrences_of("config") == 0 {
        eprintln!("WARNING: --config not specified, 'config.yml' is used");
    }

    Config::from_file(matches.value_of("config").unwrap()).and_then(|cfg| start_server(cfg))
}

fn start_server(config: Config) -> Result<(), Error> {
    let trusted_apps = config.trusted_apps;
    let trusted_users = config.trusted_users;
    let mut srv = Server::new(trusted_apps, trusted_users)?;
    srv.bind(env::var("TELEVERY_BOT_TOKEN")?, "127.0.0.1:12345".parse()?)?;
    srv.run()
}

#[derive(Deserialize)]
struct Config {
    trusted_users: BTreeSet<String>,
    trusted_apps: BTreeSet<String>,
}

impl Config {
    fn from_file(path: impl AsRef<Path>) -> Result<Config, Error> {
        File::open(path)
            .map_err(|e| Error::from(e))
            .and_then(|f| serde_yaml::from_reader(f).map_err(|e| Error::from(e)))
    }
}
