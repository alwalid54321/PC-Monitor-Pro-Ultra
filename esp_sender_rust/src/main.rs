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
use tray_item::{TrayItem, IconSource};
use chrono::Local;
use serde::{Deserialize, Serialize};
use wmi::WMIConnection;

// WinAPI
use winapi::um::winuser::{ShowWindow, SW_HIDE, SW_SHOW, MessageBoxW, MB_OK, MB_ICONINFORMATION};
use winapi::um::wincon::{GetConsoleWindow};
use winapi::um::consoleapi::{AllocConsole};

const SETTINGS_FILE: &str = "settings.json";
const LOCAL_IP: &str = "0.0.0.0:1235"; 
const INTERVAL_MS: u64 = 800;
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Serialize, Deserialize, Clone)]
struct AppConfig {
    pub com_port: String,
    pub baud_rate: u32,
    pub esp_ip: String,
    pub auto_start: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            com_port: "COM10".to_string(),
            baud_rate: 115200,
            esp_ip: "192.168.0.181:1234".to_string(),
            auto_start: false,
        }
    }
}

impl AppConfig {
    fn load() -> Self {
        if let Ok(data) = fs::read_to_string(SETTINGS_FILE) {
            if let Ok(config) = serde_json::from_str(&data) {
                return config;
            }
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
            let cmd = format!(r#"reg add "HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run" /v "EspSender" /t REG_SZ /d "\"{}\"" /f"#, exe_str);
            let _ = Command::new("powershell").args(["-Command", &cmd]).creation_flags(CREATE_NO_WINDOW).output();
        } else {
            let cmd = r#"reg delete "HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run" /v "EspSender" /f"#;
            let _ = Command::new("powershell").args(["-Command", &cmd]).creation_flags(CREATE_NO_WINDOW).output();
        }
    }
}

fn open_settings_ui(current_config: AppConfig) {
    let available_ports = serialport::available_ports().unwrap_or_default();
    let port_list = available_ports.iter()
        .map(|p| p.port_name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    
    let port_hint = if port_list.is_empty() { "None detected".to_string() } else { format!("Found: {}", port_list) };

    // Professional Single-Window PowerShell Script (WinForms)
    let ps_script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; \
         Add-Type -AssemblyName System.Drawing; \
         $form = New-Object System.Windows.Forms.Form; \
         $form.Text = 'Esp Sender Settings'; \
         $form.Size = New-Object System.Drawing.Size(320,280); \
         $form.StartPosition = 'CenterScreen'; \
         $form.FormBorderStyle = 'FixedDialog'; \
         $form.MaximizeBox = $false; \
         $font = New-Object System.Drawing.Font('Segoe UI', 10); \
         $form.Font = $font; \
         $lbl1 = New-Object System.Windows.Forms.Label; $lbl1.Text = 'ESP IP Address:'; $lbl1.Location = '10,10'; $lbl1.Size = '280,20'; \
         $txtIp = New-Object System.Windows.Forms.TextBox; $txtIp.Text = '{0}'; $txtIp.Location = '10,30'; $txtIp.Size = '280,25'; \
         $lbl2 = New-Object System.Windows.Forms.Label; $lbl2.Text = 'COM Port ({1}):'; $lbl2.Location = '10,70'; $lbl2.Size = '280,20'; \
         $txtCom = New-Object System.Windows.Forms.TextBox; $txtCom.Text = '{2}'; $txtCom.Location = '10,90'; $txtCom.Size = '280,25'; \
         $chkAuto = New-Object System.Windows.Forms.CheckBox; $chkAuto.Text = 'Start with Windows'; $chkAuto.Location = '10,135'; $chkAuto.Size = '280,25'; $chkAuto.Checked = {3}; \
         $btnSave = New-Object System.Windows.Forms.Button; $btnSave.Text = 'Save & Restart'; $btnSave.Location = '10,180'; $btnSave.Size = '120,35'; \
         $btnSave.DialogResult = [System.Windows.Forms.DialogResult]::OK; \
         $btnCancel = New-Object System.Windows.Forms.Button; $btnCancel.Text = 'Cancel'; $btnCancel.Location = '170,180'; $btnCancel.Size = '120,35'; \
         $btnCancel.DialogResult = [System.Windows.Forms.DialogResult]::Cancel; \
         $form.Controls.AddRange(@($lbl1, $txtIp, $lbl2, $txtCom, $chkAuto, $btnSave, $btnCancel)); \
         $form.AcceptButton = $btnSave; \
         if ($form.ShowDialog() -eq 'OK') {{ \
             echo ($txtIp.Text + '|' + $txtCom.Text + '|' + $chkAuto.Checked) \
         }}", 
        current_config.esp_ip, port_hint, current_config.com_port, if current_config.auto_start { "$true" } else { "$false" }
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    if let Ok(o) = output {
        let result = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !result.is_empty() {
            let parts: Vec<&str> = result.split('|').collect();
            if parts.len() == 3 {
                let mut new_config = current_config;
                new_config.esp_ip = parts[0].to_string();
                new_config.com_port = parts[1].to_string();
                new_config.auto_start = parts[2].to_lowercase() == "true";

                let _ = new_config.save();
                new_config.update_autostart();
                
                unsafe {
                    let title = "Settings Saved\0".encode_utf16().collect::<Vec<u16>>();
                    let msg = "Settings applied successfully! Please restart the application for changes to take effect.\0"
                        .encode_utf16().collect::<Vec<u16>>();
                    MessageBoxW(std::ptr::null_mut(), msg.as_ptr(), title.as_ptr(), MB_OK | MB_ICONINFORMATION);
                }
            }
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct Sensor {
    name: String,
    value: f32,
}

struct HardwareData {
    cpu_temp: f32,
    gpu_temp: f32,
    gpu_usage: f32,
    vram_usage: f32,
}

fn get_hardware_stats() -> HardwareData {
    let mut data = HardwareData { cpu_temp: 0.0, gpu_temp: 0.0, gpu_usage: 0.0, vram_usage: 0.0 };
    if let Ok(wmi) = WMIConnection::with_namespace_path("root\\LibreHardwareMonitor") {
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
    }
    data
}

fn get_active_window() -> String {
    let script = "[Win32.User32]::GetWindowText([Win32.User32]::GetForegroundWindow(), ($sb = New-Object System.Text.StringBuilder 256), $sb.Capacity) | Out-Null; $sb.ToString()";
    let full_cmd = format!("Add-Type '@\n[DllImport(\"user32.dll\")] public static extern IntPtr GetForegroundWindow();\n[DllImport(\"user32.dll\")] public static extern int GetWindowText(IntPtr hWnd, System.Text.StringBuilder lpString, int nMaxCount);\n@' -Name User32 -Namespace Win32; {}", script);
    let out = Command::new("powershell").args(["-NoProfile", "-Command", &full_cmd]).creation_flags(CREATE_NO_WINDOW).output();
    if let Ok(o) = out {
        let title = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if title.is_empty() { "Desktop".to_string() } else { title }
    } else { "Desktop".to_string() }
}

fn main() {
    let config = Arc::new(Mutex::new(AppConfig::load()));

    let tooltip = "PC Monitor v1.11\nMonitoring: AMD/Intel";
    let mut tray = TrayItem::new(tooltip, IconSource::Resource("app-icon")).expect("Failed to create tray icon");

    let cfg_ui = Arc::clone(&config);
    tray.add_menu_item("Settings", move || {
        let current = cfg_ui.lock().unwrap().clone();
        open_settings_ui(current);
    }).ok();

    tray.add_menu_item("Show Logs", || {
        unsafe {
            AllocConsole();
            let window = GetConsoleWindow();
            if !window.is_null() { ShowWindow(window, SW_SHOW); }
        }
        println!("=== PC Monitor Live Logs ===");
    }).ok();

    tray.add_menu_item("Hide Logs", || {
        unsafe {
            let window = GetConsoleWindow();
            if !window.is_null() { ShowWindow(window, SW_HIDE); }
        }
    }).ok();

    tray.add_menu_item("Quit", || { std::process::exit(0); }).ok();

    let mut sys = System::new_all();
    let mut networks = Networks::new_with_refreshed_list();
    let mut disks = Disks::new_with_refreshed_list();

    let ping_rtt = Arc::new(Mutex::new(0u128));
    let ping_clone = Arc::clone(&ping_rtt);

    let socket = UdpSocket::bind(LOCAL_IP).expect("Failed to bind UDP socket");
    let socket_clone = socket.try_clone().expect("Failed to clone socket");
    
    thread::spawn(move || {
        let mut buf = [0; 1024];
        loop {
            if let Ok((size, _)) = socket_clone.recv_from(&mut buf) {
                let msg = String::from_utf8_lossy(&buf[..size]);
                if msg.starts_with("PONG|") {
                    if let Ok(ts) = msg[5..].parse::<u128>() {
                        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
                        if now >= ts { if let Ok(mut p) = ping_clone.lock() { *p = now - ts; } }
                    }
                }
            }
        }
    });

    let cfg_bg = Arc::clone(&config);
    thread::spawn(move || {
        let mut serial_port: Option<Box<dyn serialport::SerialPort>> = None;
        let mut last_serial_retry = Instant::now() - Duration::from_secs(10);

        loop {
            let start_time = Instant::now();
            let current_cfg = cfg_bg.lock().unwrap().clone();

            if serial_port.is_none() && last_serial_retry.elapsed() > Duration::from_secs(5) {
                last_serial_retry = Instant::now();
                if let Ok(p) = serialport::new(&current_cfg.com_port, current_cfg.baud_rate).timeout(Duration::from_millis(50)).open() {
                    println!("[AUTO] Serial Connected to {}", current_cfg.com_port);
                    serial_port = Some(p);
                }
            }

            sys.refresh_all();
            sys.refresh_processes_specifics(sysinfo::ProcessesToUpdate::All, true, sysinfo::ProcessRefreshKind::nothing().with_cpu().with_memory());
            networks.refresh(true);
            disks.refresh(true);

            let cpu = sys.global_cpu_usage();
            let ram = (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0;
            let mut down = 0.0;
            let mut up = 0.0;
            for (_, data) in &networks { down += data.received() as f32 / 1024.0; up += data.transmitted() as f32 / 1024.0; }
            let mut disk_pct = 0.0;
            if let Some(d) = disks.iter().next() { disk_pct = ((d.total_space() - d.available_space()) as f32 / d.total_space() as f32) * 100.0; }

            let stats = get_hardware_stats();
            let window_title = get_active_window();
            let window_short = if window_title.len() > 30 { format!("{}...", &window_title[..27]) } else { window_title };
            let rtt = *ping_rtt.lock().unwrap();
            let now_ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
            let time_str = Local::now().format("%H:%M:%S").to_string();

            let mut processes: Vec<_> = sys.processes().values().collect();
            processes.sort_by(|a, b| b.cpu_usage().partial_cmp(&a.cpu_usage()).unwrap());
            let top_cpu = processes.iter().take(3).map(|p| format!("{}:{:.0}", p.name().to_string_lossy(), p.cpu_usage())).collect::<Vec<_>>().join(";");
            processes.sort_by(|a, b| b.memory().cmp(&a.memory()));
            let top_ram = processes.iter().take(3).map(|p| format!("{}:{:.1}", p.name().to_string_lossy(), p.memory() as f32 / 1024.0 / 1024.0 / 1024.0)).collect::<Vec<_>>().join(";");

            let packet = format!("STAT|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{}|{}|{}|{}|{}|{}\n", 
                cpu, ram, stats.gpu_usage, stats.cpu_temp, stats.gpu_temp, disk_pct, down, up, stats.vram_usage, rtt, time_str, window_short, now_ts, top_cpu, top_ram);

            let mut sent_via_serial = false;
            if let Some(ref mut p) = serial_port {
                if let Ok(_) = p.write_all(packet.as_bytes()) { sent_via_serial = true; } else { serial_port = None; }
            }
            if !sent_via_serial { let _ = socket.send_to(packet.as_bytes(), &current_cfg.esp_ip); }

            let elapsed = start_time.elapsed();
            if elapsed < Duration::from_millis(INTERVAL_MS) { thread::sleep(Duration::from_millis(INTERVAL_MS) - elapsed); }
        }
    });

    loop { thread::sleep(Duration::from_secs(3600)); }
}
