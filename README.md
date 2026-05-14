# Esp_Mon (v2.0)

A high-performance, real-time PC monitoring system featuring a Rust-based background service and an ESP32 (CYD) touchscreen display. Universal support for all PC hardware (AMD, NVIDIA, Intel) and Laptops with dual-mode communication (Serial & WiFi).

![ESP32-2432S028R]([https://ae-pic-a1.aliexpress-media.com/kf/S3c51ddfda6b24dfd8b981226d665a79d5.jpg_640x640q75.jpg_.avif])
*Target Hardware: ESP32-2432S028R (Cheap Yellow Display)*

## 🚀 Key Features

### 📡 Smart Networking
- **mDNS Auto-Discovery:** The ESP32 broadcasts its presence on the network. The PC sender finds it automatically — no IP typing required.
- **HMAC Packet Signing:** Optional security layer. Packets are signed with HMAC-SHA256 to prevent spoofed data injection.
- **Auto-Pairing:** Security key is exchanged automatically on first connection — zero manual configuration.
- **Dual-Mode Communication:** Prefers USB Serial when plugged in, falls back to WiFi when wireless.

### 🖥️ Hardware Monitoring
- **Universal Hardware Support:** Accurate temperature and usage tracking for all CPU/GPU brands via LibreHardwareMonitor.
- **NVIDIA GPU Enhancement:** Optional native NVML integration for direct NVIDIA GPU monitoring (works without LHM).
- **AMD Primary Support:** Full AMD Ryzen/Radeon support through LibreHardwareMonitor WMI.
- **Configurable Thermal Limits:** Set custom CPU/GPU temperature thresholds from the PC settings.

### 🎨 Display & Themes
- **4 Color Themes:** Default, Cyberpunk, Matrix, and Arctic. Long-press the screen to cycle themes.
- **Zero-Lag Graphing:** Sprite-based rendering for ultra-smooth 60FPS updates.
- **Thermal Safety System:** Screen alerts and LED warnings when hardware exceeds configurable temperature limits.
- **Interactive Touch UI:**
  - **Page 1:** Live Graph + CPU/RAM/GPU % Overview.
  - **Page 2:** Detailed Metrics (Temp, VRAM, Disk, Network Speed).
  - **Page 3:** Top Processes (Live CPU & RAM usage lists).

### 🔄 OTA Firmware Updates
- **Browser-Based Updates:** Navigate to `http://espmon.local/update` to flash new firmware from any browser.
- **Status Dashboard:** Visit `http://espmon.local` to view ESP32 diagnostics (IP, RSSI, uptime, heap).
- **No USB Required:** After the initial flash, all future updates go through WiFi.

### 💻 Professional Desktop App (Rust)
- **Windowless Mode:** Runs silently in the background (no CMD flicker).
- **System Tray:** Managed via a custom heartbeat icon with online/offline status.
- **Redesigned Native Settings UI:** Full egui-based settings panel with organized sections (Connection, Thermal Limits, Security, System), color-coded headers, slider controls, and inline save confirmation.
- **Log Audit Trail:** All system events are timestamped and buffered since launch. "Show Logs" dumps the complete audit history to a console window, then continues live logging.
- **Privacy Filter:** Automatically hides sensitive window titles (banking, passwords, incognito).
- **Burn-in Protection:** Auto-dims the screen after 60s of inactivity.

## 🛠️ Hardware Requirements
This project is designed specifically for the **ESP32-2432S028R**, commonly known as the **"Cheap Yellow Display" (CYD)**.

### ESP32-CYD Specifications:
*   **CPU:** ESP32-WROOM-32 (Dual Core 240MHz).
*   **Display:** 2.8" TFT LCD (320x240 Resolution).
*   **Touch:** XPT2046 Resistive Touchscreen.
*   **Sensors:** Onboard LDR (Light Sensor) for auto-brightness.
*   **LED:** Onboard RGB LED for status and thermal alerts.

## 📦 Setup Instructions

### 1. PC Side (Rust Sender)
1. **Download:** Get the latest pre-compiled executable from the [Official Releases](https://github.com/alwalid54321/Esp_Mon/releases).
2. Install [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor) and keep it running (required for AMD temps. Optional for NVIDIA if using NVML).
3. Run `Esp_Sender.exe`. It will start hidden in your system tray.
4. **Auto-Discovery:** The sender will automatically find your ESP32 via mDNS. If not found, right-click tray → **Settings** to configure manually.

*Alternatively, build from source: `cd esp_sender_rust && cargo build -r`*

### 2. ESP Side (Arduino Receiver)
1. Open `esp_receiver_cyd/esp_receiver_cyd.ino` in Arduino IDE.
2. Install required libraries: `TFT_eSPI`, `XPT2046_Touchscreen`.
3. Configure `TFT_eSPI` for the CYD pinout.
4. Upload the code via USB (Hold the **BOOT** button on the back if needed).
5. **WiFi Setup:** Connect your phone to the **"PC-Monitor-Setup"** WiFi hotspot.
6. Open `192.168.4.1` in your browser. Pick your WiFi and enter the password.
7. **Future Updates:** Navigate to `http://espmon.local/update` — no USB required!

### 3. Theme Switching
- **Short tap** the screen to cycle between pages (Graph → Dashboard → Processes).
- **Long press** (1.5s) the screen to cycle color themes (Default → Cyberpunk → Matrix → Arctic).

## 📁 Project Structure
- `esp_sender_rust/`: Rust source code for the PC background service.
- `esp_receiver_cyd/`: Arduino source code for the ESP32 display.

## ⚙️ Settings (`settings.json`)
- `com_port`: Serial port for wired connection (e.g., "COM10").
- `baud_rate`: Serial baud rate (default: 115200).
- `esp_ip`: ESP32 address. Set to `"auto"` for mDNS discovery.
- `auto_start`: Launch with Windows.
- `interval_ms`: Data refresh rate in milliseconds.
- `hmac_key`: Security key (auto-generated during pairing).
- `cpu_temp_limit` / `gpu_temp_limit`: Thermal alert thresholds in °C.

## 📋 Changelog

### v2.0
- **Redesigned Settings UI** — Organized into Connection, Thermal, Security, and System sections with colored headers, sliders, password-masked HMAC field, and inline save confirmation.
- **Log Audit System** — All events timestamped and buffered since launch. "Show Logs" dumps the full history then streams live.
- **Fixed Settings Window Crash** — Resolved winit thread panic by using `any_thread` event loop builder for cross-thread egui windows.
- **mDNS Auto-Discovery** — Automatic ESP32 detection on the local network.
- **HMAC-SHA256 Security** — Auto-pairing with signed packet authentication.
- **NVML GPU Support** — Optional native NVIDIA monitoring alongside LibreHardwareMonitor.
- **Dynamic Thermal Limits** — Configurable CPU/GPU temp thresholds sent to the CYD display.
- **OTA Firmware Updates** — Browser-based ESP32 flashing via `http://espmon.local/update`.

## 🛡️ License
This project is licensed under the **MIT License** - see the [LICENSE](LICENSE) file for details.
