use failure::{Error, SyncFailure};
use futures::{Stream, Future};
use std::net::SocketAddr;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::{Core, Handle};
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
                        .map_err(SyncFailure::new)?);
        self.listener = Some(TcpListener::bind(&addr, &self.core.handle())?);
        Ok(())
    }

    /// Run the server and consume the data structure. This function
    /// will make the server listen for incoming verification requests
    /// and Telegram updates. It panics if not bound or binding
    /// failed.
    pub fn run(mut self) -> Result<(), Error> {
        let handle = self.core.handle();
        let api = self.api.unwrap();

        let bot_fut = api.stream().for_each(|update| {
            println!("{:#?}", update);
            process_telegram_confirm(&handle, &api, update);
            Ok(())
        }).map_err(|e| format_err!("{}", e.description()));

        let req_fut = self.listener.unwrap()
            .incoming().for_each(|(stream, addr)| {
                println!("({:#?}, {:#?})", stream, addr);
                process_verification_request(&handle, stream, addr);
                Ok(())
            }).map_err(|e| Error::from(e));

        self.core.run(req_fut.join(bot_fut)).map(|_| ())
    }
}

/// Process Telegram confirm messages.
fn process_telegram_confirm(handle: &Handle, api: &Api, update: Update) {
    if let UpdateKind::Message(message) = update.kind {
        if let MessageKind::Text { ref data, .. } = message.kind {
            let inline_keyboard = reply_markup!(
                inline_keyboard,
                ["Pass" callback "0,0", "Deny" callback "0,1"]
            );
            println!("<{}>: {}", &message.from.first_name, data);

            let mut test = requests::SendMessage::new(message.chat, "this is a test message");
            let fut = api.send(test.reply_markup(inline_keyboard))
                .and_then(|message| {
                    println!("confirm message id = {:?}", message.id);
                    Ok(())
                })
                .map_err(|_| ());
            handle.spawn(fut);
        }
    }
}

/// Process verification requests. This function will consume a stream
/// and the corresponding address.
fn process_verification_request(_handle: &Handle, _stream: TcpStream, _addr: SocketAddr) {
}
