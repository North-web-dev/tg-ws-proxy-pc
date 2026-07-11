# tg-ws-proxy-pc

[![build](https://github.com/North-web-dev/tg-ws-proxy-pc/actions/workflows/build.yml/badge.svg)](https://github.com/North-web-dev/tg-ws-proxy-pc/actions/workflows/build.yml)
[![release](https://img.shields.io/github/v/release/North-web-dev/tg-ws-proxy-pc?sort=date&display_name=tag)](https://github.com/North-web-dev/tg-ws-proxy-pc/releases)
[![license](https://img.shields.io/badge/license-GPL--3.0-blue)](LICENSE)
[![platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-informational)](#install)
[![rust](https://img.shields.io/badge/rust-1.75%2B-000000?logo=rust)](https://rustup.rs)

**Desktop edition** (Windows / Linux / macOS) of the [tg-ws-proxy][upstream]
MTProto proxy for Telegram. It runs a local proxy on your machine and tunnels
Telegram's traffic over Cloudflare WebSocket connections, which helps Telegram
connect on networks where its data-centre IPs are throttled or filtered.

Ships as a single native binary in two forms:

- **`tgwsproxy-gui`** — a small desktop app (egui): configure, start/stop, copy
  the connection link, watch live stats.
- **`tgwsproxy-cli`** — headless, for servers or scripting.

> Fork of [`amurcanov/tg-ws-proxy-android`][android]. Same Rust engine, desktop
> front-ends instead of the Android UI. See [NOTICE](NOTICE) for the full
> lineage and licensing.

## How it works

The Telegram client is pointed at a **local MTProto proxy** — this program,
listening on `127.0.0.1:1443` by default. The client speaks the normal MTProto
proxy protocol to it; nothing else on the system is reconfigured.

For each client connection the proxy:

1. Reads the MTProto handshake and extracts the target **datacenter ID**.
2. Opens a transport to that datacenter. The preferred transport is a
   **Cloudflare WebSocket** relay (`cfproxy`): the MTProto stream is wrapped in
   a WebSocket that terminates on Cloudflare's edge, so the outbound connection
   looks like ordinary HTTPS to a CDN host rather than a direct hit on a known
   Telegram IP. If that path fails it **falls back to a direct TCP connection**
   to the datacenter.
3. **Bridges** the two sides, relaying frames in both directions until either
   end closes.

A small **connection pool** keeps warm WebSocket connections per datacenter to
cut latency, and a **balancer** rotates over the available Cloudflare relay
domains (refreshed periodically from the upstream domain list).

```
Telegram Desktop ──MTProto──► 127.0.0.1:1443 (this proxy)
                                     │
                    ┌────────────────┴───────────────┐
                    ▼                                 ▼
        Cloudflare WebSocket relay          direct TCP (fallback)
                    │                                 │
                    └────────────────┬────────────────┘
                                     ▼
                          Telegram datacenter
```

Because it only binds to loopback, it does **not** change system routing and
will not interfere with a VPN running alongside it.

## Install

Download a prebuilt binary from [Releases][releases], or build from source.

### Build from source

Requires a [Rust toolchain](https://rustup.rs) (1.75+).

```bash
git clone https://github.com/North-web-dev/tg-ws-proxy-pc
cd tg-ws-proxy-pc

# CLI (no system GUI libraries needed):
cargo build --release --bin tgwsproxy-cli

# GUI (needs desktop GL/X11 or Wayland libs on Linux):
cargo build --release --features gui --bin tgwsproxy-gui
```

Binaries land in `target/release/`.

On Linux the GUI build needs a few dev packages, e.g. on Debian/Ubuntu:

```bash
sudo apt install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
                 libxkbcommon-dev libgl1-mesa-dev libwayland-dev
```

## Usage

### GUI

Launch `tgwsproxy-gui`, press **Start**, then **Copy tg:// link** and open it —
Telegram picks up the proxy automatically. Or add it by hand in Telegram:
*Settings → Advanced → Connection type → Custom proxy → MTProto* with the host,
port and secret shown in the app.

### CLI

```bash
# Start with a generated secret on 127.0.0.1:1443:
tgwsproxy-cli

# Custom port, fixed secret, verbose:
tgwsproxy-cli --port 2443 --secret 00112233445566778899aabbccddeeff -v

# Direct datacenter connections only (no Cloudflare-WS transport):
tgwsproxy-cli --no-cf
```

It prints the `tg://proxy?...` and `https://t.me/proxy?...` links to paste into
Telegram, and traffic stats on an interval. `Ctrl+C` stops it cleanly.

Run `tgwsproxy-cli --help` for all flags.

## Project layout

```
src/
  proxy.rs, ws.rs, cfproxy.rs,     shared Rust engine (from upstream)
  crypto.rs, balancer.rs, config.rs
  runner.rs                        native Rust API around the engine (this fork)
  lib.rs                           library root + Android/JNI C ABI
  bin/cli.rs                       headless CLI front-end (this fork)
  bin/gui.rs                       egui GUI front-end (this fork)
```

## License

GPL-3.0-only. See [LICENSE](LICENSE) and [NOTICE](NOTICE).

Not affiliated with Telegram or Cloudflare.

[upstream]: https://github.com/Flowseal/tg-ws-proxy
[android]: https://github.com/amurcanov/tg-ws-proxy-android
[releases]: https://github.com/North-web-dev/tg-ws-proxy-pc/releases
