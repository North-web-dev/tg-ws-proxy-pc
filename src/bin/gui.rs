//! tgwsproxy-gui — desktop GUI front-end for the tg-ws-proxy engine.
//!
//! Part of tg-ws-proxy-pc (desktop fork of tg-ws-proxy-android).
//! A small, self-contained egui/eframe app: configure, start/stop, copy the
//! Telegram proxy link and watch live traffic stats. Binds to loopback by
//! default so it never interferes with a system VPN or exposes itself.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::time::Duration;

use eframe::egui::{self, Color32, RichText};
use tgwsproxy::runner::{self, gen_secret, ProxyConfig, RunningProxy};

const ACCENT: Color32 = Color32::from_rgb(51, 144, 236); // Telegram blue
const OK_GREEN: Color32 = Color32::from_rgb(46, 160, 67);
const ERR_RED: Color32 = Color32::from_rgb(248, 81, 73);

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([460.0, 560.0])
            .with_min_inner_size([420.0, 520.0])
            .with_title("TG WS Proxy — PC"),
        ..Default::default()
    };
    eframe::run_native(
        "tg-ws-proxy-pc",
        options,
        Box::new(|cc| {
            install_style(&cc.egui_ctx);
            Ok(Box::<App>::default())
        }),
    )
}

fn install_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.widgets.noninteractive.rounding = 8.0.into();
    style.visuals.widgets.inactive.rounding = 8.0.into();
    style.visuals.widgets.hovered.rounding = 8.0.into();
    style.visuals.widgets.active.rounding = 8.0.into();
    style.visuals.selection.bg_fill = ACCENT.linear_multiply(0.5);
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    ctx.set_style(style);
}

struct App {
    // form state
    host: String,
    port: String,
    secret: String,
    cf_enabled: bool,
    cf_domain: String,
    pool_size: i32,
    verbose: bool,

    // runtime state
    proxy: Option<RunningProxy>,
    status: String,
    status_color: Color32,
    stats: String,
}

impl Default for App {
    fn default() -> Self {
        App {
            host: "127.0.0.1".to_string(),
            port: "1443".to_string(),
            secret: gen_secret(),
            cf_enabled: true,
            cf_domain: String::new(),
            pool_size: 4,
            verbose: false,
            proxy: None,
            status: "stopped".to_string(),
            status_color: Color32::GRAY,
            stats: String::new(),
        }
    }
}

impl App {
    fn is_running(&self) -> bool {
        self.proxy.is_some()
    }

    fn start(&mut self) {
        let port: u16 = match self.port.trim().parse() {
            Ok(p) if p > 0 => p,
            _ => {
                self.set_status("invalid port", ERR_RED);
                return;
            }
        };
        let cfg = ProxyConfig {
            host: self.host.trim().to_string(),
            port,
            secret: self.secret.trim().to_string(),
            dc_ips: String::new(),
            cf_enabled: self.cf_enabled,
            cf_domain: self.cf_domain.trim().to_string(),
            pool_size: self.pool_size,
            verbose: self.verbose,
            cache_dir: String::new(),
        };
        match runner::start(cfg) {
            Ok(p) => {
                // Reflect the effective secret (engine may have generated one).
                self.secret = p.secret.clone();
                self.proxy = Some(p);
                self.set_status("running", OK_GREEN);
            }
            Err(e) => self.set_status(&format!("start failed: {e}"), ERR_RED),
        }
    }

    fn stop(&mut self) {
        if let Some(p) = self.proxy.take() {
            p.stop();
        }
        self.stats.clear();
        self.set_status("stopped", Color32::GRAY);
    }

    fn set_status(&mut self, s: &str, c: Color32) {
        self.status = s.to_string();
        self.status_color = c;
    }

    fn tg_link(&self) -> String {
        let port = self.port.trim().parse().unwrap_or(1443);
        runner::tg_link(self.host.trim(), port, self.secret.trim())
    }

    fn copy(&self, text: &str) {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(text.to_string());
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll live stats while running.
        if let Some(p) = &self.proxy {
            self.stats = p.stats_ru();
            ctx.request_repaint_after(Duration::from_secs(1));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading(RichText::new("🛡 TG WS Proxy").color(ACCENT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("PC edition").weak());
                });
            });
            ui.label(RichText::new("MTProto proxy for Telegram over Cloudflare WebSocket").weak().small());
            ui.separator();

            // ---- status row ----
            ui.horizontal(|ui| {
                ui.label("Status:");
                ui.label(RichText::new(&self.status).color(self.status_color).strong());
            });
            if !self.stats.is_empty() {
                ui.label(RichText::new(&self.stats).monospace().small());
            }
            ui.add_space(4.0);

            let running = self.is_running();

            // ---- config (locked while running) ----
            ui.add_enabled_ui(!running, |ui| {
                egui::Grid::new("cfg").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                    ui.label("Bind host");
                    ui.text_edit_singleline(&mut self.host);
                    ui.end_row();

                    ui.label("Local port");
                    ui.text_edit_singleline(&mut self.port);
                    ui.end_row();

                    ui.label("Secret");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.secret).desired_width(200.0));
                        if ui.button("🎲").on_hover_text("Generate new secret").clicked() {
                            self.secret = gen_secret();
                        }
                    });
                    ui.end_row();

                    ui.label("WS pool size");
                    ui.add(egui::Slider::new(&mut self.pool_size, 2..=16));
                    ui.end_row();
                });
                ui.add_space(2.0);
                ui.checkbox(&mut self.cf_enabled, "Cloudflare-WS transport (recommended)");
                ui.add_enabled_ui(self.cf_enabled, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("CF domain");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.cf_domain)
                                .hint_text("(optional — auto if blank)")
                                .desired_width(240.0),
                        );
                    });
                });
                ui.checkbox(&mut self.verbose, "Verbose logging");
            });

            ui.add_space(8.0);

            // ---- start / stop ----
            ui.vertical_centered_justified(|ui| {
                if running {
                    if ui
                        .button(RichText::new("■  Stop").size(16.0).color(ERR_RED))
                        .clicked()
                    {
                        self.stop();
                    }
                } else if ui
                    .button(RichText::new("▶  Start").size(16.0).color(OK_GREEN))
                    .clicked()
                {
                    self.start();
                }
            });

            // ---- link + copy (only meaningful while running) ----
            if running {
                ui.add_space(8.0);
                ui.separator();
                ui.label(RichText::new("Connect Telegram with this link:").small().weak());
                let link = self.tg_link();
                ui.add(
                    egui::TextEdit::singleline(&mut link.clone())
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
                ui.horizontal(|ui| {
                    if ui.button("📋 Copy tg:// link").clicked() {
                        self.copy(&link);
                    }
                    if ui.button("📋 Copy https link").clicked() {
                        let port = self.port.trim().parse().unwrap_or(1443);
                        self.copy(&runner::https_link(self.host.trim(), port, self.secret.trim()));
                    }
                });
                ui.label(
                    RichText::new(
                        "In Telegram: Settings → Advanced → Connection type → \
                         Custom proxy → MTProto (host/port/secret above).",
                    )
                    .small()
                    .weak(),
                );
            }

            // ---- footer ----
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Loopback-only bind — does not touch system routing / VPN.")
                        .small()
                        .weak(),
                );
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(p) = self.proxy.take() {
            p.stop();
        }
    }
}
