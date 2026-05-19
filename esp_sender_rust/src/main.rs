#![windows_subsystem = "windows"]

use std::io::Write;
use std::time::{Duration, Instant};
use std::process::Command;
use std::os::windows::process::CommandExt;
use std::sync::{Arc, Mutex};
use std::thread;
use std::net::UdpSocket;
use std::fs;
use sysinfo::{System, Networks, Disks};
use tray_icon::{TrayIconBuilder, Icon, menu::{Menu, MenuItem, PredefinedMenuItem, MenuEvent}};
use chrono::Local;
use serde::{Deserialize, Serialize};
use salah::prelude::*;
use wmi::WMIConnection;
use mdns_sd::{ServiceDaemon, ServiceEvent};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use eframe::egui;
use winit::platform::windows::EventLoopBuilderExtWindows;

// WinAPI
use winapi::um::winuser::{ShowWindow, SW_HIDE, SW_SHOW, GetForegroundWindow, GetWindowTextW, PeekMessageW, TranslateMessage, DispatchMessageW, MSG, PM_REMOVE};
use winapi::um::wincon::{GetConsoleWindow};
use winapi::um::consoleapi::{AllocConsole};

const SETTINGS_FILE: &str = "settings.json";
const LOCAL_IP: &str = "0.0.0.0:1235"; 
const CREATE_NO_WINDOW: u32 = 0x08000000;

type HmacSha256 = Hmac<Sha256>;
type LogBuffer = Arc<Mutex<Vec<String>>>;

fn log_msg(buffer: &LogBuffer, msg: &str) {
    let entry = format!("[{}] {}", Local::now().format("%Y-%m-%d %H:%M:%S"), msg);
    if let Ok(mut buf) = buffer.lock() {
        buf.push(entry.clone());
    }
    // Prints to console if one is attached, no-op otherwise
    println!("{}", entry);
}

#[derive(Serialize, Deserialize, Clone)]
struct AppConfig {
    pub com_port: String,
    pub baud_rate: u32,
    pub esp_ip: String,
    pub auto_start: bool,
    pub interval_ms: u64,
    #[serde(default)]
    pub hmac_key: String,
    #[serde(default = "default_temp_limit")]
    pub cpu_temp_limit: u32,
    #[serde(default = "default_temp_limit")]
    pub gpu_temp_limit: u32,
    #[serde(default = "default_latitude")]
    pub latitude: f64,
    #[serde(default = "default_longitude")]
    pub longitude: f64,
    #[serde(default = "default_prayer_method")]
    pub prayer_method: u32, // 0=Dubai, 1=UmmAlQura, 2=MuslimWorldLeague, 3=ISNA, 4=Egyptian
    #[serde(default = "default_madhab")]
    pub madhab: u32, // 0=Shafi, 1=Hanafi
}

fn default_temp_limit() -> u32 { 85 }
fn default_latitude() -> f64 { 24.4539 }  // Abu Dhabi default
fn default_longitude() -> f64 { 54.3773 } // Abu Dhabi default
fn default_prayer_method() -> u32 { 0 } // Default to Dubai
fn default_madhab() -> u32 { 0 } // Default to Shafi

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            com_port: "COM10".to_string(),
            baud_rate: 115200,
            esp_ip: "auto".to_string(),
            auto_start: false,
            interval_ms: 800,
            hmac_key: String::new(),
            cpu_temp_limit: 85,
            gpu_temp_limit: 85,
            latitude: 24.4539,
            longitude: 54.3773,
            prayer_method: 0,
            madhab: 0,
        }
    }
}

impl AppConfig {
    fn load() -> Self {
        if let Ok(data) = fs::read_to_string(SETTINGS_FILE) {
            if let Ok(config) = serde_json::from_str(&data) { return config; }
        }
        let default = AppConfig::default();
        let _ = default.save();
        default
    }
    fn save(&self) -> std::io::Result<()> {
        let data = serde_json::to_string_pretty(self).unwrap();
        fs::write(SETTINGS_FILE, data)
    }
    fn update_autostart(&self) {
        let exe_path = std::env::current_exe().unwrap();
        let exe_str = exe_path.to_str().unwrap();
        if self.auto_start {
            let _ = Command::new("reg")
                .args(["add", "HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", "EspSender", "/t", "REG_SZ", "/d", &format!("\"{}\"", exe_str), "/f"])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        } else {
            let _ = Command::new("reg")
                .args(["delete", "HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", "EspSender", "/f"])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        }
    }
}

fn open_settings_ui(config_mutex: Arc<Mutex<AppConfig>>) {
    thread::spawn(move || {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([440.0, 560.0])
                .with_title("Esp_Mon — Settings")
                .with_resizable(false),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        };

        let cfg = config_mutex.lock().unwrap().clone();

        let _ = eframe::run_native(
            "Esp_Mon — Settings",
            options,
            Box::new(|_cc| Ok(Box::new(SettingsApp {
                config: cfg,
                config_mutex: Arc::clone(&config_mutex),
                saved_flash: None,
            }))),
        );
    });
}

struct SettingsApp {
    config: AppConfig,
    config_mutex: Arc<Mutex<AppConfig>>,
    saved_flash: Option<Instant>,
}

impl SettingsApp {
    fn section_header(ui: &mut egui::Ui, icon: &str, title: &str, color: egui::Color32) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(icon).size(16.0).color(color));
            ui.label(egui::RichText::new(title).size(15.0).strong().color(color));
        });
        ui.add_space(2.0);
    }
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut visuals = egui::Visuals::dark();
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(30, 30, 40);
        ctx.set_visuals(visuals);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(22, 22, 32)).inner_margin(20.0))
            .show(ctx, |ui| {

            // Title bar
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("⚙").size(22.0).color(egui::Color32::from_rgb(100, 180, 255)));
                ui.label(egui::RichText::new("Esp_Mon Settings").size(20.0).strong().color(egui::Color32::WHITE));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("v2.0").size(12.0).color(egui::Color32::from_rgb(120, 120, 140)));
                });
            });
            ui.add_space(4.0);
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {

            // ── Connection Section ──
            SettingsApp::section_header(ui, "📡", "Connection", egui::Color32::from_rgb(100, 200, 130));
            egui::Grid::new("conn_grid").num_columns(2).spacing([16.0, 8.0]).min_col_width(130.0).show(ui, |ui| {
                ui.label("ESP IP Address");
                let resp = ui.add_sized([200.0, 22.0], egui::TextEdit::singleline(&mut self.config.esp_ip));
                resp.on_hover_text("Use 'auto' for mDNS discovery, or set IP:port manually");
                ui.end_row();

                ui.label("COM Port");
                ui.add_sized([200.0, 22.0], egui::TextEdit::singleline(&mut self.config.com_port));
                ui.end_row();

                ui.label("Baud Rate");
                ui.add(egui::DragValue::new(&mut self.config.baud_rate).range(9600..=921600).speed(100));
                ui.end_row();

                ui.label("Refresh Interval");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut self.config.interval_ms, 100..=5000).suffix(" ms"));
                });
                ui.end_row();
            });

            ui.add_space(4.0);
            ui.separator();

            // ── Monitoring Section ──
            SettingsApp::section_header(ui, "🌡", "Thermal Limits", egui::Color32::from_rgb(255, 160, 70));
            egui::Grid::new("temp_grid").num_columns(2).spacing([16.0, 8.0]).min_col_width(130.0).show(ui, |ui| {
                ui.label("CPU Temp Limit");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut self.config.cpu_temp_limit, 50..=105).suffix(" °C"));
                });
                ui.end_row();

                ui.label("GPU Temp Limit");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut self.config.gpu_temp_limit, 50..=105).suffix(" °C"));
                });
                ui.end_row();
            });
            ui.add_space(2.0);
            ui.label(egui::RichText::new("  CYD display will flash red when temps exceed these limits.").size(11.0).color(egui::Color32::from_rgb(140, 140, 160)));

            ui.add_space(4.0);
            ui.separator();

            // ── Security Section ──
            SettingsApp::section_header(ui, "🔒", "Security", egui::Color32::from_rgb(200, 130, 255));
            egui::Grid::new("sec_grid").num_columns(2).spacing([16.0, 8.0]).min_col_width(130.0).show(ui, |ui| {
                ui.label("HMAC Key");
                let resp = ui.add_sized([200.0, 22.0], egui::TextEdit::singleline(&mut self.config.hmac_key).password(true));
                resp.on_hover_text("Shared secret for packet authentication. Auto-generated if blank.");
                ui.end_row();
            });
            ui.add_space(2.0);
            ui.label(egui::RichText::new("  Packets are HMAC-SHA256 signed to prevent spoofing.").size(11.0).color(egui::Color32::from_rgb(140, 140, 160)));

            ui.add_space(4.0);
            ui.separator();

            // ── Prayer Times Section ──
            SettingsApp::section_header(ui, "🕌", "Prayer Times", egui::Color32::from_rgb(80, 200, 180));
            egui::Grid::new("prayer_grid").num_columns(2).spacing([16.0, 8.0]).min_col_width(130.0).show(ui, |ui| {
                ui.label("Latitude");
                ui.add(egui::DragValue::new(&mut self.config.latitude).range(-90.0..=90.0).speed(0.01).max_decimals(4));
                ui.end_row();

                ui.label("Longitude");
                ui.add(egui::DragValue::new(&mut self.config.longitude).range(-180.0..=180.0).speed(0.01).max_decimals(4));
                ui.end_row();

                ui.label("Method");
                egui::ComboBox::from_id_salt("prayer_method").selected_text(match self.config.prayer_method {
                    0 => "UAE (Abu Dhabi)", 1 => "Umm Al-Qura (KSA)", 2 => "Muslim World League", 3 => "ISNA (North America)", _ => "Egyptian"
                }).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.config.prayer_method, 0, "UAE (Abu Dhabi)");
                    ui.selectable_value(&mut self.config.prayer_method, 1, "Umm Al-Qura (KSA)");
                    ui.selectable_value(&mut self.config.prayer_method, 2, "Muslim World League");
                    ui.selectable_value(&mut self.config.prayer_method, 3, "ISNA (North America)");
                    ui.selectable_value(&mut self.config.prayer_method, 4, "Egyptian");
                });
                ui.end_row();

                ui.label("Madhab (Asr)");
                egui::ComboBox::from_id_salt("prayer_madhab").selected_text(match self.config.madhab {
                    0 => "Shafi / Maliki / Hanbali", _ => "Hanafi"
                }).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.config.madhab, 0, "Shafi / Maliki / Hanbali");
                    ui.selectable_value(&mut self.config.madhab, 1, "Hanafi");
                });
                ui.end_row();
            });

            ui.add_space(4.0);
            ui.separator();

            // ── System Section ──
            SettingsApp::section_header(ui, "💻", "System", egui::Color32::from_rgb(100, 180, 255));
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.checkbox(&mut self.config.auto_start, "Launch with Windows");
            });
            ui.add_space(2.0);
            ui.label(egui::RichText::new("  Registers Esp_Mon in the Windows startup registry.").size(11.0).color(egui::Color32::from_rgb(140, 140, 160)));

            }); // end ScrollArea

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            // ── Action Buttons ──
            ui.horizontal(|ui| {
                let save_btn = ui.add_sized([120.0, 32.0], egui::Button::new(
                    egui::RichText::new("💾  Save").size(14.0).strong()
                ).fill(egui::Color32::from_rgb(40, 120, 70)));

                if save_btn.clicked() {
                    let _ = self.config.save();
                    self.config.update_autostart();
                    if let Ok(mut c) = self.config_mutex.lock() {
                        *c = self.config.clone();
                    }
                    self.saved_flash = Some(Instant::now());
                }

                ui.add_space(8.0);

                let cancel_btn = ui.add_sized([120.0, 32.0], egui::Button::new(
                    egui::RichText::new("✖  Cancel").size(14.0)
                ).fill(egui::Color32::from_rgb(70, 40, 40)));

                if cancel_btn.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                // Inline save confirmation
                if let Some(t) = self.saved_flash {
                    if t.elapsed() < Duration::from_secs(3) {
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new("✅ Settings saved!").size(13.0).color(egui::Color32::from_rgb(100, 230, 120)));
                        ctx.request_repaint();
                    } else {
                        self.saved_flash = None;
                    }
                }
            });
        });
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct Sensor { name: String, value: f32 }

struct HardwareData { cpu_temp: f32, gpu_temp: f32, gpu_usage: f32, vram_usage: f32 }

/// Prayer time data computed locally using Umm Al-Qura method (same as UAE/KSA gov)
struct PrayerData {
    next_name: String,
    countdown_secs: i64,
}

fn calculate_prayer_info(config: &AppConfig) -> PrayerData {
    let coords = Coordinates::new(config.latitude, config.longitude);
    let now = Local::now();
    let date = now.date_naive();
    
    let method = match config.prayer_method {
        0 => Method::Dubai,
        1 => Method::UmmAlQura,
        2 => Method::MuslimWorldLeague,
        3 => Method::NorthAmerica,
        _ => Method::Egyptian,
    };
    
    let madhab = match config.madhab {
        0 => Madhab::Shafi,
        _ => Madhab::Hanafi,
    };
    
    let params = Configuration::with(method, madhab);

    let prayers = match PrayerSchedule::new()
        .on(date)
        .for_location(coords)
        .with_configuration(params)
        .calculate() {
            Ok(p) => p,
            Err(_) => return PrayerData { next_name: String::new(), countdown_secs: 0 },
        };

    let next = prayers.next();
    let next_time = prayers.time(next);
    let now_utc = chrono::Utc::now();
    let diff = next_time.signed_duration_since(now_utc);
    let countdown_secs = diff.num_seconds().max(0);

    // Map prayer names — skip Sunrise/Qiyam/FajrTomorrow for display
    let next_name = match next {
        Prayer::Fajr | Prayer::FajrTomorrow => "Fajr".to_string(),
        Prayer::Sunrise => "Sunrise".to_string(),
        Prayer::Dhuhr => "Dhuhr".to_string(),
        Prayer::Asr => "Asr".to_string(),
        Prayer::Maghrib => "Maghrib".to_string(),
        Prayer::Isha => "Isha".to_string(),
        Prayer::Qiyam => "Qiyam".to_string(),
    };

    PrayerData { next_name, countdown_secs }
}

/// Discover ESP32 via mDNS. Returns "ip:port" or None.
fn discover_esp(log: &LogBuffer) -> Option<String> {
    log_msg(log, "[mDNS] Scanning for espmon devices...");
    let mdns = ServiceDaemon::new().ok()?;
    let receiver = mdns.browse("_espmon._udp.local.").ok()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(event) = receiver.recv_timeout(Duration::from_millis(500)) {
            if let ServiceEvent::ServiceResolved(info) = event {
                for addr in info.get_addresses() {
                    let target = format!("{}:{}", addr, info.get_port());
                    log_msg(log, &format!("[mDNS] Found ESP at {}", target));
                    let _ = mdns.shutdown();
                    return Some(target);
                }
            }
        }
    }
    let _ = mdns.shutdown();
    log_msg(log, "[mDNS] No ESP found on network.");
    None
}

fn get_hardware_stats(nvml: &Option<nvml_wrapper::Nvml>, log: &LogBuffer) -> HardwareData {
    let mut data = HardwareData { cpu_temp: 0.0, gpu_temp: 0.0, gpu_usage: 0.0, vram_usage: 0.0 };
    
    static LHM_LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    // Primary path: LibreHardwareMonitor WMI (works for AMD + Intel + NVIDIA)
    match WMIConnection::with_namespace_path("root\\LibreHardwareMonitor") {
        Ok(wmi) => {
            let query = "SELECT Name, Value FROM Sensor";
            if let Ok(results) = wmi.raw_query::<Sensor>(query) {
                for s in results {
                    if s.name.contains("CPU Package") || s.name.contains("Core (Tctl/Tdie)") { data.cpu_temp = s.value; }
                    if s.name.contains("GPU Core") || s.name.contains("GPU Edge") {
                        if s.name.contains("Temp") || data.gpu_temp == 0.0 {
                             if s.name.contains("Temp") { data.gpu_temp = s.value; }
                             else if data.gpu_temp == 0.0 { data.gpu_temp = s.value; }
                        }
                    }
                    if s.name == "GPU Core" { data.gpu_usage = s.value; }
                    if s.name.contains("GPU Memory") { data.vram_usage = s.value; }
                }
            }
            LHM_LOGGED.store(false, std::sync::atomic::Ordering::Relaxed);
        }
        Err(_) => {
            if !LHM_LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                log_msg(log, "[WARN] LibreHardwareMonitor not found. GPU/temp data may be incomplete.");
            }
        }
    }

    // Optional overlay: NVML for NVIDIA GPUs (more accurate if available)
    if let Some(nvml) = nvml {
        if let Ok(device) = nvml.device_by_index(0) {
            if let Ok(temp) = device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu) {
                data.gpu_temp = temp as f32;
            }
            if let Ok(util) = device.utilization_rates() {
                data.gpu_usage = util.gpu as f32;
            }
            if let Ok(mem) = device.memory_info() {
                if mem.total > 0 {
                    data.vram_usage = (mem.used as f32 / mem.total as f32) * 100.0;
                }
            }
        }
    }

    data
}

fn get_active_window() -> String {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() { return "Desktop".to_string(); }
        let mut buffer = [0u16; 512];
        let len = GetWindowTextW(hwnd, buffer.as_mut_ptr(), 512);
        if len > 0 {
            let title = String::from_utf16_lossy(&buffer[..len as usize]);
            if title.is_empty() { "Desktop".to_string() } else { title }
        } else { "Desktop".to_string() }
    }
}

fn main() {
    let config = Arc::new(Mutex::new(AppConfig::load()));
    let log_buffer: LogBuffer = Arc::new(Mutex::new(Vec::new()));

    log_msg(&log_buffer, "[SYS] Esp_Mon v2.0 starting...");

    // Auto-discover ESP if ip is "auto"
    {
        let mut cfg = config.lock().unwrap();
        if cfg.esp_ip == "auto" || cfg.esp_ip.is_empty() {
            if let Some(discovered) = discover_esp(&log_buffer) {
                cfg.esp_ip = discovered;
                let _ = cfg.save();
            } else {
                cfg.esp_ip = "192.168.0.181:1234".to_string();
            }
        }
    }

    // Init NVML for NVIDIA GPU monitoring (graceful: None on AMD)
    let nvml_instance: Arc<Option<nvml_wrapper::Nvml>> = Arc::new(nvml_wrapper::Nvml::init().ok());
    if nvml_instance.is_some() { log_msg(&log_buffer, "[GPU] NVIDIA GPU detected via NVML"); }
    else { log_msg(&log_buffer, "[GPU] No NVIDIA GPU — using LHM for AMD/Intel"); }

    let tray_menu = Menu::new();
    let settings_item = MenuItem::new("Settings", true, None);
    let show_logs_item = MenuItem::new("Show Logs", true, None);
    let hide_logs_item = MenuItem::new("Hide Logs", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let _ = tray_menu.append_items(&[
        &settings_item, &PredefinedMenuItem::separator(),
        &show_logs_item, &hide_logs_item, &PredefinedMenuItem::separator(),
        &quit_item,
    ]);

    // Load both icons
    let icon_online = Icon::from_resource(101, Some((32, 32))).expect("Failed to load online icon");
    let icon_offline = Icon::from_resource(102, Some((32, 32))).expect("Failed to load offline icon");

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("PC Monitor v1.23 [Initializing]")
        .with_icon(icon_offline.clone())
        .build()
        .expect("Failed to create tray icon");

    let config_clone = Arc::clone(&config);
    let settings_id = settings_item.id().clone();
    let show_logs_id = show_logs_item.id().clone();
    let hide_logs_id = hide_logs_item.id().clone();
    let quit_id = quit_item.id().clone();

    let last_pong = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(60)));
    let last_pong_clone = Arc::clone(&last_pong);
    let ping_rtt = Arc::new(Mutex::new(0u128));
    let ping_clone = Arc::clone(&ping_rtt);

    let socket = UdpSocket::bind(LOCAL_IP).expect("Failed to bind UDP socket");
    let socket_clone = socket.try_clone().expect("Failed to clone socket");
    
    thread::spawn(move || {
        let mut buf = [0; 1024];
        loop {
            if let Ok((size, _)) = socket_clone.recv_from(&mut buf) {
                let msg = String::from_utf8_lossy(&buf[..size]);
                if let Some(rest) = msg.strip_prefix("PONG|") {
                    if let Ok(ts) = rest.parse::<u128>() {
                        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
                        if now >= ts { 
                            if let Ok(mut p) = ping_clone.lock() { *p = now - ts; } 
                            if let Ok(mut lp) = last_pong_clone.lock() { *lp = Instant::now(); }
                        }
                    }
                }
            }
        }
    });

    let cfg_bg = Arc::clone(&config);
    let nvml_bg = Arc::clone(&nvml_instance);
    let log_bg = Arc::clone(&log_buffer);
    thread::spawn(move || {
        let mut sys = System::new_all();
        let mut networks = Networks::new_with_refreshed_list();
        let mut disks = Disks::new_with_refreshed_list();
        let mut serial_port: Option<Box<dyn serialport::SerialPort>> = None;
        let mut last_serial_retry = Instant::now() - Duration::from_secs(10);

        // --- Auto-Pairing: Generate key if empty and send PAIR packets ---
        {
            let mut cfg = cfg_bg.lock().unwrap();
            if cfg.hmac_key.is_empty() {
                use rand::Rng;
                let key: String = rand::rng()
                    .sample_iter(&rand::distr::Alphanumeric)
                    .take(64)
                    .map(char::from)
                    .collect();
                cfg.hmac_key = key;
                let _ = cfg.save();
                log_msg(&log_bg, "[SEC] Generated new HMAC key for pairing.");
            }
        }

        // Send PAIR packets for 10 seconds to catch the ESP32's pairing window
        let pair_deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < pair_deadline {
            let cfg = cfg_bg.lock().unwrap().clone();
            let pair_pkt = format!("PAIR|{}\n", cfg.hmac_key);
            let _ = socket.send_to(pair_pkt.as_bytes(), &cfg.esp_ip);
            log_msg(&log_bg, &format!("[SEC] Sending PAIR packet to {}...", cfg.esp_ip));
            thread::sleep(Duration::from_secs(2));
        }
        log_msg(&log_bg, "[SEC] Pairing phase complete. Switching to telemetry.");

        loop {
            let start_time = Instant::now();
            let current_cfg = cfg_bg.lock().unwrap().clone();
            if serial_port.is_none() && last_serial_retry.elapsed() > Duration::from_secs(5) {
                last_serial_retry = Instant::now();
                if let Ok(p) = serialport::new(&current_cfg.com_port, current_cfg.baud_rate).timeout(Duration::from_millis(50)).open() {
                    serial_port = Some(p);
                }
            }

            sys.refresh_all();
            sys.refresh_processes_specifics(sysinfo::ProcessesToUpdate::All, true, sysinfo::ProcessRefreshKind::nothing().with_cpu().with_memory());
            networks.refresh(true); disks.refresh(true);

            let cpu = sys.global_cpu_usage();
            let ram = (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0;
            let mut down = 0.0; let mut up = 0.0;
            for (_, data) in &networks { down += data.received() as f32 / 1024.0; up += data.transmitted() as f32 / 1024.0; }
            let mut disk_pct = 0.0; if let Some(d) = disks.iter().next() { disk_pct = ((d.total_space() - d.available_space()) as f32 / d.total_space() as f32) * 100.0; }
            let stats = get_hardware_stats(&nvml_bg, &log_bg);
            let window_title = get_active_window();
            
            // Security: Privacy Filter for Active Window
            let window_short = {
                let lower = window_title.to_lowercase();
                if lower.contains("private") || lower.contains("incognito") || lower.contains("password") || lower.contains("bank") || lower.contains("vault") {
                    "Privacy Restricted".to_string()
                } else if window_title.chars().count() > 30 {
                    format!("{}...", window_title.chars().take(27).collect::<String>())
                } else {
                    window_title
                }
            };

            let rtt = *ping_rtt.lock().unwrap();
            let now_ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
            let time_str = Local::now().format("%I:%M:%S %p").to_string();

            // Calculate next prayer time (Umm Al-Qura / Shafi)
            let prayer = calculate_prayer_info(&current_cfg);

            let mut processes: Vec<_> = sys.processes().values().collect();
            processes.sort_by(|a, b| b.cpu_usage().partial_cmp(&a.cpu_usage()).unwrap());
            let top_cpu = processes.iter().take(3).map(|p| format!("{}:{:.0}", p.name().to_string_lossy(), p.cpu_usage())).collect::<Vec<_>>().join(";");
            processes.sort_by(|a, b| b.memory().cmp(&a.memory()));
            let top_ram = processes.iter().take(3).map(|p| format!("{}:{:.1}", p.name().to_string_lossy(), p.memory() as f32 / 1024.0 / 1024.0 / 1024.0)).collect::<Vec<_>>().join(";");

            let packet = format!("STAT|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n", 
                cpu, ram, stats.gpu_usage, stats.cpu_temp, stats.gpu_temp, disk_pct, down, up, stats.vram_usage, rtt, time_str, window_short, now_ts, top_cpu, top_ram, current_cfg.cpu_temp_limit, current_cfg.gpu_temp_limit, prayer.next_name, prayer.countdown_secs);

            // Sign packet with HMAC if key is set
            let final_packet = if !current_cfg.hmac_key.is_empty() {
                let mut mac = HmacSha256::new_from_slice(current_cfg.hmac_key.as_bytes()).unwrap();
                mac.update(packet.trim().as_bytes());
                let sig = hex::encode(mac.finalize().into_bytes());
                format!("{}|HMAC={}\n", packet.trim(), sig)
            } else {
                packet
            };

            let mut sent_via_serial = false;
            if let Some(ref mut p) = serial_port {
                if let Ok(_) = p.write_all(final_packet.as_bytes()) { 
                    let _ = p.flush();
                    sent_via_serial = true; 
                } else { 
                    serial_port = None;
                }
            }
            if !sent_via_serial { let _ = socket.send_to(final_packet.as_bytes(), &current_cfg.esp_ip); }

            let elapsed = start_time.elapsed();
            let interval = Duration::from_millis(current_cfg.interval_ms);
            if elapsed < interval { thread::sleep(interval - elapsed); }
        }
    });

    let mut last_status = false;
    loop { 
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == settings_id {
                log_msg(&log_buffer, "[UI] Opening settings window...");
                open_settings_ui(Arc::clone(&config_clone));
            } else if event.id == show_logs_id {
                unsafe { AllocConsole(); let window = GetConsoleWindow(); if !window.is_null() { ShowWindow(window, SW_SHOW); } }
                println!("=== Esp_Mon Logs Since Launch ===");
                if let Ok(buf) = log_buffer.lock() {
                    for entry in buf.iter() {
                        println!("{}", entry);
                    }
                }
                println!("=== Live Logging Active ===");
            } else if event.id == hide_logs_id {
                unsafe { let window = GetConsoleWindow(); if !window.is_null() { ShowWindow(window, SW_HIDE); } }
            } else if event.id == quit_id { std::process::exit(0); }
        }

        let is_online = last_pong.lock().unwrap().elapsed() < Duration::from_secs(3);
        if is_online != last_status {
            let status_str = if is_online { "Online" } else { "Offline" };
            log_msg(&log_buffer, &format!("[NET] ESP status changed: {}", status_str));
            let cfg = config_clone.lock().unwrap();
            let _ = tray.set_tooltip(Some(format!("PC Monitor v2.0\nStatus: {}\nTarget: {}\nPort: {}", status_str, cfg.esp_ip, cfg.com_port)));
            let _ = tray.set_icon(Some(if is_online { icon_online.clone() } else { icon_offline.clone() }));
            last_status = is_online;
        }
        thread::sleep(Duration::from_millis(100)); 
    }
}
