use bytes::BytesMut;
use failure::{Error, SyncFailure};
use futures::*;
use futures::sync::mpsc;
use std::net::SocketAddr;
use std::io;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::{Core, Handle};
use tokio_io::AsyncRead;
use tokio_io::io::ReadHalf;
use telegram_bot::*;

/// A running Televery server.
pub struct Server {
    /// Tokio Core for running Telegram bots and the TCP listener.
    core: Core,

    /// Telegram API interface.
    api: Option<Api>,

    /// Listener for verification requests.
    listener: Option<TcpListener>,
}

impl Server {
    /// Bind a local address to listen for verification requests, and
    /// configure Telegram bot API.
    pub fn bind(token: impl AsRef<str>, addr: SocketAddr)
                -> Result<Server, Error>
    {
        let core = Core::new()?;
        let api = Some(Api::configure(token).build(core.handle())
                       .map_err(SyncFailure::new)?);
        let listener = Some(TcpListener::bind(&addr, &core.handle())?);

        Ok(Server {
            core,
            api,
            listener,
        })
    }

    /// Run the server and consume the data structure. This function
    /// will make the server listen for incoming verification requests
    /// and Telegram updates. It panics if not bound or binding
    /// failed.
    pub fn run(mut self) -> Result<(), Error> {
        let handle = self.core.handle();
        let api = self.api.unwrap();

        let bot = api.stream().for_each(|update| {
            println!("{:#?}", update);
            process_telegram_confirm(&handle, &api, update);
            Ok(())
        }).map_err(|e| format_err!("{}", e.description()));

        let req = self.listener.unwrap() .incoming().for_each(|(stream, addr)| {
            println!("New connection: {}", addr);
            let (reader, writer) = stream.split();
            let (tx, rx) = mpsc::unbounded();

            let socket_reader = process_verification_request(reader, tx);
            let socket_writer = rx.fold(writer, |writer, msg: &str| {
                ::tokio_io::io::write_all(writer, msg.as_bytes())
                    .map(|(writer, _)| writer)
                    .map_err(|e| println!("in write all: {:?}", e))
            }).map(unit);

            handle.spawn(socket_reader.select(socket_writer).map(unit).map_err(unit));
            Ok(())
        }).map_err(|e| Error::from(e));

        self.core.run(req.join(bot)).map(unit)
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
                .map_err(unit);

            handle.spawn(fut);
        }
    }
}

/// Process verification requests. This function will consume a stream
/// and the corresponding address.
fn process_verification_request(
    stream: ReadHalf<TcpStream>,
    tx: mpsc::UnboundedSender<&'static str>
) -> impl Future<Item = (), Error = ()>
{
    Frames::new(stream)
        .for_each(move |request| {
            println!("{} asks {}", request.appname, request.method);
            tx.unbounded_send(VerifyResponse::Deny.into()).expect("chan_send");
            Ok(())
        })
        .map_err(|e| println!("in process verify: {:?}", e))
}

/// A custom codec for turning stream of bytes into custom requests. I
/// just can't get LineCodec compiling and working...
///
/// Frames takes the ownership of your TcpStream.
struct Frames {
    stream: ReadHalf<TcpStream>,
    rd: BytesMut,
}

/// A verification request.
struct VerifyRequest {
    method: String,
    appname: String,
}

/// Possible verification responses.
enum VerifyResponse {
    Allow,
    Deny,
}

impl Frames {
    fn new(stream: ReadHalf<TcpStream>) -> Self {
        Frames {
            stream,
            rd: BytesMut::new(),
        }
    }

    /// Read into the buffer whatever has been read by the system so
    /// far.
    fn read_off(&mut self) -> Poll<(), io::Error> {
        loop {
            self.rd.reserve(1024);
            let n = try_ready!(self.stream.read_buf(&mut self.rd));
            if n == 0 {
                return Ok(Async::Ready(()))
            }
        }
    }
}

impl Stream for Frames {
    type Item = VerifyRequest;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<VerifyRequest>, io::Error> {
        let sock_closed = self.read_off()?.is_ready();
        let pos = self.rd.iter().position(|byte| *byte == b'\n');
        if let Some(pos) = pos {
            let mut line = self.rd.split_to(pos+1);
            line.split_off(pos);

            // convert to &str and parse it
            let line = ::std::str::from_utf8(line.as_ref());
            match line {
                Err(_) => {
                    // if request is invalid, close the connection
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "request is not utf8"))
                }
                Ok(line) => {
                    println!("Received line: {}", line);
                    let mut parts = line.split(' ');
                    // check length
                    // fixme: magic number
                    if parts.clone().count() != 2 {
                        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid length of request"))
                    }
                    // check method
                    let (method, appname) = (parts.next(), parts.next());
                    println!("Parsed: {:?} {:?}", method, appname);
                    match (method, appname) {
                        (None, _) | (_, None) => return Ok(Async::NotReady),
                        (Some(method), Some(appname)) => {
                            return Ok(Async::Ready(Some(VerifyRequest {
                                method: method.to_string(),
                                appname: appname.to_string(),
                            })))
                        }
                    }
                }
            }
        }

        if sock_closed {
            Ok(Async::Ready(None))
        } else {
            Ok(Async::NotReady)
        }
    }
}

impl From<VerifyResponse> for &'static str {
    fn from(response: VerifyResponse) -> &'static str {
        match response {
            VerifyResponse::Allow => "ALLOW\n",
            VerifyResponse::Deny => "DENY\n",
        }
    }
}

#[doc(hidden)]
fn unit<T>(_: T) { }
