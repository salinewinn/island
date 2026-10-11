//! Island relay server.
//!
//! Players behind home routers can't accept incoming connections, so both the host and the joiners
//! connect OUT to this relay, which pipes them together. It only forwards bytes; it never looks at
//! game data. Usage: `island-relay [port]` (default 47400, or the PORT environment variable).
//!
//! Protocol (one text line, then raw bytes):
//!   host control : "HOST <room>"            -> "OK" | "ERR ..."; then the host sends "PING" now and
//!                                              then (answered with "PONG"); the relay sends
//!                                              "NEW <id>" whenever somebody joins the room
//!   host data    : "ACCEPT <room> <id>"     -> becomes the pipe to that joiner
//!   joiner       : "JOIN <room>"            -> piped to the host (or "ERR ..." and closed)

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const MAX_ROOMS: usize = 2000;
const MAX_PENDING_PER_ROOM: usize = 8;

struct Room {
    ctl: TcpStream,
    pending: HashMap<u64, (TcpStream, Instant)>,
}

struct State {
    rooms: HashMap<String, Room>,
    next_id: u64,
}

type Shared = Arc<Mutex<State>>;

fn read_line(s: &mut TcpStream, max: usize) -> Option<String> {
    // one byte at a time so nothing after the newline is swallowed
    let mut buf = Vec::new();
    let mut b = [0u8; 1];
    loop {
        match s.read(&mut b) {
            Ok(1) => {
                if b[0] == b'\n' {
                    return Some(String::from_utf8_lossy(&buf).trim().to_string());
                }
                buf.push(b[0]);
                if buf.len() > max {
                    return None;
                }
            }
            _ => return None,
        }
    }
}

fn valid_room(r: &str) -> bool {
    r.len() == 12 && r.chars().all(|c| c.is_ascii_hexdigit())
}

fn pipe(a: TcpStream, b: TcpStream) {
    let _ = a.set_read_timeout(None);
    let _ = b.set_read_timeout(None);
    let (mut a_r, mut b_w) = (a.try_clone().unwrap(), b.try_clone().unwrap());
    let (mut b_r, mut a_w) = (b, a);
    let t = thread::spawn(move || {
        let _ = std::io::copy(&mut a_r, &mut b_w);
        let _ = b_w.shutdown(Shutdown::Both);
        let _ = a_r.shutdown(Shutdown::Both);
    });
    let _ = std::io::copy(&mut b_r, &mut a_w);
    let _ = a_w.shutdown(Shutdown::Both);
    let _ = b_r.shutdown(Shutdown::Both);
    let _ = t.join();
}

fn handle(mut s: TcpStream, st: Shared) {
    let _ = s.set_nodelay(true);
    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = s.set_write_timeout(Some(Duration::from_secs(10)));
    let Some(line) = read_line(&mut s, 100) else { return };
    let parts: Vec<&str> = line.split_whitespace().collect();
    match parts.as_slice() {
        ["HOST", room] if valid_room(room) => {
            let room = room.to_string();
            {
                let mut g = st.lock().unwrap();
                if g.rooms.len() >= MAX_ROOMS {
                    let _ = s.write_all(b"ERR relay is full, try again later\n");
                    return;
                }
                if g.rooms.contains_key(&room) {
                    let _ = s.write_all(b"ERR room taken\n");
                    return;
                }
                let Ok(ctl) = s.try_clone() else { return };
                g.rooms.insert(room.clone(), Room { ctl, pending: HashMap::new() });
            }
            let _ = s.write_all(b"OK\n");
            // keep the control connection alive until the host goes away
            let _ = s.set_read_timeout(Some(Duration::from_secs(60)));
            while let Some(l) = read_line(&mut s, 50) {
                if l == "PING" {
                    let _ = s.write_all(b"PONG\n");
                }
            }
            let mut g = st.lock().unwrap();
            if let Some(r) = g.rooms.remove(&room) {
                for (_, (j, _)) in r.pending {
                    let _ = j.shutdown(Shutdown::Both);
                }
            }
        }
        ["JOIN", room] if valid_room(room) => {
            let id;
            {
                let mut g = st.lock().unwrap();
                g.next_id += 1;
                id = g.next_id;
                let Some(r) = g.rooms.get_mut(*room) else {
                    let _ = s.write_all(b"ERR No game found for that code. Ask the host for a fresh code (a new one is made every time they host).\n");
                    return;
                };
                r.pending.retain(|_, (_, t)| t.elapsed() < Duration::from_secs(12));
                if r.pending.len() >= MAX_PENDING_PER_ROOM {
                    let _ = s.write_all(b"ERR The host is busy, try again in a moment\n");
                    return;
                }
                let Ok(joiner) = s.try_clone() else { return };
                if r.ctl.write_all(format!("NEW {id}\n").as_bytes()).is_err() {
                    let _ = s.write_all(b"ERR The host went away\n");
                    return;
                }
                r.pending.insert(id, (joiner, Instant::now()));
            }
            // if the host never picks the connection up, tell the joiner
            let (st2, room) = (st.clone(), room.to_string());
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(12));
                let mut g = st2.lock().unwrap();
                if let Some(r) = g.rooms.get_mut(&room) {
                    if let Some((mut j, _)) = r.pending.remove(&id) {
                        let _ = j.write_all(b"ERR The host didn't answer in time\n");
                        let _ = j.shutdown(Shutdown::Both);
                    }
                }
            });
        }
        ["ACCEPT", room, id] if valid_room(room) => {
            let Ok(id) = id.parse::<u64>() else { return };
            let joiner = {
                let mut g = st.lock().unwrap();
                g.rooms.get_mut(*room).and_then(|r| r.pending.remove(&id))
            };
            if let Some((j, _)) = joiner {
                pipe(s, j);
            }
        }
        _ => {
            let _ = s.write_all(b"ERR bad request\n");
        }
    }
}

fn main() {
    let port = std::env::args().nth(1).or_else(|| std::env::var("PORT").ok()).and_then(|p| p.parse::<u16>().ok()).unwrap_or(47400);
    let listener = TcpListener::bind(("0.0.0.0", port)).expect("couldn't open the relay port");
    println!("island-relay listening on port {port}");
    let st: Shared = Arc::new(Mutex::new(State { rooms: HashMap::new(), next_id: 0 }));
    for conn in listener.incoming() {
        match conn {
            Ok(s) => {
                let st = st.clone();
                thread::spawn(move || handle(s, st));
            }
            Err(_) => thread::sleep(Duration::from_millis(50)),
        }
    }
}
