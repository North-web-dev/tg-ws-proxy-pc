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

Download a prebuilt binary from [Releases][releases], build from source, or use
Nix (click a section below to expand it).

<details>
<summary><b>Build from source</b></summary>

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

</details>

<details>
<summary><b>Nix / NixOS</b></summary>

The repository ships a `flake.nix` with two packages (`cli`, `gui`), matching
apps, a dev shell, and a NixOS module.

<details>
<summary>Run or install with the flake</summary>

```bash
# Run without installing
nix run github:North-web-dev/tg-ws-proxy-pc              # CLI
nix run github:North-web-dev/tg-ws-proxy-pc#gui          # GUI
nix run github:North-web-dev/tg-ws-proxy-pc -- --port 2443 -v

# Install into your profile
nix profile install github:North-web-dev/tg-ws-proxy-pc
nix profile install github:North-web-dev/tg-ws-proxy-pc#gui

# Build from a checkout
nix build .#cli
nix build .#gui

# Development shell (Rust toolchain + GUI libraries)
nix develop
```

The `cli` package has no GUI or OpenSSL dependencies; its runtime closure is
just the binary and glibc.

</details>

<details>
<summary>Binary cache (skip building from source)</summary>

Prebuilt packages are available from Cachix. With the flake's NixOS module or in
`configuration.nix`:

```nix
nix.settings = {
  substituters = [
    "https://santg3ar1a.cachix.org"
  ];
  trusted-public-keys = [
    "santg3ar1a.cachix.org-1:fFHsUyTT2bX0OcS+vF3FPPULqZJlFI1B9E2gZcGjxGs="
  ];
};
```

On non-NixOS, add to `/etc/nix/nix.conf`:

```ini
extra-substituters = https://santg3ar1a.cachix.org
extra-trusted-public-keys = santg3ar1a.cachix.org-1:fFHsUyTT2bX0OcS+vF3FPPULqZJlFI1B9E2gZcGjxGs=
```

Apply the cache settings (`nixos-rebuild switch`) **before** installing the
package, otherwise Nix will build it locally.

</details>

<details>
<summary>NixOS module (flake-based configuration)</summary>

Add the flake as an input and import the module:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    tg-ws-proxy.url = "github:North-web-dev/tg-ws-proxy-pc";
    tg-ws-proxy.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { nixpkgs, tg-ws-proxy, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        tg-ws-proxy.nixosModules.default
        {
          services.tg-ws-proxy-rs = {
            enable = true;
            port = 1443;
            secret = "/var/lib/secrets/tg-ws-proxy-rs";
          };
        }
      ];
    };
  };
}
```

Create the secret file once (32 hex characters):

```bash
sudo install -d -m 700 /var/lib/secrets
nix run nixpkgs#openssl -- rand -hex 16 | sudo tee /var/lib/secrets/tg-ws-proxy-rs >/dev/null
sudo chmod 600 /var/lib/secrets/tg-ws-proxy-rs
```

Apply and get the link for Telegram from the journal:

```bash
sudo nixos-rebuild switch --flake .#myhost
systemctl status tg-ws-proxy-rs
journalctl -u tg-ws-proxy-rs -n 30 | grep tg://
```

</details>

<details>
<summary>NixOS module (classic <code>configuration.nix</code>, no flakes)</summary>

Fetch the flake with `builtins.getFlake`. Pin a commit so it also works in pure
evaluation:

```nix
{ config, pkgs, ... }:
let
  tg-ws-proxy = builtins.getFlake "github:North-web-dev/tg-ws-proxy-pc/<commit-sha>";
in
{
  imports = [ tg-ws-proxy.nixosModules.default ];

  services.tg-ws-proxy-rs = {
    enable = true;
    port = 1443;
    
    # nix run nixpkgs#openssl -- rand -hex 16
    secret = "/var/lib/secrets/tg-ws-proxy-rs"; 
  };
}
```

For a local checkout use `builtins.getFlake "git+file:///path/to/tg-ws-proxy-pc"`
(files must be tracked by git; use `--impure` if your system is built with
flakes).

</details>

<details>
<summary>Module options (<code>services.tg-ws-proxy-rs</code>)</summary>

Options with a CLI flag map to it one-to-one; `enable`, `package` and
`openFirewall` only configure the NixOS service.

| Option          | Type               | Default                    | CLI flag           |
|-----------------|--------------------|----------------------------|--------------------|
| `enable`        | bool               | `false`                    | —                  |
| `package`       | package            | flake's `cli` package      | —                  |
| `host`          | string             | `"127.0.0.1"`              | `--host`           |
| `port`          | port               | `1443`                     | `--port`           |
| `openFirewall`  | bool               | `false`                    | —                  |
| `secret`        | null or path (str) | `null` (random each start) | `--secret`         |
| `cfDomain`      | null or string     | `null`                     | `--cf-domain`      |
| `noCf`          | bool               | `false`                    | `--no-cf`          |
| `dcIps`         | null or string     | `null`                     | `--dc-ips`         |
| `poolSize`      | int 2..16          | `4`                        | `--pool-size`      |
| `cacheDir`      | string             | `/var/lib/tg-ws-proxy-rs`  | `--cache-dir`      |
| `statsInterval` | unsigned int       | `30`                       | `--stats-interval` |
| `verbose`       | bool               | `false`                    | `--verbose`        |

`secret` must be an **absolute path to a file** containing the secret. The file
is loaded as a systemd credential, so the secret never enters the Nix store or
the process command line. If it is `null`, a new random secret (and therefore a
new link) is generated on every start.

The service runs under `DynamicUser` with a hardened systemd sandbox. Ports
below 1024 are supported through `CAP_NET_BIND_SERVICE`.

</details>

</details>

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

# Secret from a file (a leading `dd` from tg:// links is accepted too):
tgwsproxy-cli --secret /path/to/secret-file

# Direct datacenter connections only (no Cloudflare-WS transport):
tgwsproxy-cli --no-cf
```

`--secret` takes 32 hex characters or a path to a file containing them. An
invalid value is an error; the proxy never silently replaces it with a random
one.

It prints the `tg://proxy?...` and `https://t.me/proxy?...` links to paste into
Telegram, and traffic stats on an interval. `Ctrl+C` stops it cleanly.

Run `tgwsproxy-cli --help` for all flags.

## Project layout

```
flake.nix                          Nix packages, apps, dev shell, NixOS module export
modules/nixos/default.nix          NixOS module (services.tg-ws-proxy-rs)
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