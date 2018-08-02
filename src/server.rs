use bytes::BytesMut;
use failure::{Error, SyncFailure};
use futures::prelude::*;
use futures::sync::mpsc;
use std::cell::RefCell;
use std::collections::{BTreeSet, BTreeMap};
use std::net::SocketAddr;
use std::io::{Error as io_Error, ErrorKind as io_ErrorKind};
use std::rc::Rc;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::{Core, Handle};
use tokio_io::AsyncRead;
use tokio_io::io::{self, ReadHalf};
use telegram_bot::*;

/// A running Televery server.
pub struct Server {
    /// Tokio Core for running Telegram bots and the TCP listener.
    core: Core,

    /// Telegram API interface.
    api: Option<Api>,

    /// Listener for verification requests.
    listener: Option<TcpListener>,

    /// Trusted apps.
    trusted_apps: BTreeSet<String>,

    /// Trusted Telegram usernames.
    trusted_users: BTreeSet<String>,

    /// Server state.
    state: Rc<State>,
}

/// Server states.
struct State {
    /// Mapping from usernames to chat ID.
    user_chatid: RefCell<BTreeMap<String, ChatId>>,
}

impl Server {
    pub fn new(trusted_apps: BTreeSet<String>, trusted_users: BTreeSet<String>)
               -> Result<Server, Error>
    {
        let core = Core::new()?;

        Ok(Server {
            core,
            api: None,
            listener: None,
            trusted_apps,
            trusted_users,
            state: Rc::new(State {
                user_chatid: RefCell::new(BTreeMap::new()),
            }),
        })
    }

    /// Bind a local address to listen for verification requests, and
    /// configure Telegram bot API.
    pub fn bind(&mut self, token: impl AsRef<str>, addr: SocketAddr)
                -> Result<(), Error>
    {
        let core = &self.core;
        let api = Some(Api::configure(token).build(core.handle())
                       .map_err(SyncFailure::new)?);
        let listener = Some(TcpListener::bind(&addr, &core.handle())?);

        self.api = api;
        self.listener = listener;
        Ok(())
    }

    /// Run the server and consume the data structure. This function
    /// will make the server listen for incoming verification requests
    /// and Telegram updates. It panics if not bound or binding
    /// failed.
    pub fn run(mut self) -> ! {
        let handle = &self.core.handle();
        let api = &self.api.unwrap();
        let trusted_users = &self.trusted_users;
        let state = self.state;

        // channel between sock and bot sides
        let (_sock_tx, bot_rx) = mpsc::unbounded();

        // bot side: communicate with telegram api and socket
        let bot_updates = api.stream().for_each(|update| {
            process_telegram_update(handle, api, update, trusted_users, state.clone());
            Ok(())
        }).map_err(|e| println!("bot updates error: {:?}", e));
        let bot_server = bot_rx.for_each(|message: ServerMessage| {
            match message {
                ServerMessage::SendMessage(ref username) => {
                    let user_chatid = state.user_chatid.borrow();
                    if !user_chatid.contains_key(username) {
                        println!("{} is asked to give permission, but chat id is missing", username);
                    }

                    let chatid = user_chatid.get(username).unwrap();
                    let inline_keyboard = reply_markup!(
                        inline_keyboard,
                        ["Pass" callback "0,0", "Deny" callback "0,1"]
                    );
                    let mut msg = requests::SendMessage::new(chatid, "OK?");
                    let fut = api.send(msg.reply_markup(inline_keyboard))
                        .and_then(|msg| {
                            println!("confirm message id = {}", msg.id);
                            Ok(())
                        })
                        .map_err(|e| println!("error sending message: {:?}", e));
                    handle.spawn(fut);
                }
                _ => unreachable!()
            }
            Ok(())
        });
        let bot = bot_updates.select(bot_server).map(unit).map_err(unit);

        // socket side:
        // impl Stream<Item = (), Error = ()>
        let req = self.listener.unwrap().incoming().for_each(|(stream, addr)| {
            println!("New connection: {}", addr);
            let (reader, writer) = stream.split();
            let (chan_tx, chan_rx) = mpsc::unbounded();

            let socket_reader = process_verification_request(reader, chan_tx);
            let socket_writer = chan_rx.fold(writer, |writer, msg: &str| {
                io::write_all(writer, msg.as_bytes())
                    .map(|(writer, _)| writer)
                    .map_err(|e| println!("in write all: {:?}", e))
            }).map(unit);

            handle.spawn(socket_reader.select(socket_writer).map(unit).map_err(unit));
            Ok(())
        }).map_err(|e| println!("socket error: {:?}", e));

        self.core.run(req.join(bot)).map(unit).unwrap();
        panic!("Unexpected exit from event loop")
    }
}

/// Process Telegram updates.
fn process_telegram_update(handle: &Handle, api: &Api, update: Update,
                           trusted_users: &BTreeSet<String>, state: Rc<State>) {
    match update.kind {
        UpdateKind::CallbackQuery(query) => process_telegram_callback(handle, query),
        UpdateKind::Message(message) => {
            process_telegram_message(api, &message, trusted_users, state.clone());
        },
        _ => (),
    }
}

fn process_telegram_message(api: &Api, message: &Message, trusted_users: &BTreeSet<String>,
                            state: Rc<State>)
{
    use self::MessageKind::*;
    if let Text {..} = message.kind {
        let username = &message.from.username;
        let fut = match username {
            Some(username) => {
                if trusted_users.contains(username) {
                    // store chat id for future use
                    state.user_chatid.borrow_mut().insert(username.clone(), message.chat.id());
                    message.text_reply(
                        format!("Hi, @{}! This chat (ID = {}) will be used for notifications later on.",
                                username, message.chat.id())
                    )
                } else {
                    // unknown username
                    message.text_reply(
                        format!("Sorry, @{}, but you're not authorized to use this bot.",
                                username)
                    )
                }
            }
            None => message.text_reply("You need a username to use this bot.")
        };
        api.spawn(fut);
    }
}

fn process_telegram_callback(_handle: &Handle, query: CallbackQuery) {
    println!("process_telegram_callback: {:#?}", query);
    if query.data == "0,1" {
        println!("admin denied")
    } else {
        println!("admin passed")
    }
}

/// Process verification requests. This function will consume a stream
/// and the corresponding address.
fn process_verification_request(
    stream: ReadHalf<TcpStream>,
    chan_tx: mpsc::UnboundedSender<&'static str>
) -> impl Future<Item = (), Error = ()>
{
    Frames::new(stream)
        .for_each(move |request| {
            println!("{} asks {}", request.appname, request.method);
            let res = match request.method.as_str() {
                "REQ" => VerifyResult::Allow.into(),
                _ => VerifyResult::Deny.into()
            };
            chan_tx.unbounded_send(res).expect("chan_send");
            Ok(())
        })
        .map_err(|e| println!("in process verify: {:?}", e))
}

/// A custom codec for turning stream of bytes into custom requests. I
/// just can't get LineCodec compiling and working...
///
/// Frames takes the ownership of your TcpStream.
#[derive(Debug)]
struct Frames {
    stream: ReadHalf<TcpStream>,
    rd: BytesMut,
}

/// A verification request.
#[derive(Debug, Clone)]
struct VerifyRequest {
    method: String,
    appname: String,
}

/// Possible verification results.
#[derive(Debug, Clone, Copy)]
enum VerifyResult {
    Allow,
    Deny,
}

/// Message type used for channel between the Telegram side and the
/// socket side.
#[derive(Debug, Clone)]
enum ServerMessage {
    // Send a verification message via Telegram bot to the user.  The
    // first argument means the Telegram username.
    SendMessage(String),

    // The user has answered, and forward the result to the socket
    // side.
    Answer(VerifyResult),
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
    fn read_off(&mut self) -> Poll<(), io_Error> {
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
    type Error = io_Error;

    fn poll(&mut self) -> Poll<Option<VerifyRequest>, io_Error> {
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
                    return Err(io_Error::new(io_ErrorKind::InvalidInput, "request is not utf8"))
                }
                Ok(line) => {
                    println!("Received line: {}", line);
                    let mut parts = line.split(' ');
                    // check length
                    // fixme: magic number
                    if parts.clone().count() != 2 {
                        return Err(io_Error::new(io_ErrorKind::InvalidInput, "invalid length of request"))
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

impl From<VerifyResult> for &'static str {
    fn from(response: VerifyResult) -> &'static str {
        match response {
            VerifyResult::Allow => "ALLOW\n",
            VerifyResult::Deny => "DENY\n",
        }
    }
}

#[doc(hidden)]
fn unit<T>(_: T) { }
