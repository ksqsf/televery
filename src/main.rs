extern crate dotenv;
extern crate futures;
extern crate telegram_bot;
extern crate tokio_core;

#[macro_use] extern crate failure;

mod server;

use std::env;
use failure::Error;
use server::Server;

fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();

    let mut server = Server::new()?;
    server.bind(env::var("TELEVERY_BOT_TOKEN")?, "127.0.0.1:12345".parse()?)?;
    server.run()
}
