extern crate bytes;
extern crate dotenv;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;

#[macro_use] extern crate failure;
#[macro_use] extern crate telegram_bot;

mod server;

use std::env;
use failure::Error;
use server::Server;

fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();

    Server::bind(env::var("TELEVERY_BOT_TOKEN")?,
                 "127.0.0.1:12345".parse()?)
        .and_then(|server| server.run())
}
