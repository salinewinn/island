//! A very small UPnP (IGD) client: asks the home router to forward a TCP port to this PC and
//! reports the router's external address. Everything stays between this PC and the router.
//!
//! Nothing here may hang the game: every socket has a timeout, discovery has an overall deadline
//! and odd replies produce an error string instead of a panic. It is always run on a worker thread.

use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

const DISCOVERY_BUDGET: Duration = Duration::from_secs(7);
const MAX_REPLY: usize = 256 * 1024;

#[derive(Clone, Debug)]
pub struct Mapping {
    pub external_ip: Option<Ipv4Addr>,
    host: String,
    control: String,
    service: String,
    port: u16,
}

pub fn is_private(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private() || ip.is_loopback() || ip.is_link_local() || (o[0] == 100 && (64..=127).contains(&o[1])) || ip.is_unspecified()
}

/// "192.168.1.1:5000" or "192.168.1.1" (port 80). No DNS lookups.
fn resolve(host: &str) -> Result<SocketAddr, String> {
    if let Ok(a) = host.parse::<SocketAddr>() {
        return Ok(a);
    }
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return Ok(SocketAddr::from((ip, 80)));
    }
    Err(format!("bad router address '{host}'"))
}

/// Has a full HTTP reply (headers plus Content-Length bytes of body) arrived?
fn response_complete(buf: &[u8]) -> bool {
    let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") else { return false };
    let head = String::from_utf8_lossy(&buf[..pos]).to_ascii_lowercase();
    for line in head.lines() {
        if let Some(v) = line.strip_prefix("content-length:") {
            if let Ok(n) = v.trim().parse::<usize>() {
                return buf.len() >= pos + 4 + n;
            }
        }
    }
    false
}

/// One request/response exchange that always finishes within about `limit`.
fn http_exchange(addr: SocketAddr, request: &str, limit: Duration) -> Result<String, String> {
    let start = Instant::now();
    let limit = limit.max(Duration::from_millis(300));
    let mut s = TcpStream::connect_timeout(&addr, limit.min(Duration::from_secs(3))).map_err(|e| format!("can't connect to {addr} ({e})"))?;
    let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
    s.write_all(request.as_bytes()).map_err(|e| format!("can't talk to {addr} ({e})"))?;
    let mut out: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let left = limit.saturating_sub(start.elapsed());
        if left < Duration::from_millis(50) {
            break;
        }
        let _ = s.set_read_timeout(Some(left.min(Duration::from_secs(2))));
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                out.extend_from_slice(&buf[..n]);
                if out.len() > MAX_REPLY || response_complete(&out) {
                    break;
                }
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => {
                if out.is_empty() {
                    return Err(format!("no answer from {addr} ({e})"));
                }
                break;
            }
        }
    }
    if out.is_empty() {
        return Err(format!("no answer from {addr}"));
    }
    Ok(String::from_utf8_lossy(&out).to_string())
}

fn body_of(resp: &str) -> &str {
    resp.split_once("\r\n\r\n").map(|x| x.1).unwrap_or(resp)
}

fn tag<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let a = text.find(&open)? + open.len();
    let b = text[a..].find(&close)? + a;
    Some(text[a..b].trim())
}

fn status_ok(resp: &str) -> bool {
    let first = resp.lines().next().unwrap_or("");
    let mut it = first.split_whitespace();
    matches!(it.next(), Some(v) if v.starts_with("HTTP/1.")) && it.next() == Some("200")
}

fn soap(m: &Mapping, action: &str, args: &str) -> Result<String, String> {
    let body = format!(
        "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} xmlns:u=\"{svc}\">{args}</u:{action}></s:Body></s:Envelope>",
        svc = m.service
    );
    let req = format!(
        "POST {ctl} HTTP/1.1\r\nHOST: {host}\r\nContent-Type: text/xml; charset=\"utf-8\"\r\nSOAPACTION: \"{svc}#{action}\"\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
        ctl = m.control,
        host = m.host,
        svc = m.service,
        len = body.len()
    );
    let resp = http_exchange(resolve(&m.host)?, &req, Duration::from_secs(5))?;
    if status_ok(&resp) {
        Ok(resp)
    } else {
        match tag(&resp, "errorCode") {
            Some(code) => Err(format!("router said error {code}{}", tag(&resp, "errorDescription").map(|d| format!(" ({d})")).unwrap_or_default())),
            None => Err(format!("router said '{}'", resp.lines().next().unwrap_or("nothing").chars().take(60).collect::<String>())),
        }
    }
}

/// Reads a device description from `loc` and returns the mapping target if it has a WAN service.
fn try_location(loc: &str, limit: Duration) -> Result<Mapping, String> {
    let rest = loc.strip_prefix("http://").ok_or("not an http location")?;
    let (host, path) = match rest.split_once('/') {
        Some((h, p)) => (h.to_string(), format!("/{p}")),
        None => (rest.to_string(), "/".to_string()),
    };
    let addr = resolve(&host)?;
    match addr.ip() {
        std::net::IpAddr::V4(ip) if is_private(ip) => {}
        _ => return Err("gateway isn't on the local network".into()),
    }
    let req = format!("GET {path} HTTP/1.1\r\nHOST: {host}\r\nConnection: close\r\n\r\n");
    let resp = http_exchange(addr, &req, limit)?;
    let desc = body_of(&resp);
    for svc in [
        "urn:schemas-upnp-org:service:WANIPConnection:1",
        "urn:schemas-upnp-org:service:WANIPConnection:2",
        "urn:schemas-upnp-org:service:WANPPPConnection:1",
    ] {
        if let Some(i) = desc.find(&format!("<serviceType>{svc}</serviceType>")) {
            if let Some(ctl) = tag(&desc[i..], "controlURL") {
                let ctl = match ctl.strip_prefix("http://") {
                    Some(r) => r.find('/').map(|p| r[p..].to_string()).unwrap_or_else(|| "/".into()),
                    None if ctl.starts_with('/') => ctl.to_string(),
                    None => format!("/{ctl}"),
                };
                return Ok(Mapping { external_ip: None, host, control: ctl, service: svc.to_string(), port: 0 });
            }
        }
    }
    Err("no internet-gateway service in the device description".into())
}

fn find_gateway(local: Ipv4Addr) -> Result<Mapping, String> {
    let deadline = Instant::now() + DISCOVERY_BUDGET;
    // binding to the LAN address makes the multicast leave through the right network adapter
    let sock = UdpSocket::bind((local, 0)).or_else(|_| UdpSocket::bind("0.0.0.0:0")).map_err(|e| e.to_string())?;
    let _ = sock.set_multicast_ttl_v4(2);
    let targets = [
        "urn:schemas-upnp-org:service:WANIPConnection:1",
        "urn:schemas-upnp-org:device:InternetGatewayDevice:1",
        "upnp:rootdevice",
    ];
    let mut seen: Vec<String> = vec![];
    let mut last_err = String::from("no UPnP router answered");
    for st in targets {
        let req = format!("M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: {st}\r\n\r\n");
        let _ = sock.send_to(req.as_bytes(), "239.255.255.250:1900");
        let round_end = (Instant::now() + Duration::from_millis(1200)).min(deadline);
        let mut buf = [0u8; 2048];
        while Instant::now() < round_end && seen.len() < 16 {
            let left = round_end.saturating_duration_since(Instant::now()).max(Duration::from_millis(20));
            let _ = sock.set_read_timeout(Some(left.min(Duration::from_millis(400))));
            let n = match sock.recv_from(&mut buf) {
                Ok((n, _)) => n,
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => continue,
                Err(_) => break, // e.g. Windows reports ICMP errors here; try the next search type
            };
            let text = String::from_utf8_lossy(&buf[..n]).to_string();
            let loc = text.lines().find_map(|l| {
                let (k, v) = l.split_once(':')?;
                if k.trim().eq_ignore_ascii_case("location") {
                    Some(v.trim().to_string())
                } else {
                    None
                }
            });
            let Some(loc) = loc else { continue };
            if seen.contains(&loc) {
                continue;
            }
            seen.push(loc.clone());
            let left = deadline.saturating_duration_since(Instant::now());
            if left < Duration::from_millis(300) {
                return Err(last_err);
            }
            match try_location(&loc, left.min(Duration::from_secs(3))) {
                Ok(m) => return Ok(m),
                Err(e) => last_err = e,
            }
        }
        if Instant::now() >= deadline {
            break;
        }
    }
    if seen.is_empty() {
        Err("no UPnP router found".into())
    } else {
        Err(last_err)
    }
}

/// Opens `port` (TCP) on the router and learns the external address.
pub fn open_port(port: u16, local: Ipv4Addr) -> Result<Mapping, String> {
    if local.is_loopback() || local.is_unspecified() {
        return Err("no network connection".into());
    }
    map_port(find_gateway(local)?, port, local)
}

fn map_port(mut m: Mapping, port: u16, local: Ipv4Addr) -> Result<Mapping, String> {
    let mut last = String::from("router refused");
    // a finite lease first (cleans itself up if the game crashes); some routers only allow 0
    for lease in [86400u32, 0] {
        for attempt in 0..2 {
            let args = format!(
                "<NewRemoteHost></NewRemoteHost><NewExternalPort>{port}</NewExternalPort><NewProtocol>TCP</NewProtocol><NewInternalPort>{port}</NewInternalPort><NewInternalClient>{local}</NewInternalClient><NewEnabled>1</NewEnabled><NewPortMappingDescription>Island game</NewPortMappingDescription><NewLeaseDuration>{lease}</NewLeaseDuration>"
            );
            match soap(&m, "AddPortMapping", &args) {
                Ok(_) => {
                    m.port = port;
                    if let Ok(resp) = soap(&m, "GetExternalIPAddress", "") {
                        m.external_ip = tag(body_of(&resp), "NewExternalIPAddress").and_then(|s| s.parse().ok());
                    }
                    return Ok(m);
                }
                Err(e) => {
                    let conflict = e.contains("718");
                    last = e;
                    if conflict && attempt == 0 {
                        // a stale mapping from an earlier run: remove it and try again
                        m.port = port;
                        close_port(&m);
                        m.port = 0;
                        continue;
                    }
                    break;
                }
            }
        }
    }
    Err(last)
}

pub fn close_port(m: &Mapping) {
    if m.port == 0 {
        return;
    }
    let args = format!("<NewRemoteHost></NewRemoteHost><NewExternalPort>{}</NewExternalPort><NewProtocol>TCP</NewProtocol>", m.port);
    let _ = soap(m, "DeletePortMapping", &args);
}

fn parse_ip_reply(resp: &str) -> Result<Ipv4Addr, String> {
    if !status_ok(resp) {
        return Err("the lookup service refused".into());
    }
    body_of(resp).split_whitespace().find_map(|t| t.parse::<Ipv4Addr>().ok()).ok_or_else(|| "unexpected reply".to_string())
}

/// Explicit, user-requested lookup of the public address through a web service.
pub fn lookup_public_ip() -> Result<Ipv4Addr, String> {
    let addr = "api.ipify.org:80"
        .to_socket_addrs()
        .map_err(|e| format!("no internet? ({e})"))?
        .find(|a| a.is_ipv4())
        .ok_or_else(|| "no address for the lookup service".to_string())?;
    // HTTP/1.0 so the answer is never chunked
    let req = "GET / HTTP/1.0\r\nHost: api.ipify.org\r\nConnection: close\r\nUser-Agent: island-game\r\n\r\n";
    parse_ip_reply(&http_exchange(addr, req, Duration::from_secs(6))?)
}

/// A pretend router for tests (here and in net.rs).
#[cfg(test)]
pub mod testsupport {
    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc::{channel, Receiver};

    /// Returns (host:port, log of requests). `only_permanent` makes it refuse non-zero leases.
    pub fn fake_router(only_permanent: bool) -> (String, Receiver<String>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = l.local_addr().unwrap().to_string();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for conn in l.incoming() {
                let Ok(mut s) = conn else { continue };
                let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                let mut req = Vec::new();
                let mut buf = [0u8; 2048];
                loop {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => req.extend_from_slice(&buf[..n]),
                    }
                    let t = String::from_utf8_lossy(&req).to_string();
                    if let Some(p) = t.find("\r\n\r\n") {
                        let cl = t.to_ascii_lowercase().lines().find_map(|l| l.strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok())).unwrap_or(0);
                        if req.len() >= p + 4 + cl {
                            break;
                        }
                    }
                }
                let t = String::from_utf8_lossy(&req).to_string();
                let (status, body, note) = if t.starts_with("GET") {
                    (200, "<root><device><serviceList><service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>/ctl/IPConn</controlURL></service></serviceList></device></root>".to_string(), "GET".to_string())
                } else if t.contains("#AddPortMapping") {
                    let lease: u32 = tag(&t, "NewLeaseDuration").and_then(|v| v.parse().ok()).unwrap_or(0);
                    let note = format!("Add lease={lease}");
                    if only_permanent && lease != 0 {
                        (500, "<errorCode>725</errorCode><errorDescription>OnlyPermanentLeasesSupported</errorDescription>".to_string(), note)
                    } else {
                        (200, "<ok/>".to_string(), note)
                    }
                } else if t.contains("#GetExternalIPAddress") {
                    (200, "<NewExternalIPAddress>8.8.4.4</NewExternalIPAddress>".to_string(), "GetIP".to_string())
                } else if t.contains("#DeletePortMapping") {
                    (200, "<ok/>".to_string(), "Delete".to_string())
                } else {
                    (500, "".to_string(), "other".to_string())
                };
                let _ = tx.send(note);
                let reason = if status == 200 { "OK" } else { "Error" };
                let _ = s.write_all(format!("HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            }
        });
        (host, rx)
    }

    pub fn fake_mapping(host: &str, port: u16) -> Mapping {
        Mapping {
            external_ip: None,
            host: host.to_string(),
            control: "/ctl/IPConn".into(),
            service: "urn:schemas-upnp-org:service:WANIPConnection:1".into(),
            port,
        }
    }

    pub fn map(m: Mapping, port: u16) -> Result<Mapping, String> {
        map_port(m, port, Ipv4Addr::new(192, 168, 1, 5))
    }

    pub fn locate(host: &str) -> Result<Mapping, String> {
        try_location(&format!("http://{host}/desc.xml"), Duration::from_secs(3))
    }
}

#[cfg(test)]
mod tests {
    use super::testsupport::*;
    use super::*;

    fn next(rx: &std::sync::mpsc::Receiver<String>) -> String {
        rx.recv_timeout(Duration::from_secs(5)).expect("router request")
    }

    #[test]
    fn open_and_close_against_fake_router() {
        let (host, rx) = fake_router(false);
        let m = locate(&host).unwrap();
        assert_eq!(next(&rx), "GET");
        let m = map(m, 47321).unwrap();
        assert_eq!(next(&rx), "Add lease=86400");
        assert_eq!(next(&rx), "GetIP");
        assert_eq!(m.external_ip, Some(Ipv4Addr::new(8, 8, 4, 4)));
        close_port(&m);
        assert_eq!(next(&rx), "Delete");
    }

    #[test]
    fn falls_back_to_permanent_lease() {
        let (host, rx) = fake_router(true);
        let m = map(fake_mapping(&host, 0), 47321).unwrap();
        assert_eq!(next(&rx), "Add lease=86400");
        assert_eq!(next(&rx), "Add lease=0");
        assert_eq!(m.port, 47321);
    }

    #[test]
    fn odd_replies_do_not_panic_or_hang() {
        // silent device: the exchange gives up on time
        let silent = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let t = Instant::now();
        let r = try_location(&format!("http://{}/x.xml", silent.local_addr().unwrap()), Duration::from_secs(1));
        assert!(r.is_err() && t.elapsed() < Duration::from_secs(4));
        // junk / non-local / non-http locations
        assert!(try_location("ftp://1.2.3.4/x", Duration::from_secs(1)).is_err());
        assert!(try_location("http://8.8.8.8/x", Duration::from_secs(1)).is_err());
        assert!(try_location("http://example.com/x", Duration::from_secs(1)).is_err());
        assert!(tag("<a>héllo</b>", "a").is_none());
        assert_eq!(tag("<a> é </a>", "a"), Some("é"));
        assert!(parse_ip_reply("HTTP/1.1 200 OK\r\n\r\ngarbage").is_err());
        assert!(parse_ip_reply("HTTP/1.1 503 x\r\n\r\n1.2.3.4").is_err());
        assert_eq!(parse_ip_reply("HTTP/1.0 200 OK\r\nX: y\r\n\r\n1.2.3.4\n").unwrap(), Ipv4Addr::new(1, 2, 3, 4));
        assert!(is_private(Ipv4Addr::new(100, 64, 1, 1)) && !is_private(Ipv4Addr::new(8, 8, 8, 8)));
    }
}
