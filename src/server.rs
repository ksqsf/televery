use failure::Error;
use futures::{Stream, Future};
use std::net::SocketAddr;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Core;
use telegram_bot::*;

pub struct Server {
    core: Core,
    api: Option<Api>,
    listener: Option<TcpListener>,
}

impl Server {
    /// Create a new Televery server. Call `bind` to bind a local
    /// address and configure Telegram bot API.
    pub fn new() -> Result<Server, Error> {
        let core = Core::new()?;
        Ok(Server {
            core,
            api: None,
            listener: None,
        })
    }

    /// Bind a local address to listen for verification requests, and
    /// configure Telegram bot API.
    pub fn bind(&mut self, token: impl AsRef<str>, addr: SocketAddr)
                -> Result<(), Error>
    {
        self.api = Some(Api::configure(token).build(self.core.handle())
                        .map_err(|e| format_err!("{}", e.description()))?);
        self.listener = Some(TcpListener::bind(&addr, &self.core.handle())?);
        Ok(())
    }

    /// Run the server. This function will make the server listen for
    /// incoming verification requests and Telegram updates.
    pub fn run(mut self) -> Result<(), Error> {
        let bot_fut = self.api.unwrap().stream().for_each(|update| {
            println!("{:#?}", update);
            process_telegram_confirm(update);
            Ok(())
        }).map_err(|e| format_err!("{}", e.description()));

        let req_fut = self.listener.unwrap()
            .incoming().for_each(|(stream, addr)| {
                println!("({:#?}, {:#?})", stream, addr);
                process_local_request(stream, addr);
                Ok(())
            }).map_err(|e| Error::from(e));

        self.core.run(req_fut.join(bot_fut)).map(|_| ())
    }
}

fn process_telegram_confirm(_update: Update) {
}

fn process_local_request(_stream: TcpStream, _addr: SocketAddr) {
}
