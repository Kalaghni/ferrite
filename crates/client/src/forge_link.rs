//! The map maker's door for tools: newline-separated JSON requests on a
//! TCP port on this machine only, so the FORGE MCP server (and with it an
//! AI assistant) can read and edit the open map while it is on screen.
//! Every edit lands as an ordinary undo step.
//!
//! The socket is never blocked on: the editor polls it once a frame.

use ferrite_shared::map::ops::BRIDGE_PORT;
use serde_json::Value;
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};

/// The longest request taken, so a stuck client cannot grow a buffer forever.
const MAX_LINE: usize = 8 << 20;

struct Conn {
    stream: TcpStream,
    buf: Vec<u8>,
    dead: bool,
}

pub struct Link {
    listener: TcpListener,
    conns: Vec<Conn>,
}

/// A request, and which connection to answer on.
pub struct Request {
    pub conn: usize,
    pub body: Value,
}

impl Link {
    /// Listen on the loopback port, or `None` if another editor has it.
    pub fn open() -> Option<Link> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, BRIDGE_PORT)).ok()?;
        listener.set_nonblocking(true).ok()?;
        Some(Link { listener, conns: Vec::new() })
    }

    /// New connections, and every whole request that has arrived.
    pub fn poll(&mut self) -> Vec<Request> {
        while let Ok((stream, _)) = self.listener.accept() {
            if stream.set_nonblocking(true).is_ok() {
                let _ = stream.set_nodelay(true);
                self.conns.push(Conn { stream, buf: Vec::new(), dead: false });
            }
        }
        let mut out = Vec::new();
        for (k, c) in self.conns.iter_mut().enumerate() {
            let mut chunk = [0u8; 16384];
            loop {
                match c.stream.read(&mut chunk) {
                    Ok(0) => {
                        c.dead = true;
                        break;
                    }
                    Ok(n) => c.buf.extend_from_slice(&chunk[..n]),
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(_) => {
                        c.dead = true;
                        break;
                    }
                }
            }
            while let Some(nl) = c.buf.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = c.buf.drain(..=nl).collect();
                let body = serde_json::from_slice(&line).unwrap_or_else(|e| serde_json::json!({ "cmd": "bad", "error": e.to_string() }));
                out.push(Request { conn: k, body });
            }
            if c.buf.len() > MAX_LINE {
                c.dead = true;
            }
        }
        out
    }

    /// Answer on connection `conn`. Answers are small apart from a whole
    /// document, so a short blocking write is fine.
    pub fn reply(&mut self, conn: usize, v: &Value) {
        if let Some(c) = self.conns.get_mut(conn) {
            let mut line = v.to_string();
            line.push('\n');
            let _ = c.stream.set_nonblocking(false);
            if c.stream.write_all(line.as_bytes()).is_err() {
                c.dead = true;
            }
            let _ = c.stream.set_nonblocking(true);
        }
    }

    /// Drop closed connections. Only with no answer pending: connection
    /// ids would shift under it.
    pub fn sweep(&mut self) {
        self.conns.retain(|c| !c.dead);
    }

    /// Whether any tool is connected.
    pub fn linked(&self) -> bool {
        self.conns.iter().any(|c| !c.dead)
    }
}
