extern crate bytes;
extern crate dotenv;
extern crate failure;
extern crate tokio_core;
extern crate tokio_io;

#[macro_use]
extern crate futures;
#[macro_use]
extern crate telegram_bot;

mod server;

use failure::Error;
use server::Server;
use std::collections::BTreeSet;
use std::env;

fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();

    let mut trusted_users = BTreeSet::new();
    let mut trusted_apps = BTreeSet::new();

    trusted_users.insert("ksqsf".to_string());
    trusted_apps.insert("nc".to_string());

    let mut srv = Server::new(trusted_apps, trusted_users).unwrap();
    srv.bind(env::var("TELEVERY_BOT_TOKEN")?, "127.0.0.1:12345".parse()?)
        .unwrap();
    srv.run()
}
