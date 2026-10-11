//! Free internet hosting with no router setup and no server of our own: the host opens a tunnel
//! on the public `bore.pub` service (https://github.com/ekzhang/bore). Anybody who connects to
//! bore.pub:<port> is passed straight through to the host's game, so the room code is simply that
//! address plus the usual random room id.
//!
//! Protocol (null-terminated JSON): we send {"Hello":0}; the server answers {"Hello":<port>} and
//! then sends {"Connection":"<uuid>"} for every visitor. For each one we open a second connection
//! and send {"Accept":"<uuid>"}; from then on that connection carries the visitor's bytes.

use crate::net::RelayMsg;
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const SERVER: &str = "bore.pub";
const CONTROL_PORT: u16 = 7835;

fn server_addr() -> Result<SocketAddr, String> {
    let host = std::env::var("ISLAND_BORE").unwrap_or_else(|_| format!("{SERVER}:{CONTROL_PORT}"));
    let all: Vec<SocketAddr> = host.to_socket_addrs().map_err(|e| format!("couldn't find the tunnel server ({e})"))?.collect();
    all.iter().find(|a| a.is_ipv4()).or_else(|| all.first()).copied().ok_or_else(|| "couldn't find the tunnel server".to_string())
}

fn send_msg(s: &mut TcpStream, json: &str) -> bool {
    let mut b = json.as_bytes().to_vec();
    b.push(0);
    s.write_all(&b).is_ok()
}

/// Reads one null-terminated message (blocking up to the socket timeout). Ok(None) = timed out.
fn read_msg(s: &mut TcpStream, acc: &mut Vec<u8>) -> Result<Option<String>, String> {
    let mut buf = [0u8; 512];
    loop {
        if let Some(p) = acc.iter().position(|&b| b == 0) {
            let m: Vec<u8> = acc.drain(..=p).collect();
            return Ok(Some(String::from_utf8_lossy(&m[..m.len() - 1]).to_string()));
        }
        if acc.len() > 4096 {
            return Err("the tunnel server sent garbage".into());
        }
        match s.read(&mut buf) {
            Ok(0) => return Err("the tunnel server closed the connection".into()),
            Ok(n) => acc.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => return Ok(None),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(format!("lost the tunnel connection ({e})")),
        }
    }
}

fn between<'a>(s: &'a str, pre: &str, post: &str) -> Option<&'a str> {
    let a = s.find(pre)? + pre.len();
    let b = s[a..].find(post)? + a;
    Some(&s[a..b])
}

pub fn host_thread(tx: Sender<RelayMsg>, stop: Arc<AtomicBool>) {
    let fail = |m: String| {
        let _ = tx.send(RelayMsg::Failed(m));
    };
    let sa = match server_addr() {
        Ok(a) => a,
        Err(e) => return fail(e),
    };
    let mut s = match TcpStream::connect_timeout(&sa, Duration::from_secs(8)) {
        Ok(s) => s,
        Err(e) => return fail(format!("couldn't reach the tunnel server ({e})")),
    };
    let _ = s.set_nodelay(true);
    let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
    if !send_msg(&mut s, "{\"Hello\":0}") {
        return fail("the tunnel server hung up".into());
    }
    let mut acc = vec![];
    let started = Instant::now();
    let port: u16 = loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        match read_msg(&mut s, &mut acc) {
            Err(e) => return fail(e),
            Ok(Some(m)) => {
                if let Some(p) = between(&m, "{\"Hello\":", "}").and_then(|v| v.trim().parse::<u16>().ok()) {
                    break p;
                }
                if m.contains("Error") {
                    return fail(format!("the tunnel server said: {m}"));
                }
            }
            Ok(None) => {}
        }
        if started.elapsed() > Duration::from_secs(10) {
            return fail("the tunnel server didn't answer".into());
        }
    };
    let ip = match sa.ip() {
        std::net::IpAddr::V4(v) => v,
        _ => Ipv4Addr::UNSPECIFIED,
    };
    if tx.send(RelayMsg::Tunnel(ip, port)).is_err() {
        return;
    }
    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        match read_msg(&mut s, &mut acc) {
            Err(e) => return fail(e),
            Ok(Some(m)) => {
                if let Some(id) = between(&m, "{\"Connection\":\"", "\"") {
                    let (id, tx2) = (id.to_string(), tx.clone());
                    std::thread::spawn(move || {
                        if let Ok(mut d) = TcpStream::connect_timeout(&sa, Duration::from_secs(8)) {
                            let _ = d.set_nodelay(true);
                            if send_msg(&mut d, &format!("{{\"Accept\":\"{id}\"}}")) {
                                let _ = tx2.send(RelayMsg::Stream(d));
                            }
                        }
                    });
                } else if m.contains("Error") {
                    return fail(format!("the tunnel server said: {m}"));
                }
            }
            Ok(None) => {}
        }
    }
}
