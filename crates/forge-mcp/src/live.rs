//! Talking to a running map maker over its loopback port.

use ferrite_shared::map::ops::BRIDGE_PORT;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

/// Send one request and wait up to `wait` for its answer. An answer with
/// `"ok": false` comes back as its error.
pub fn request(body: Value, wait: Duration) -> Result<Value, String> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, BRIDGE_PORT));
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(400)).map_err(|_| "FORGE is not open".to_string())?;
    s.set_read_timeout(Some(wait)).map_err(|e| e.to_string())?;
    let mut line = body.to_string();
    line.push('\n');
    s.write_all(line.as_bytes()).map_err(|e| format!("FORGE link: {e}"))?;
    let mut answer = String::new();
    BufReader::new(&s)
        .read_line(&mut answer)
        .map_err(|_| "FORGE did not answer in time (it may be in a playtest or minimised; it answers between frames)".to_string())?;
    if answer.is_empty() {
        return Err("FORGE closed the link".into());
    }
    let v: Value = serde_json::from_str(&answer).map_err(|e| format!("FORGE sent something odd: {e}"))?;
    if v["ok"] == Value::Bool(false) {
        return Err(v["error"].as_str().map(str::to_string).unwrap_or_else(|| v["error"].to_string()));
    }
    Ok(v)
}

pub fn status() -> Option<Value> {
    request(serde_json::json!({ "cmd": "status" }), Duration::from_secs(3)).ok()
}
