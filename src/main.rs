use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::fmt;
use std::result;
use std::thread;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::Arc;
use std::ops::Deref;
use std::collections::HashMap;

type Result<T> = result::Result<T, ()>;

const SAFE_MODE: bool = true;

struct Sensitive<T>(T);

impl<T: fmt::Display> fmt::Display for Sensitive<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
	if SAFE_MODE {
	    writeln!(f, "[REDACTED]")
	} else {
	    writeln!(f, "{}", self.0)
	}
    }
}

enum Message {
    ClientConnected {
	author: Arc<TcpStream>
    },
    ClientDisconnected {
	author: Arc<TcpStream>
    },
    NewMessage {
	author: Arc<TcpStream>,
	bytes: Vec<u8>,
    },
}

struct Client {
    conn: Arc<TcpStream>,
}

fn server(message: Receiver<Message>) -> Result<()> {
    let mut clients = HashMap::new();
    loop {
	match message.recv() {
	    Ok(Message::ClientConnected{author}) => {
		let addr = author.peer_addr().expect("Could not get peer address");
		clients.insert(addr.clone(), Client { conn: author.clone() });
		println!("Client connected: {addr}");
	    },
	    Ok(Message::ClientDisconnected{author}) => {
		let addr = author.peer_addr().expect("Could not get peer address");
		clients.remove(&addr);
		println!("Client disconnected: {addr}");
	    },
	    Ok(Message::NewMessage {author, bytes }) => {
		let author_addr = author.peer_addr().expect("Could not get peer address");
		for (addr, client) in clients.iter() {
		    if *addr != author_addr {
			let _ = client.conn.as_ref().write(&bytes);	    
		    }
		}
	    },
	    Err(err) => {
		eprintln!("Error: could not receive message: {err}");
	    },
	}
    }
}

fn client(stream: Arc<TcpStream>, messages: Sender<Message>)
	  -> Result<()>
{
    messages.send(Message::ClientConnected{ author: stream.clone()}).map_err(|err| {
	eprintln!("Error: could not send message: {err}");
    })?;
    let mut buffer = Vec::new();
    buffer.resize(64, 0);
    loop {
	let n = stream.deref().read(&mut buffer).map_err(|err| {
	    eprintln!("Error: could not read from stream: {err}");
	    let _ = messages.send(Message::ClientDisconnected { author: stream.clone() });
	})?;
	let _ = messages.send(
	    Message::NewMessage {
		author: stream.clone(),
		bytes: buffer[..n].to_vec(),
	    }
	);
    }
}

fn main() -> Result<()> {
    let address = "0.0.0.0:6969";
    let listener = TcpListener::bind(address).map_err(|err| {
	eprintln!("Error: could not bind to address {address}: {err}", err = Sensitive(err));
    })?;
    println!("Listening on: {address}");

    // Create a channel
    let (sender, receiver) = channel();
    thread::spawn(|| server(receiver));

    // accept connections and process them serially
    for stream in listener.incoming() {
	match stream {
	    Ok(stream) => {
		let stream = Arc::new(stream);
		let sender_clone = sender.clone();
		thread::spawn(|| client(stream, sender_clone)); 
	    },
	    Err(err) => {
		eprintln!("Error: could not accept connection: {err}");
	    },
	}
    }

    Ok(())
}
