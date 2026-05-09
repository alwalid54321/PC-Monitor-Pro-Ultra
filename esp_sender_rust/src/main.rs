#![windows_subsystem = "windows"]

use std::io::Write;
use std::time::{Duration, Instant};
use std::process::Command;
use std::os::windows::process::CommandExt;
use std::sync::{Arc, Mutex};
use std::thread;
use std::net::UdpSocket;
use sysinfo::{System, Networks, Disks};
use tray_item::{TrayItem, IconSource};
use chrono::Local;
use serde::Deserialize;
use wmi::WMIConnection;

// WinAPI for advanced console and UI management
use winapi::um::winuser::{ShowWindow, SW_HIDE, SW_SHOW};
use winapi::um::wincon::{GetConsoleWindow};
use winapi::um::consoleapi::{AllocConsole};

// --- CONFIGURATION ---
const COM_PORT: &str = "COM10"; 
const BAUD_RATE: u32 = 115200;
const DEFAULT_ESP_IP: &str = "192.168.0.181:1234"; 
const LOCAL_IP: &str = "0.0.0.0:1235"; 
const INTERVAL_MS: u64 = 500;
const CREATE_NO_WINDOW: u32 = 0x08000000;
// ---------------------

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
    if data.gpu_usage == 0.0 {
        let out = Command::new("nvidia-smi")
            .args(["--query-gpu=utilization.gpu,utilization.memory", "--format=csv,noheader,nounits"])
            .creation_flags(CREATE_NO_WINDOW).output();
        if let Ok(o) = out {
            if let Ok(txt) = String::from_utf8(o.stdout) {
                let parts: Vec<&str> = txt.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 2 {
                    data.gpu_usage = parts[0].parse().unwrap_or(0.0);
                    data.vram_usage = parts[1].parse().unwrap_or(0.0);
                }
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
    // The tooltip will act as the "About" info when hovering over the tray icon
    let tooltip = "PC Monitor Pro v10.5 Ultra\nMonitoring: AMD/Intel";
    let mut tray = TrayItem::new(tooltip, IconSource::Resource("app-icon")).expect("Failed to create tray icon");

    tray.add_menu_item("Show Logs", || {
        unsafe {
            AllocConsole();
            let window = GetConsoleWindow();
            if !window.is_null() { ShowWindow(window, SW_SHOW); }
        }
        println!("=== PC Monitor Pro Live Logs ===");
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

    thread::spawn(move || {
        let mut serial_port: Option<Box<dyn serialport::SerialPort>> = None;
        let mut last_serial_retry = Instant::now() - Duration::from_secs(10);

        loop {
            let start_time = Instant::now();
            if serial_port.is_none() && last_serial_retry.elapsed() > Duration::from_secs(5) {
                last_serial_retry = Instant::now();
                if let Ok(p) = serialport::new(COM_PORT, BAUD_RATE).timeout(Duration::from_millis(50)).open() {
                    serial_port = Some(p);
                }
            }

            sys.refresh_all();
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
            let now_ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
            let time_str = Local::now().format("%H:%M:%S").to_string();
            let rtt = *ping_rtt.lock().unwrap();

            let packet = format!("STAT|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{:.0}|{}|{}|{}|{}\n", 
                cpu, ram, stats.gpu_usage, stats.cpu_temp, stats.gpu_temp, disk_pct, down, up, stats.vram_usage, rtt, time_str, window_short, now_ts);

            let mut sent_via_serial = false;
            if let Some(ref mut p) = serial_port {
                if let Ok(_) = p.write_all(packet.as_bytes()) { sent_via_serial = true; } else { serial_port = None; }
            }
            if !sent_via_serial { let _ = socket.send_to(packet.as_bytes(), DEFAULT_ESP_IP); }

            // Log to console if it's currently showing
            println!("[{}] Sending: {}", if sent_via_serial { "SERIAL" } else { "WIFI" }, packet.trim());

            let elapsed = start_time.elapsed();
            if elapsed < Duration::from_millis(INTERVAL_MS) { thread::sleep(Duration::from_millis(INTERVAL_MS) - elapsed); }
        }
    });

    loop { thread::sleep(Duration::from_secs(3600)); }
}
