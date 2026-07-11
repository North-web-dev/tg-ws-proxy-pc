//! Safe, native Rust API around the proxy core.
//!
//! The Android build drives the engine through the C ABI in `lib.rs` (JNI).
//! On desktop we don't need FFI at all — this module exposes an idiomatic Rust
//! handle that starts/stops the same `proxy::run_proxy` engine directly, so the
//! CLI and GUI front-ends share one code path.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

use crate::cfproxy;
use crate::config::{
    init_logging, CFPROXY, CFPROXY_ENABLED, POOL_SIZE, PROXY_SECRET, STATS,
};
use crate::proxy::{parse_cidr_pool, run_proxy, WsPool};

/// Everything needed to bring one proxy instance up.
#[derive(Clone, Debug)]
pub struct ProxyConfig {
    /// Bind address. Defaults to loopback so the proxy never touches system
    /// routing and cannot interfere with a VPN or expose itself to the LAN.
    pub host: String,
    pub port: u16,
    /// 32 hex chars (16 bytes). Empty ⇒ a random one is generated.
    pub secret: String,
    /// Optional DC IP overrides as a CIDR pool string (see `parse_cidr_pool`).
    pub dc_ips: String,
    /// Cloudflare-WS transport toggle + optional user domain.
    pub cf_enabled: bool,
    pub cf_domain: String,
    /// WS connection pool size per DC (clamped 2..=16 by the engine).
    pub pool_size: i32,
    pub verbose: bool,
    /// Directory for the cfproxy domain cache (empty ⇒ engine default).
    pub cache_dir: String,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        ProxyConfig {
            host: "127.0.0.1".to_string(),
            port: crate::config::DEFAULT_PORT,
            secret: String::new(),
            dc_ips: String::new(),
            cf_enabled: true,
            cf_domain: String::new(),
            pool_size: crate::config::DEFAULT_POOL_SZ,
            verbose: false,
            cache_dir: String::new(),
        }
    }
}

/// A live proxy instance. Dropping it (or calling [`RunningProxy::stop`]) shuts
/// the engine down gracefully.
pub struct RunningProxy {
    runtime: Runtime,
    cancel: CancellationToken,
    pool: Arc<WsPool>,
    handle: Option<tokio::task::JoinHandle<()>>,
    /// The secret actually in use (generated one if the caller passed none).
    pub secret: String,
    pub host: String,
    pub port: u16,
}

impl RunningProxy {
    /// The `tg://proxy?...` deep link a Telegram client can open to use us.
    pub fn tg_link(&self) -> String {
        tg_link(&self.host, self.port, &self.secret)
    }

    /// The `https://t.me/proxy?...` shareable link.
    pub fn https_link(&self) -> String {
        https_link(&self.host, self.port, &self.secret)
    }

    /// One-line English stats summary from the engine.
    pub fn stats(&self) -> String {
        STATS.summary()
    }

    /// Compact Russian stats summary (matches the Android status line).
    pub fn stats_ru(&self) -> String {
        STATS.summary_ru()
    }

    /// Graceful shutdown. Idempotent.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.cancel.cancel();
        if let Some(handle) = self.handle.take() {
            let pool = self.pool.clone();
            self.runtime.block_on(async move {
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;
                pool.close_all().await;
            });
        }
        STATS.reset();
    }
}

impl Drop for RunningProxy {
    fn drop(&mut self) {
        if self.handle.is_some() {
            self.shutdown();
        }
    }
}

/// Generate a fresh 32-hex-char MTProto secret (16 random bytes).
pub fn gen_secret() -> String {
    use rand::RngCore;
    let mut buf = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

/// Build a `tg://proxy` deep link. The `dd` prefix requests MTProto secured
/// (random-padding) mode, which is what this proxy speaks to the client.
pub fn tg_link(host: &str, port: u16, secret: &str) -> String {
    format!("tg://proxy?server={host}&port={port}&secret=dd{secret}")
}

/// Build a shareable `https://t.me/proxy` link.
pub fn https_link(host: &str, port: u16, secret: &str) -> String {
    format!("https://t.me/proxy?server={host}&port={port}&secret=dd{secret}")
}

fn valid_secret(s: &str) -> bool {
    s.len() == 32 && hex::decode(s).is_ok()
}

/// Start a proxy instance. Blocks only until the listener is bound (so a
/// returned `Ok` means the port is actually accepting connections), then the
/// engine runs on its own runtime threads.
pub fn start(mut cfg: ProxyConfig) -> std::io::Result<RunningProxy> {
    init_logging(cfg.verbose);

    // Secret: use the caller's if valid, otherwise mint one.
    if !valid_secret(&cfg.secret) {
        cfg.secret = gen_secret();
    }
    *PROXY_SECRET.write() = cfg.secret.clone();

    // Cloudflare-WS transport configuration.
    CFPROXY_ENABLED.store(cfg.cf_enabled, Ordering::Relaxed);
    {
        let mut cf = CFPROXY.write();
        cf.cache_dir = cfg.cache_dir.trim().to_string();
        if !cfg.cf_domain.trim().is_empty() {
            let d = cfg.cf_domain.trim().to_string();
            cf.user_domain = d.clone();
            cf.domains = vec![d.clone()];
            cf.active = d;
        }
    }
    cfproxy::clear_cfproxy_429_cooldowns();
    cfproxy::init_cfproxy_domains();

    // Pool size (engine re-clamps to 2..=16).
    POOL_SIZE.store(cfg.pool_size, Ordering::Relaxed);

    let dc_map: HashMap<i32, String> = parse_cidr_pool(&cfg.dc_ips);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_name("tgwsproxy-rt")
        .enable_all()
        .build()?;

    let cancel = CancellationToken::new();
    let pool = Arc::new(WsPool::new(cancel.clone()));

    // Bind synchronously on the runtime so we can report bind errors to the
    // caller before spawning the accept loop.
    let addr = format!("{}:{}", cfg.host, cfg.port);
    let listener = runtime.block_on(async { TcpListener::bind(&addr).await })?;

    let handle = {
        let pool = pool.clone();
        let host = cfg.host.clone();
        let cancel = cancel.clone();
        runtime.spawn(async move {
            if let Err(e) = run_proxy(pool, host, cfg.port, dc_map, cancel, listener).await {
                crate::config::log_error(&format!("run_proxy exited: {e}"));
            }
        })
    };

    Ok(RunningProxy {
        runtime,
        cancel,
        pool,
        handle: Some(handle),
        secret: cfg.secret,
        host: cfg.host,
        port: cfg.port,
    })
}
