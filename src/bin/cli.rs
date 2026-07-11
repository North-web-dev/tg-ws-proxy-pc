//! tgwsproxy-cli — headless desktop/server front-end for the tg-ws-proxy engine.
//!
//! Part of tg-ws-proxy-pc (desktop fork of tg-ws-proxy-android).
//! Runs a local MTProto proxy that tunnels Telegram over Cloudflare WebSocket.

use std::time::Duration;

use clap::Parser;
use tgwsproxy::runner::{self, ProxyConfig};

/// Local MTProto proxy for Telegram over Cloudflare WebSocket (desktop CLI).
#[derive(Parser, Debug)]
#[command(name = "tgwsproxy-cli", version, about)]
struct Args {
    /// Bind address. Keep 127.0.0.1 so it never touches system routing / VPN.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Local port the Telegram client connects to.
    #[arg(long, default_value_t = 1443)]
    port: u16,

    /// 32-hex-char MTProto secret. Omit to generate a random one.
    #[arg(long, default_value = "")]
    secret: String,

    /// Cloudflare-WS domain override (advanced; blank = auto pool).
    #[arg(long, default_value = "")]
    cf_domain: String,

    /// Disable the Cloudflare-WS transport (direct DC connections only).
    #[arg(long)]
    no_cf: bool,

    /// DC IP overrides as a CIDR pool string (advanced; blank = defaults).
    #[arg(long, default_value = "")]
    dc_ips: String,

    /// WS pool size per datacenter (2..=16).
    #[arg(long, default_value_t = 4)]
    pool_size: i32,

    /// Cache directory for the cfproxy domain list.
    #[arg(long, default_value = "")]
    cache_dir: String,

    /// Verbose (debug) logging.
    #[arg(short, long)]
    verbose: bool,

    /// Print stats every N seconds (0 = off).
    #[arg(long, default_value_t = 30)]
    stats_interval: u64,
}

fn main() {
    let args = Args::parse();
    let cf_enabled = !args.no_cf;

    let cfg = ProxyConfig {
        host: args.host,
        port: args.port,
        secret: args.secret,
        dc_ips: args.dc_ips,
        cf_enabled,
        cf_domain: args.cf_domain,
        pool_size: args.pool_size,
        verbose: args.verbose,
        cache_dir: args.cache_dir,
    };

    let proxy = match runner::start(cfg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("failed to start proxy: {e}");
            std::process::exit(1);
        }
    };

    println!("\n  tg-ws-proxy-pc  ·  MTProto proxy for Telegram (desktop)\n");
    println!("  listening   : {}:{}", proxy.host, proxy.port);
    println!("  secret      : dd{}", proxy.secret);
    println!(
        "  transport   : {}",
        if cf_enabled {
            "Cloudflare-WS (+ direct fallback)"
        } else {
            "direct DC"
        }
    );
    println!("\n  Point Telegram Desktop → Settings → Advanced → Connection type");
    println!("  → Use custom proxy → MTProto, or just open this link:\n");
    println!("    {}", proxy.tg_link());
    println!("    {}\n", proxy.https_link());
    println!("  Press Ctrl+C to stop.\n");

    // A tiny current-thread runtime just to await Ctrl+C and tick stats; the
    // proxy engine runs on its own multi-thread runtime inside `proxy`.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build control runtime");

    rt.block_on(async {
        let mut tick = tokio::time::interval(Duration::from_secs(
            args.stats_interval.max(1),
        ));
        tick.tick().await; // consume the immediate first tick
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => break,
                _ = tick.tick() => {
                    if args.stats_interval > 0 {
                        println!("  [stats] {}", proxy.stats());
                    }
                }
            }
        }
    });

    println!("\n  stopping…");
    proxy.stop();
    println!("  bye.");
}
