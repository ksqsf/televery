extern crate bytes;
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

use failure::Error;
use server::Server;
use std::collections::BTreeSet;
use std::env;
use std::fs::File;
use std::path::Path;

fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();
    Config::from_file("config.yml").and_then(|cfg| start_server(cfg))
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
