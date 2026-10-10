//! Tiny TCP "room code" multiplayer for up to four players.
//!
//! One player hosts and gets a room code that encodes their address, port and a random room id
//! (a new one every time they host). Friends type the code in to join. The host relays everybody's
//! state to everybody else. Messages are plain text lines.

use crate::upnp;
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const PORT: u16 = 47321;
pub const MAX_PLAYERS: usize = 4;
/// Bump whenever the world or protocol changes: players must run the same build.
pub const VERSION: u32 = 5;

const ALPHA: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// Host-side id of a connection that hasn't finished the HELLO handshake yet.
const NO_ID: u8 = 255;
const MAX_LINE: usize = 64 * 1024;
const MAX_MSG_CHARS: usize = 500;
const MAX_PENDING: usize = 16;

#[derive(Clone, Debug, Default)]
pub struct PlayerNet {
    pub id: u8,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub slot: u8,
    pub fstate: u8,
    pub lure: [f32; 3],
    pub tier: u8,
    pub swim: bool,
    pub speed: f32,
}

#[derive(Clone, Debug)]
pub enum NetEvent {
    State(PlayerNet),
    Join(u8),
    Leave(u8),
    Time(f32),
    Pick(u32),
    Spawn { id: u32, p: [f32; 3], v: [f32; 3] },
    Msg(String),
    Welcome(u8),
    /// another player's encoded character look (people::Look::encode)
    Look(u8, String),
    Disconnected(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Off,
    Host,
    Client,
    Connecting,
}

struct Peer {
    stream: TcpStream,
    inbuf: Vec<u8>,
    out: Vec<u8>,
    /// internal unique number (ids can be reused, this can't)
    uid: u32,
    /// player id; NO_ID on the host until HELLO is accepted; 0 (the host) on a client
    id: u8,
    /// set when we are sending a final error line and will then hang up
    kick_at: Option<Instant>,
    shut: bool,
    since: Instant,
    last_rx: Instant,
    last_tx: Instant,
}

type Done = Arc<AtomicBool>;

fn wait_done(d: &Done, max: Duration) {
    let t = Instant::now();
    while !d.load(Ordering::Acquire) && t.elapsed() < max {
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub struct Net {
    pub mode: Mode,
    listener: Option<TcpListener>,
    peers: Vec<Peer>,
    pub my_id: u8,
    pub names: [Option<String>; MAX_PLAYERS],
    /// Code for friends on the same network (or VPN).
    pub code: String,
    /// Code for friends on the internet (filled in once the router / lookup answers).
    pub inet_code: String,
    pub inet_status: String,
    pub status: String,
    nonce: u16,
    last_nonce: u16,
    port: u16,
    connect_rx: Option<Receiver<Result<TcpStream, String>>>,
    upnp_rx: Option<Receiver<Result<upnp::Mapping, String>>>,
    pub_rx: Option<Receiver<Result<Ipv4Addr, String>>>,
    mapping: Option<upnp::Mapping>,
    /// set while a background thread is removing a router mapping
    closer: Option<Done>,
    join_nonce: u16,
    join_ver: u32,
    welcomed: bool,
    removed: Vec<u32>,
    looks: [Option<String>; MAX_PLAYERS],
    my_look: Option<String>,
    next_uid: u32,
    /// ask the router for a port mapping when hosting (tests switch this off)
    pub use_upnp: bool,
    hello_timeout: Duration,
    idle_timeout: Duration,
    keepalive: Duration,
}

fn encode(ip: Ipv4Addr, port: u16, nonce: u16) -> String {
    let v: u64 = ((u32::from(ip) as u64) << 32) | ((port as u64) << 16) | nonce as u64;
    let mut s = String::new();
    // 64 bits -> 13 symbols of 5 bits (the top symbol only uses 4 bits)
    for i in (0..13).rev() {
        let idx = ((v >> (i * 5)) & 31) as usize;
        s.push(ALPHA[idx] as char);
        if i == 8 || i == 3 {
            s.push('-');
        }
    }
    s
}

/// Returns the address to connect to and the room id (0 when a raw address was typed).
pub fn decode(text: &str) -> Result<(SocketAddr, u16), String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("Type the room code first".into());
    }
    if t.contains('.') || t.contains(':') {
        let a = if let Ok(a) = t.parse::<SocketAddr>() {
            a
        } else if let Ok(ip) = t.parse::<Ipv4Addr>() {
            SocketAddr::from((ip, PORT))
        } else {
            return Err("That isn't a valid address".into());
        };
        if a.port() == 0 || a.ip().is_unspecified() {
            return Err("That isn't a valid address".into());
        }
        return Ok((a, 0));
    }
    let mut v: u64 = 0;
    let mut n = 0;
    for ch in t.chars() {
        if ch == '-' || ch == ' ' {
            continue;
        }
        let ch = match ch.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            'U' => 'V',
            c => c,
        };
        match ALPHA.iter().position(|&a| a as char == ch) {
            Some(p) => {
                if n == 0 && p > 15 {
                    return Err("That code isn't valid (check it for typing mistakes)".into());
                }
                v = (v << 5) | p as u64;
                n += 1;
                if n > 13 {
                    break;
                }
            }
            None => return Err("The code has a character that isn't allowed".into()),
        }
    }
    if n != 13 {
        return Err("Room codes are 13 characters long (like ABCDE-FGHJK-MNP)".into());
    }
    let nonce = (v & 0xFFFF) as u16;
    let port = ((v >> 16) & 0xFFFF) as u16;
    let ip = Ipv4Addr::from((v >> 32) as u32);
    if port == 0 || ip.is_unspecified() {
        return Err("That code isn't valid (check it for typing mistakes)".into());
    }
    Ok((SocketAddr::from((ip, port)), nonce))
}

pub fn local_ip() -> Ipv4Addr {
    // Connecting a UDP socket sends nothing; it just lets the OS pick the outgoing interface.
    if let Ok(s) = UdpSocket::bind("0.0.0.0:0") {
        if s.connect("8.8.8.8:80").is_ok() {
            if let Ok(SocketAddr::V4(a)) = s.local_addr() {
                return *a.ip();
            }
        }
    }
    Ipv4Addr::LOCALHOST
}

/// A fresh random room id: never 0 (means "raw address") and never equal to the previous one.
fn fresh_nonce(prev: u16) -> u16 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    static CTR: AtomicU64 = AtomicU64::new(0);
    let c = CTR.fetch_add(1, Ordering::Relaxed);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(1);
    let mut h = RandomState::new().build_hasher();
    h.write_u128(t);
    h.write_u64(c);
    h.write_u32(std::process::id());
    let mut x = h.finish();
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    let mut n = (x >> 32) as u16;
    while n == 0 || n == prev {
        n = n.wrapping_add(1);
    }
    n
}

fn f(it: &mut std::str::SplitWhitespace) -> f32 {
    it.next().and_then(|s| s.parse::<f32>().ok()).filter(|v| v.is_finite()).unwrap_or(0.0)
}

fn fmt_state(s: &PlayerNet) -> String {
    format!(
        "S {} {:.2} {:.2} {:.2} {:.3} {:.3} {} {} {:.2} {:.2} {:.2} {} {} {:.2}\n",
        s.id, s.pos[0], s.pos[1], s.pos[2], s.yaw, s.pitch, s.slot, s.fstate, s.lure[0], s.lure[1], s.lure[2], s.tier, s.swim as u8, s.speed
    )
}

fn parse_state(rest: &str) -> Option<PlayerNet> {
    let mut it = rest.split_whitespace();
    let id = it.next()?.parse().ok()?;
    let pos = [f(&mut it), f(&mut it), f(&mut it)];
    let yaw = f(&mut it);
    let pitch = f(&mut it);
    let slot = f(&mut it) as u8;
    let fstate = f(&mut it) as u8;
    let lure = [f(&mut it), f(&mut it), f(&mut it)];
    let tier = f(&mut it) as u8;
    let swim = f(&mut it) > 0.5;
    let speed = f(&mut it);
    Some(PlayerNet { id, pos, yaw, pitch, slot, fstate, lure, tier, swim, speed })
}

/// A look code is digits and dots only, and short.
fn valid_look(s: &str) -> bool {
    !s.is_empty() && s.len() < 64 && s.bytes().all(|b| b.is_ascii_digit() || b == b'.')
}

fn parse_spawn(rest: &str) -> Option<(u32, [f32; 3], [f32; 3])> {
    let mut it = rest.split_whitespace();
    let id = it.next()?.parse::<u32>().ok()?;
    let p = [f(&mut it), f(&mut it), f(&mut it)];
    let v = [f(&mut it), f(&mut it), f(&mut it)];
    Some((id, p, v))
}

fn fmt_spawn(id: u32, p: [f32; 3], v: [f32; 3]) -> String {
    format!("CN {id} {:.2} {:.2} {:.2} {:.2} {:.2} {:.2}\n", p[0], p[1], p[2], v[0], v[1], v[2])
}

/// Reads everything available from a peer; complete lines go to `lines`.
fn read_peer(p: &mut Peer, now: Instant, lines: &mut Vec<(u32, String)>, dead: &mut Vec<u32>) {
    let mut buf = [0u8; 4096];
    let mut got = 0usize;
    loop {
        match p.stream.read(&mut buf) {
            Ok(0) => {
                dead.push(p.uid);
                break;
            }
            Ok(n) => {
                p.last_rx = now;
                got += n;
                // a peer we are hanging up on is only drained, never listened to
                if p.kick_at.is_none() {
                    p.inbuf.extend_from_slice(&buf[..n]);
                }
                if got > 256 * 1024 {
                    break;
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => {
                dead.push(p.uid);
                break;
            }
        }
    }
    while let Some(pos) = p.inbuf.iter().position(|&b| b == b'\n') {
        let raw: Vec<u8> = p.inbuf.drain(..=pos).collect();
        if raw.len() > MAX_LINE {
            dead.push(p.uid);
            continue;
        }
        let s = String::from_utf8_lossy(&raw).trim().to_string();
        if !s.is_empty() {
            lines.push((p.uid, s));
        }
    }
    if p.inbuf.len() > MAX_LINE {
        dead.push(p.uid);
        p.inbuf.clear();
    }
}

impl Peer {
    fn new(stream: TcpStream, uid: u32, id: u8, now: Instant) -> Peer {
        let _ = stream.set_nodelay(true);
        let _ = stream.set_nonblocking(true);
        Peer { stream, inbuf: vec![], out: vec![], uid, id, kick_at: None, shut: false, since: now, last_rx: now, last_tx: now }
    }
}

impl Net {
    pub fn new() -> Self {
        Net {
            mode: Mode::Off,
            listener: None,
            peers: vec![],
            my_id: 0,
            names: [None, None, None, None],
            code: String::new(),
            inet_code: String::new(),
            inet_status: String::new(),
            status: String::new(),
            nonce: 0,
            last_nonce: 0,
            port: PORT,
            connect_rx: None,
            upnp_rx: None,
            pub_rx: None,
            mapping: None,
            closer: None,
            join_nonce: 0,
            join_ver: VERSION,
            welcomed: false,
            removed: vec![],
            looks: [None, None, None, None],
            my_look: None,
            next_uid: 1,
            use_upnp: true,
            hello_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(60),
            keepalive: Duration::from_secs(3),
        }
    }

    /// Starts a brand-new room. Every call makes a new room id, so the codes are never reused.
    pub fn host(&mut self) -> Result<(), String> {
        self.leave();
        let mut bound = None;
        for p in PORT..PORT + 10 {
            if let Ok(l) = TcpListener::bind(("0.0.0.0", p)) {
                bound = Some((l, p));
                break;
            }
        }
        let Some((l, port)) = bound else {
            self.status = "Couldn't open a network port to host on".into();
            return Err(self.status.clone());
        };
        if let Err(e) = l.set_nonblocking(true) {
            self.status = e.to_string();
            return Err(self.status.clone());
        }
        self.port = port;
        self.listener = Some(l);
        self.mode = Mode::Host;
        self.my_id = 0;
        self.welcomed = true;
        self.names = [Some("Player 1".into()), None, None, None];
        self.looks = [self.my_look.clone(), None, None, None];
        self.nonce = fresh_nonce(self.last_nonce);
        self.last_nonce = self.nonce;
        let lan = local_ip();
        self.code = encode(lan, self.port, self.nonce);
        self.inet_code.clear();
        self.removed.clear();
        self.status = if lan == Ipv4Addr::LOCALHOST {
            "Hosting, but no network connection was found - only players on this PC can join.".into()
        } else {
            "Hosting. Friends on your network can use the LAN code.".into()
        };
        if self.use_upnp && lan != Ipv4Addr::LOCALHOST {
            self.inet_status = "Asking your router to open the port (UPnP)...".into();
            let (tx, rx) = channel();
            // wait for any earlier mapping removal first, so it can't delete the new mapping
            let prev = self.closer.clone();
            std::thread::spawn(move || {
                if let Some(d) = prev {
                    wait_done(&d, Duration::from_secs(30));
                }
                let _ = tx.send(upnp::open_port(port, lan));
            });
            self.upnp_rx = Some(rx);
        } else {
            self.inet_status = "Internet play needs a router port forward or a VPN; use the lookup button.".into();
        }
        Ok(())
    }

    /// Explicit request: ask a web service for this PC's public address.
    pub fn lookup_public(&mut self) {
        if self.mode != Mode::Host || self.pub_rx.is_some() {
            return;
        }
        self.inet_status = "Looking up your public address...".into();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(upnp::lookup_public_ip());
        });
        self.pub_rx = Some(rx);
    }

    pub fn join(&mut self, text: &str) {
        self.join_inner(text, VERSION);
    }

    /// Test hook: pretend to be a different game version.
    #[cfg(test)]
    pub fn join_with_version(&mut self, text: &str, version: u32) {
        self.join_inner(text, version);
    }

    fn join_inner(&mut self, text: &str, version: u32) {
        self.leave();
        match decode(text) {
            Err(e) => self.status = e,
            Ok((addr, nonce)) => {
                self.join_nonce = nonce;
                self.join_ver = version;
                let (tx, rx) = channel();
                std::thread::spawn(move || {
                    let r = TcpStream::connect_timeout(&addr, Duration::from_secs(6)).map_err(|e| {
                        format!(
                            "Couldn't reach the host ({e}). Check the code, that the host allowed the game through their firewall, and that you use the internet code if you're on different networks."
                        )
                    });
                    let _ = tx.send(r);
                });
                self.connect_rx = Some(rx);
                self.mode = Mode::Connecting;
                self.status = "Connecting...".into();
            }
        }
    }

    /// Stops hosting / disconnects. The listening port is released immediately and the router
    /// mapping (if any) is removed in the background.
    pub fn leave(&mut self) {
        self.listener = None;
        self.peers.clear();
        self.connect_rx = None;
        self.pub_rx = None;
        self.close_mapping();
        self.mode = Mode::Off;
        self.names = [None, None, None, None];
        self.looks = [None, None, None, None];
        self.code.clear();
        self.inet_code.clear();
        self.inet_status.clear();
        self.nonce = 0;
        self.my_id = 0;
        self.welcomed = false;
        self.removed.clear();
    }

    fn close_mapping(&mut self) {
        let mapping = self.mapping.take();
        let pending = self.upnp_rx.take();
        if mapping.is_none() && pending.is_none() {
            return;
        }
        let prev = self.closer.take();
        let done: Done = Arc::new(AtomicBool::new(false));
        let d2 = done.clone();
        std::thread::spawn(move || {
            if let Some(p) = prev {
                wait_done(&p, Duration::from_secs(30));
            }
            let mut m = mapping;
            if m.is_none() {
                // the router request was still running: take over its result so it can't leak
                if let Some(rx) = pending {
                    if let Ok(Ok(mm)) = rx.recv_timeout(Duration::from_secs(45)) {
                        m = Some(mm);
                    }
                }
            }
            if let Some(m) = m {
                upnp::close_port(&m);
            }
            d2.store(true, Ordering::Release);
        });
        self.closer = Some(done);
    }

    fn can_send(&self) -> bool {
        self.mode == Mode::Host || (self.mode == Mode::Client && self.welcomed)
    }

    /// Queues a line for every handshaken peer (except one player id).
    fn broadcast(&mut self, line: &str, except: Option<u8>) {
        for p in self.peers.iter_mut() {
            if p.id != NO_ID && p.kick_at.is_none() && Some(p.id) != except {
                p.out.extend_from_slice(line.as_bytes());
            }
        }
    }

    pub fn send_state(&mut self, s: &PlayerNet) {
        if self.can_send() {
            let mut s = s.clone();
            s.id = self.my_id;
            let line = fmt_state(&s);
            self.broadcast(&line, None);
        }
    }

    pub fn send_time(&mut self, t: f32) {
        if self.mode == Mode::Host && t.is_finite() {
            self.broadcast(&format!("T {t:.2}\n"), None);
        }
    }

    pub fn send_pick(&mut self, id: u32) {
        if self.mode == Mode::Host && !self.removed.contains(&id) {
            self.removed.push(id);
        }
        if self.can_send() {
            self.broadcast(&format!("PK {id}\n"), None);
        }
    }

    pub fn send_spawn(&mut self, id: u32, p: [f32; 3], v: [f32; 3]) {
        if self.can_send() {
            self.broadcast(&fmt_spawn(id, p, v), None);
        }
    }

    /// Tell everyone what our character looks like (digits and dots only). Remembered, so it is
    /// also sent to players who join later, and re-sent after reconnecting.
    pub fn send_look(&mut self, encoded: &str) {
        let enc = encoded.trim();
        if !valid_look(enc) {
            return;
        }
        self.my_look = Some(enc.to_string());
        match self.mode {
            Mode::Host => {
                self.looks[0] = Some(enc.to_string());
                self.broadcast(&format!("LOOK 0 {enc}\n"), None);
            }
            Mode::Client if self.welcomed => self.broadcast(&format!("LOOK {enc}\n"), None),
            _ => {}
        }
    }

    pub fn send_msg(&mut self, text: &str) {
        if self.can_send() {
            let clean: String = text.chars().filter(|c| !c.is_control()).collect();
            self.broadcast(&format!("MSG {clean}\n"), None);
        }
    }

    fn set_inet(&mut self, ip: Ipv4Addr, note: &str) {
        self.inet_code = encode(ip, self.port, self.nonce);
        self.inet_status = note.to_string();
    }

    fn poll_background(&mut self) {
        if let Some(rx) = &self.upnp_rx {
            if let Ok(res) = rx.try_recv() {
                self.upnp_rx = None;
                match res {
                    Ok(m) => {
                        let ext = m.external_ip;
                        self.mapping = Some(m);
                        match ext {
                            Some(ip) if !upnp::is_private(ip) => self.set_inet(ip, "Port opened on your router. Share the internet code."),
                            Some(_) => {
                                self.inet_status = "Your router's address looks private (double NAT / mobile broadband), so friends outside your network probably can't reach you directly. Try a VPN such as Tailscale.".into()
                            }
                            None => self.inet_status = "Port opened, but your router didn't report its public address. Use the lookup button.".into(),
                        }
                    }
                    Err(e) => {
                        self.inet_status = format!(
                            "Router didn't open the port ({e}). For internet play, forward TCP port {} to this PC or use a VPN such as Tailscale, then use the lookup button.",
                            self.port
                        );
                    }
                }
            }
        }
        if let Some(rx) = &self.pub_rx {
            if let Ok(res) = rx.try_recv() {
                self.pub_rx = None;
                match res {
                    Ok(ip) => {
                        let opened = self.mapping.is_some();
                        let note = if opened {
                            "Port opened on your router. Share the internet code.".to_string()
                        } else {
                            format!("Public address found. This only works if TCP port {} is forwarded on your router.", self.port)
                        };
                        self.set_inet(ip, &note);
                    }
                    Err(e) => self.inet_status = format!("Couldn't look up the public address ({e})."),
                }
            }
        }
    }

    fn poll_connect(&mut self, now: Instant) {
        if let Some(rx) = &self.connect_rx {
            if let Ok(res) = rx.try_recv() {
                self.connect_rx = None;
                match res {
                    Ok(stream) => {
                        let uid = self.next_uid;
                        self.next_uid += 1;
                        let mut peer = Peer::new(stream, uid, 0, now);
                        peer.out.extend_from_slice(format!("HELLO {} {}\n", self.join_ver, self.join_nonce).as_bytes());
                        self.peers.push(peer);
                        self.mode = Mode::Client;
                        self.welcomed = false;
                        self.status = "Connected - waiting for the host...".into();
                    }
                    Err(e) => {
                        self.mode = Mode::Off;
                        self.status = e;
                    }
                }
            }
        }
    }

    fn accept_new(&mut self, now: Instant) {
        if self.mode != Mode::Host {
            return;
        }
        let Some(l) = &self.listener else { return };
        loop {
            match l.accept() {
                Ok((stream, _)) => {
                    if self.peers.len() >= MAX_PENDING + MAX_PLAYERS {
                        continue; // dropped: too many half-open connections
                    }
                    let uid = self.next_uid;
                    self.next_uid += 1;
                    self.peers.push(Peer::new(stream, uid, NO_ID, now));
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    }

    fn reject(&mut self, idx: usize, line: &str) {
        let p = &mut self.peers[idx];
        p.out.extend_from_slice(line.as_bytes());
        p.kick_at = Some(Instant::now());
    }

    fn host_hello(&mut self, idx: usize, rest: &str, ev: &mut Vec<NetEvent>) {
        if self.peers[idx].id != NO_ID {
            return;
        }
        let mut it = rest.split_whitespace();
        let ver: u32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let nonce: u16 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        if ver != VERSION {
            self.reject(idx, &format!("BADVER {VERSION}\n"));
            return;
        }
        if nonce != 0 && nonce != self.nonce {
            self.reject(idx, "BADCODE\n");
            return;
        }
        let Some(id) = (1..MAX_PLAYERS as u8).find(|i| !self.peers.iter().any(|p| p.id == *i)) else {
            self.reject(idx, "FULL\n");
            return;
        };
        let name = format!("Player {}", id + 1);
        self.peers[idx].id = id;
        self.names[id as usize] = Some(name.clone());
        self.looks[id as usize] = None;
        let mut msg = format!("WELCOME {id}\n");
        for (i, n) in self.names.iter().enumerate() {
            if let Some(n) = n {
                if i as u8 != id {
                    msg.push_str(&format!("JOIN {i} {n}\n"));
                }
            }
        }
        for r in &self.removed {
            msg.push_str(&format!("PK {r}\n"));
        }
        for (i, l) in self.looks.iter().enumerate() {
            if let Some(l) = l {
                if i as u8 != id && self.names[i].is_some() {
                    msg.push_str(&format!("LOOK {i} {l}\n"));
                }
            }
        }
        self.peers[idx].out.extend_from_slice(msg.as_bytes());
        self.broadcast(&format!("JOIN {id} {name}\n"), Some(id));
        ev.push(NetEvent::Join(id));
    }

    fn host_line(&mut self, uid: u32, line: &str, ev: &mut Vec<NetEvent>) {
        let Some(idx) = self.peers.iter().position(|p| p.uid == uid) else { return };
        if self.peers[idx].kick_at.is_some() {
            return;
        }
        let from = self.peers[idx].id;
        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        if cmd == "HELLO" {
            self.host_hello(idx, rest, ev);
            return;
        }
        if from == NO_ID || from as usize >= MAX_PLAYERS {
            return; // hasn't said hello properly
        }
        match cmd {
            "S" => {
                if let Some(mut st) = parse_state(rest) {
                    st.id = from;
                    let l = fmt_state(&st);
                    self.broadcast(&l, Some(from));
                    ev.push(NetEvent::State(st));
                }
            }
            "PK" => {
                if let Ok(id) = rest.parse::<u32>() {
                    if !self.removed.contains(&id) {
                        self.removed.push(id);
                    }
                    self.broadcast(&format!("PK {id}\n"), Some(from));
                    ev.push(NetEvent::Pick(id));
                }
            }
            "CN" => {
                if let Some((id, p, v)) = parse_spawn(rest) {
                    self.broadcast(&fmt_spawn(id, p, v), Some(from));
                    ev.push(NetEvent::Spawn { id, p, v });
                }
            }
            "MSG" => {
                let text: String = rest.chars().filter(|c| !c.is_control()).take(MAX_MSG_CHARS).collect();
                self.broadcast(&format!("MSG {text}\n"), Some(from));
                ev.push(NetEvent::Msg(text));
            }
            "LOOK" => {
                if valid_look(rest) {
                    self.looks[from as usize] = Some(rest.to_string());
                    self.broadcast(&format!("LOOK {from} {rest}\n"), Some(from));
                    ev.push(NetEvent::Look(from, rest.to_string()));
                }
            }
            _ => {}
        }
    }

    /// Returns Some(reason) when the connection must be closed.
    fn client_line(&mut self, line: &str, ev: &mut Vec<NetEvent>) -> Option<String> {
        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        match cmd {
            "FULL" => return Some("That room is full (4 players max)".into()),
            "BADVER" => {
                return Some(format!(
                    "The host is running a different version of the game (theirs is v{}, yours is v{}). Everyone needs the same Island.exe.",
                    rest, self.join_ver
                ))
            }
            "BADCODE" => return Some("That room code isn't valid any more - the host has started a new room. Ask for the new code.".into()),
            "WELCOME" => {
                if self.welcomed {
                    return None;
                }
                if let Ok(id) = rest.parse::<u8>() {
                    if (1..MAX_PLAYERS as u8).contains(&id) {
                        self.welcomed = true;
                        self.my_id = id;
                        self.names[id as usize] = Some(format!("Player {}", id + 1));
                        self.names[0] = Some("Player 1".into());
                        self.status = format!("Connected - you are Player {}", id + 1);
                        if let Some(l) = self.my_look.clone() {
                            self.broadcast(&format!("LOOK {l}\n"), None);
                        }
                        ev.push(NetEvent::Welcome(id));
                    }
                }
                return None;
            }
            _ => {}
        }
        if !self.welcomed {
            return None;
        }
        match cmd {
            "JOIN" => {
                let (id, name) = rest.split_once(' ').unwrap_or((rest, "Player"));
                if let Ok(id) = id.parse::<u8>() {
                    if (id as usize) < MAX_PLAYERS && id != self.my_id {
                        self.names[id as usize] = Some(name.chars().take(32).collect());
                        ev.push(NetEvent::Join(id));
                    }
                }
            }
            "BYE" => {
                if let Ok(id) = rest.parse::<u8>() {
                    if (id as usize) < MAX_PLAYERS && id != self.my_id {
                        self.names[id as usize] = None;
                        ev.push(NetEvent::Leave(id));
                    }
                }
            }
            "S" => {
                if let Some(st) = parse_state(rest) {
                    if (st.id as usize) < MAX_PLAYERS && st.id != self.my_id {
                        ev.push(NetEvent::State(st));
                    }
                }
            }
            "T" => {
                if let Some(t) = rest.parse::<f32>().ok().filter(|t| t.is_finite()) {
                    ev.push(NetEvent::Time(t));
                }
            }
            "PK" => {
                if let Ok(id) = rest.parse::<u32>() {
                    ev.push(NetEvent::Pick(id));
                }
            }
            "CN" => {
                if let Some((id, p, v)) = parse_spawn(rest) {
                    ev.push(NetEvent::Spawn { id, p, v });
                }
            }
            "MSG" => ev.push(NetEvent::Msg(rest.chars().take(MAX_MSG_CHARS).collect())),
            "LOOK" => {
                let (id, code) = rest.split_once(' ').unwrap_or((rest, ""));
                if let Ok(id) = id.parse::<u8>() {
                    if (id as usize) < MAX_PLAYERS && id != self.my_id && valid_look(code) {
                        ev.push(NetEvent::Look(id, code.to_string()));
                    }
                }
            }
            _ => {}
        }
        None
    }

    /// Call every frame: accepts connections, reads/writes sockets, returns what happened.
    pub fn poll(&mut self) -> Vec<NetEvent> {
        let mut ev = vec![];
        let now = Instant::now();
        self.poll_background();
        self.poll_connect(now);
        self.accept_new(now);

        // read from everyone
        let mut lines: Vec<(u32, String)> = vec![];
        let mut dead: Vec<u32> = vec![];
        for p in self.peers.iter_mut() {
            read_peer(p, now, &mut lines, &mut dead);
        }

        let mut fatal: Option<String> = None;
        for (uid, line) in lines {
            match self.mode {
                Mode::Host => self.host_line(uid, &line, &mut ev),
                Mode::Client => {
                    if fatal.is_none() {
                        fatal = self.client_line(&line, &mut ev);
                    }
                }
                _ => {}
            }
        }

        // timeouts and keep-alives
        let (mode, welcomed, hello, idle, ka) = (self.mode, self.welcomed, self.hello_timeout, self.idle_timeout, self.keepalive);
        for p in self.peers.iter_mut() {
            if let Some(k) = p.kick_at {
                if now.saturating_duration_since(k) > Duration::from_secs(3) {
                    dead.push(p.uid);
                }
                continue;
            }
            let idle_for = now.saturating_duration_since(p.last_rx);
            let since = now.saturating_duration_since(p.since);
            match mode {
                Mode::Host if p.id == NO_ID => {
                    if since > hello {
                        dead.push(p.uid);
                    }
                }
                Mode::Host => {
                    if idle_for > idle {
                        dead.push(p.uid);
                    } else if now.saturating_duration_since(p.last_tx) > ka {
                        p.out.extend_from_slice(b"P\n");
                    }
                }
                Mode::Client => {
                    if !welcomed {
                        if since > hello && fatal.is_none() {
                            fatal = Some("The host didn't answer. Check the code and that you both run the same version of the game.".into());
                        }
                    } else if idle_for > idle {
                        if fatal.is_none() {
                            fatal = Some("Lost the connection to the host (no answer).".into());
                        }
                    } else if now.saturating_duration_since(p.last_tx) > ka {
                        p.out.extend_from_slice(b"P\n");
                    }
                }
                _ => {}
            }
        }

        // write out whatever is queued
        for p in self.peers.iter_mut() {
            while !p.out.is_empty() {
                match p.stream.write(&p.out) {
                    Ok(0) => {
                        dead.push(p.uid);
                        break;
                    }
                    Ok(n) => {
                        p.out.drain(..n);
                        p.last_tx = now;
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(_) => {
                        dead.push(p.uid);
                        break;
                    }
                }
            }
            // final message delivered: close our side politely, then wait for them to hang up
            if p.kick_at.is_some() && p.out.is_empty() && !p.shut {
                let _ = p.stream.shutdown(Shutdown::Write);
                p.shut = true;
            }
            if p.out.len() > 1 << 20 {
                dead.push(p.uid);
            }
        }

        // drop broken connections
        dead.sort_unstable();
        dead.dedup();
        match self.mode {
            Mode::Host => {
                for uid in dead {
                    if let Some(pos) = self.peers.iter().position(|p| p.uid == uid) {
                        let p = self.peers.remove(pos);
                        if p.id != NO_ID && (p.id as usize) < MAX_PLAYERS && self.names[p.id as usize].take().is_some() {
                            self.looks[p.id as usize] = None;
                            self.broadcast(&format!("BYE {}\n", p.id), None);
                            ev.push(NetEvent::Leave(p.id));
                        }
                    }
                }
            }
            Mode::Client => {
                if !dead.is_empty() || fatal.is_some() {
                    self.status = match fatal {
                        Some(m) => m,
                        None if self.welcomed => "Disconnected from the host".into(),
                        None => "The host closed the connection. Is the room code current and are you both on the same version?".into(),
                    };
                    self.peers.clear();
                    self.mode = Mode::Off;
                    self.welcomed = false;
                    self.names = [None, None, None, None];
                    self.my_id = 0;
                    ev.push(NetEvent::Disconnected(self.status.clone()));
                }
            }
            _ => {}
        }
        ev
    }
}

impl Drop for Net {
    /// Quitting the game frees the router mapping (waits briefly so the message gets out).
    fn drop(&mut self) {
        self.leave();
        if let Some(d) = self.closer.take() {
            wait_done(&d, Duration::from_millis(1500));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // hosts use a small range of ports, so tests that host run one at a time
    static LOCK: Mutex<()> = Mutex::new(());
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    struct T {
        n: Net,
        ev: Vec<NetEvent>,
    }
    impl T {
        fn host() -> T {
            let mut n = Net::new();
            n.use_upnp = false;
            n.host().expect("host");
            T { n, ev: vec![] }
        }
        fn client(code: &str) -> T {
            let mut n = Net::new();
            n.join(code);
            T { n, ev: vec![] }
        }
        fn poll(&mut self) {
            let e = self.n.poll();
            self.ev.extend(e);
        }
        fn has(&self, f: impl Fn(&NetEvent) -> bool) -> bool {
            self.ev.iter().any(f)
        }
    }
    macro_rules! all {
        ($($x:expr),*) => { &mut [$(&mut $x),*] };
    }
    fn wait(ts: &mut [&mut T], what: &str, cond: impl Fn(&[&mut T]) -> bool) {
        let t0 = Instant::now();
        loop {
            for t in ts.iter_mut() {
                t.poll();
            }
            if cond(&*ts) {
                return;
            }
            if t0.elapsed() > Duration::from_secs(10) {
                panic!("timed out waiting for: {what}");
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn pump(ts: &mut [&mut T], ms: u64) {
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_millis(ms) {
            for t in ts.iter_mut() {
                t.poll();
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn code_for(h: &Net) -> String {
        encode(Ipv4Addr::LOCALHOST, h.port, h.nonce)
    }
    fn welcomed(c: &T) -> bool {
        c.has(|e| matches!(e, NetEvent::Welcome(_)))
    }

    #[test]
    fn code_round_trip_and_errors() {
        let ip = Ipv4Addr::new(203, 0, 113, 77);
        for (port, nonce) in [(47321u16, 1u16), (65535, 65535), (1, 2), (47322, 40000)] {
            let c = encode(ip, port, nonce);
            assert_eq!(c.len(), 15);
            let (a, n) = decode(&c).unwrap();
            assert_eq!(a, SocketAddr::from((ip, port)));
            assert_eq!(n, nonce);
            // forgiving input
            assert_eq!(decode(&c.to_lowercase().replace('-', " ")).unwrap().1, nonce);
        }
        assert!(decode("").is_err());
        assert!(decode("ABCDE-FGHJK-MN").is_err()); // too short
        assert!(decode("ABCDE-FGHJK-MNPQR").is_err()); // too long
        assert!(decode("ABCDE-FGHJK-MN!").is_err()); // bad char
        assert!(decode("ZZZZZ-ZZZZZ-ZZZ").is_err()); // would overflow 64 bits
        assert!(decode(&encode(Ipv4Addr::UNSPECIFIED, 1, 1)).is_err());
        assert!(decode(&encode(ip, 0, 1)).is_err());
        assert_eq!(decode("1.2.3.4").unwrap(), (SocketAddr::from(([1, 2, 3, 4], PORT)), 0));
        assert_eq!(decode(" 1.2.3.4:5000 ").unwrap(), (SocketAddr::from(([1, 2, 3, 4], 5000)), 0));
        assert!(decode("1.2.3.4:0").is_err());
        assert!(decode("999.1.1.1").is_err());
        assert!(decode("1.2.3").is_err());
    }

    #[test]
    fn handshake_full_room_and_id_reuse() {
        let _g = lock();
        let mut h = T::host();
        let code = code_for(&h.n);
        let mut c1 = T::client(&code);
        wait(all!(h, c1), "c1 welcome", |t| welcomed(t[1]) && t[0].has(|e| matches!(e, NetEvent::Join(1))));
        assert_eq!(c1.n.my_id, 1);
        assert_eq!(c1.n.mode, Mode::Client);
        let mut c2 = T::client(&code);
        wait(all!(h, c1, c2), "c2 welcome", |t| welcomed(t[2]));
        let mut c3 = T::client(&code);
        wait(all!(h, c1, c2, c3), "c3 welcome", |t| welcomed(t[3]));
        assert_eq!((c2.n.my_id, c3.n.my_id), (2, 3));
        assert!(c1.has(|e| matches!(e, NetEvent::Join(3)))); // told about later players
        assert!(c3.n.names[1].is_some() && c3.n.names[2].is_some() && c3.n.names[0].is_some());
        // fourth client: full
        let mut c4 = T::client(&code);
        wait(all!(h, c1, c2, c3, c4), "full", |t| t[4].has(|e| matches!(e, NetEvent::Disconnected(_))));
        assert!(c4.n.status.contains("full"), "{}", c4.n.status);
        assert_eq!(c4.n.mode, Mode::Off);
        assert_eq!(c4.ev.iter().filter(|e| matches!(e, NetEvent::Disconnected(_))).count(), 1);
        // a slot frees up and is reused
        c2.n.leave();
        wait(all!(h, c1, c3), "leave 2", |t| t[0].has(|e| matches!(e, NetEvent::Leave(2))) && t[1].has(|e| matches!(e, NetEvent::Leave(2))));
        let mut c5 = T::client(&code);
        wait(all!(h, c1, c3, c5), "c5 welcome", |t| welcomed(t[3]));
        assert_eq!(c5.n.my_id, 2);
    }

    #[test]
    fn relay_state_pick_spawn_msg_time() {
        let _g = lock();
        let mut h = T::host();
        let code = code_for(&h.n);
        let mut c1 = T::client(&code);
        wait(all!(h, c1), "c1", |t| welcomed(t[1]));
        let mut c2 = T::client(&code);
        wait(all!(h, c1, c2), "c2", |t| welcomed(t[2]));
        let st = |x: f32| PlayerNet { pos: [x, 2.0, 3.0], yaw: 1.5, slot: 2, swim: true, speed: 3.0, ..Default::default() };
        c1.n.send_state(&st(10.0));
        h.n.send_state(&st(5.0));
        h.n.send_time(12.5);
        c1.n.send_pick(77);
        c2.n.send_spawn(9, [1.0, 2.0, 3.0], [0.5, 0.0, -0.5]);
        c2.n.send_msg("hi\nthere");
        let seen = |t: &T, id: u8, x: f32| t.has(|e| matches!(e, NetEvent::State(s) if s.id == id && (s.pos[0] - x).abs() < 0.01 && s.swim && s.slot == 2));
        wait(all!(h, c1, c2), "relay", |t| {
            seen(t[0], 1, 10.0)
                && seen(t[2], 1, 10.0)
                && seen(t[1], 0, 5.0)
                && seen(t[2], 0, 5.0)
                && t[1].has(|e| matches!(e, NetEvent::Time(x) if (*x - 12.5).abs() < 0.01))
                && t[0].has(|e| matches!(e, NetEvent::Pick(77)))
                && t[2].has(|e| matches!(e, NetEvent::Pick(77)))
                && t[0].has(|e| matches!(e, NetEvent::Spawn { id: 9, .. }))
                && t[1].has(|e| matches!(e, NetEvent::Spawn { id: 9, .. }))
                && t[0].has(|e| matches!(e, NetEvent::Msg(m) if m == "hithere"))
                && t[1].has(|e| matches!(e, NetEvent::Msg(m) if m == "hithere"))
        });
        pump(all!(h, c1, c2), 200);
        // nothing is echoed back to its sender
        assert!(!c1.has(|e| matches!(e, NetEvent::State(s) if s.id == 1)));
        assert!(!c1.has(|e| matches!(e, NetEvent::Pick(77))));
        assert!(!c2.has(|e| matches!(e, NetEvent::Spawn { .. } | NetEvent::Msg(_))));
        // a late joiner learns what was already picked up
        let mut c3 = T::client(&code);
        wait(all!(h, c1, c2, c3), "c3", |t| t[3].has(|e| matches!(e, NetEvent::Pick(77))));
    }

    #[test]
    fn new_code_every_time_and_port_is_reusable() {
        let _g = lock();
        let mut n = Net::new();
        n.use_upnp = false;
        let mut codes = std::collections::HashSet::new();
        let mut first_port = None;
        for _ in 0..6 {
            n.host().unwrap();
            assert!(codes.insert(n.code.clone()), "room code was reused: {}", n.code);
            let port = n.port;
            assert_eq!(*first_port.get_or_insert(port), port, "port should be reusable straight away");
            // give the port some TIME_WAIT history
            let mut h = T { n, ev: vec![] };
            let mut c = T::client(&code_for(&h.n));
            wait(all!(h, c), "welcome", |t| welcomed(t[1]));
            n = std::mem::replace(&mut h.n, Net::new());
            drop(c);
            n.leave();
            assert!(n.code.is_empty() && n.inet_code.is_empty());
            drop(TcpListener::bind(("0.0.0.0", port)).expect("port should be free after leave()"));
        }
        assert_eq!(codes.len(), 6);
    }

    #[test]
    fn old_code_after_host_restart_gets_badcode() {
        let _g = lock();
        let mut h = T::host();
        let old = code_for(&h.n);
        h.n.host().unwrap();
        assert_ne!(old, code_for(&h.n));
        let mut c = T::client(&old);
        wait(all!(h, c), "badcode", |t| t[1].has(|e| matches!(e, NetEvent::Disconnected(_))));
        assert!(c.n.status.contains("new room"), "{}", c.n.status);
        assert_eq!(c.n.mode, Mode::Off);
        assert!(!h.has(|e| matches!(e, NetEvent::Join(_))));
        let mut c2 = T::client(&code_for(&h.n));
        wait(all!(h, c2), "new code works", |t| welcomed(t[1]));
    }

    #[test]
    fn version_mismatch() {
        let _g = lock();
        let mut h = T::host();
        let mut c = Net::new();
        c.join_with_version(&code_for(&h.n), VERSION + 1);
        let mut c = T { n: c, ev: vec![] };
        wait(all!(h, c), "badver", |t| t[1].has(|e| matches!(e, NetEvent::Disconnected(_))));
        assert!(c.n.status.contains("different version"), "{}", c.n.status);
        assert!(!h.has(|e| matches!(e, NetEvent::Join(_))));
        assert_eq!(h.n.names.iter().flatten().count(), 1);
    }

    #[test]
    fn look_sync() {
        let _g = lock();
        let mut h = T::host();
        h.n.send_look("1.2.3"); // before anyone joined
        h.n.send_look("not digits"); // ignored
        h.n.send_look(&"1".repeat(64)); // too long, ignored
        assert_eq!(h.n.looks[0].as_deref(), Some("1.2.3"));
        let code = code_for(&h.n);
        let mut c1 = T::client(&code);
        c1.n.send_look("4.5.6"); // before connecting: sent on welcome
        wait(all!(h, c1), "c1 looks", |t| {
            t[1].has(|e| matches!(e, NetEvent::Look(0, l) if l == "1.2.3")) && t[0].has(|e| matches!(e, NetEvent::Look(1, l) if l == "4.5.6"))
        });
        // live update reaches everybody, late joiner gets all stored looks
        let mut c2 = T::client(&code);
        wait(all!(h, c1, c2), "c2 looks", |t| {
            t[2].has(|e| matches!(e, NetEvent::Look(0, l) if l == "1.2.3")) && t[2].has(|e| matches!(e, NetEvent::Look(1, l) if l == "4.5.6"))
        });
        c2.n.send_look("7.8");
        h.n.send_look(&"9".repeat(63));
        wait(all!(h, c1, c2), "live looks", |t| {
            t[0].has(|e| matches!(e, NetEvent::Look(2, l) if l == "7.8"))
                && t[1].has(|e| matches!(e, NetEvent::Look(2, l) if l == "7.8"))
                && t[1].has(|e| matches!(e, NetEvent::Look(0, l) if l.len() == 63))
        });
        assert!(!c2.has(|e| matches!(e, NetEvent::Look(2, _))));
        assert!(!h.has(|e| matches!(e, NetEvent::Look(0, _))));
    }

    #[test]
    fn leave_and_host_stop() {
        let _g = lock();
        let mut h = T::host();
        let code = code_for(&h.n);
        let mut c1 = T::client(&code);
        wait(all!(h, c1), "c1", |t| welcomed(t[1]));
        let mut c2 = T::client(&code);
        wait(all!(h, c1, c2), "c2", |t| welcomed(t[2]));
        c1.n.leave();
        wait(all!(h, c2), "bye", |t| t[0].has(|e| matches!(e, NetEvent::Leave(1))) && t[1].has(|e| matches!(e, NetEvent::Leave(1))));
        assert!(c2.n.names[1].is_none());
        h.n.leave();
        wait(all!(c2), "host stop", |t| t[0].has(|e| matches!(e, NetEvent::Disconnected(_))));
        assert_eq!(c2.n.mode, Mode::Off);
        assert_eq!(c2.ev.iter().filter(|e| matches!(e, NetEvent::Disconnected(_))).count(), 1);
    }

    #[test]
    fn malformed_and_early_traffic() {
        let _g = lock();
        let mut h = T::host();
        let addr = SocketAddr::from(([127, 0, 0, 1], h.n.port));
        // junk before HELLO is ignored, HELLO split over two writes still works
        let mut raw = TcpStream::connect(addr).unwrap();
        raw.write_all(b"S 1 1 1 1\nMSG hi\nLOOK 1.2\nPK 5\n\xff\xfe\n").unwrap();
        pump(all!(h), 100);
        assert!(h.ev.is_empty());
        raw.write_all(b"HEL").unwrap();
        pump(all!(h), 50);
        raw.write_all(format!("LO {VERSION} 0\nS 9 nan inf 1\nLOOK 1.2 3\nLOOK ab\nCN x\n").as_bytes()).unwrap();
        wait(all!(h), "join", |t| t[0].has(|e| matches!(e, NetEvent::Join(1))));
        pump(all!(h), 100);
        assert!(h.has(|e| matches!(e, NetEvent::State(s) if s.id == 1 && s.pos.iter().all(|v| v.is_finite()))));
        assert!(!h.has(|e| matches!(e, NetEvent::Look(..) | NetEvent::Msg(_) | NetEvent::Pick(_) | NetEvent::Spawn { .. })));
        // half-open connections don't take slots
        let _idle: Vec<TcpStream> = (0..5).map(|_| TcpStream::connect(addr).unwrap()).collect();
        let code = code_for(&h.n);
        let mut c2 = T::client(&code);
        let mut c3 = T::client(&code);
        wait(all!(h, c2, c3), "two more fit", |t| welcomed(t[1]) && welcomed(t[2]));
        // connecting to nothing gives a readable error
        let dead_port = {
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let mut bad = T::client(&format!("127.0.0.1:{dead_port}"));
        wait(all!(h, bad), "refused", |t| t[1].n.mode == Mode::Off);
        assert!(bad.n.status.contains("Couldn't reach"), "{}", bad.n.status);
        let mut typo = T::client("zzzz");
        typo.poll();
        assert_eq!(typo.n.mode, Mode::Off);
        assert!(!typo.n.status.is_empty());
    }

    #[test]
    fn leave_and_drop_remove_router_mapping() {
        let _g = lock();
        let (host, rx) = crate::upnp::testsupport::fake_router(false);
        let get = || rx.recv_timeout(Duration::from_secs(5)).expect("router request");
        // leave() closes an existing mapping
        let mut n = Net::new();
        n.use_upnp = false;
        n.host().unwrap();
        n.mapping = Some(crate::upnp::testsupport::fake_mapping(&host, 47321));
        n.leave();
        assert_eq!(get(), "Delete");
        // so does a mapping that was still being negotiated when we left
        n.host().unwrap();
        let (tx, urx) = channel();
        n.upnp_rx = Some(urx);
        n.leave();
        tx.send(Ok(crate::upnp::testsupport::fake_mapping(&host, 47321))).unwrap();
        assert_eq!(get(), "Delete");
        // and dropping the Net (quitting the game) does it before returning
        n.host().unwrap();
        n.mapping = Some(crate::upnp::testsupport::fake_mapping(&host, 47321));
        drop(n);
        assert_eq!(rx.try_recv().unwrap(), "Delete");
    }

    #[test]
    fn silent_host_and_silent_client_time_out() {
        let _g = lock();
        // a "host" that accepts but never talks
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut c = T::client(&l.local_addr().unwrap().to_string());
        c.n.hello_timeout = Duration::from_millis(300);
        wait(all!(c), "client gives up", |t| t[0].has(|e| matches!(e, NetEvent::Disconnected(_))));
        assert!(c.n.status.contains("didn't answer"), "{}", c.n.status);
        // a client that says hello and then goes silent is dropped by the host
        let mut h = T::host();
        h.n.idle_timeout = Duration::from_millis(400);
        let mut raw = TcpStream::connect(("127.0.0.1", h.n.port)).unwrap();
        raw.write_all(format!("HELLO {VERSION} 0\n").as_bytes()).unwrap();
        wait(all!(h), "join", |t| t[0].has(|e| matches!(e, NetEvent::Join(1))));
        wait(all!(h), "idle leave", |t| t[0].has(|e| matches!(e, NetEvent::Leave(1))));
        drop(raw);
    }
}
