# Island relay server

Home routers block incoming connections, so two players in different countries can't connect
directly. The relay fixes that: the host and every joiner connect **out** to this small server,
which passes the bytes between them. It never reads game data and has no accounts.

With a relay set up, the host gets an **internet code** that works from anywhere in the world,
with no router settings.

## 1. Run it somewhere that is always on

It is one small program (`island-relay`) that listens on TCP port **47400** (or `PORT`).
You need a machine with a public IP address and that port open. Any of these work:

* a cheap or free-tier cloud VM (Oracle Cloud "Always Free", Google Cloud e2-micro, a few-pound
  VPS...). Open TCP 47400 in the provider's firewall / security rules and in the OS firewall.
* a spare PC at home with TCP 47400 forwarded on your router (then use your public IP or a free
  dynamic-DNS name such as DuckDNS).
* any host that runs Docker (see `Dockerfile` below).

Build and run (needs Rust: https://rustup.rs):

```
cd relay
cargo build --release
./target/release/island-relay          # listens on 47400
```

Linux service (systemd), save as `/etc/systemd/system/island-relay.service`:

```
[Unit]
Description=Island relay
After=network.target

[Service]
ExecStart=/opt/island-relay/island-relay
Restart=always
User=nobody

[Install]
WantedBy=multi-user.target
```

then `sudo systemctl enable --now island-relay`.

Docker: `docker build -t island-relay . && docker run -d --restart=always -p 47400:47400 island-relay`

## 2. Tell the game where it is

Open `src/net.rs` and set the address of that machine:

```rust
pub const RELAY_ADDR: &str = "relay.example.com:47400";
```

Rebuild, then publish a new version (see `../RELEASING.md`) so everyone's auto-updater gets it.
For quick tests you can instead set the environment variable `ISLAND_RELAY=host:47400` before
starting the game.

## How it works

* Host: opens a control connection `HOST <room>`; the relay answers `OK` and sends `NEW <id>`
  each time a player joins; the host opens a second connection `ACCEPT <room> <id>` per player.
* Joiner: connects and sends `JOIN <room>`; the relay pipes it to the host.
* Room ids are random and change every time someone hosts, so old codes stop working.
