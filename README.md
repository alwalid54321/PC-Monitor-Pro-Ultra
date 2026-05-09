# PC Monitor (v1.22)

A high-performance, real-time PC monitoring system featuring a Rust-based background service and an ESP32 (CYD) touchscreen display. Optimized for AMD hardware with dual-mode communication (Serial & WiFi).

![ESP32-2432S028R](https://raw.githubusercontent.com/witnessmenow/ESP32-Cheap-Yellow-Display/main/images/CYD-Front.jpg)
*Target Hardware: ESP32-2432S028R (Cheap Yellow Display)*

## 🚀 Key Features
- **AMD Ryzen/Radeon Native Support:** Accurate temperature and usage tracking via LibreHardwareMonitor.
- **Zero-Lag Graphing:** Sprite-based rendering on the ESP32 for ultra-smooth 60FPS updates.
- **Smart Auto-Switching:** Automatically prefers USB Serial connection when plugged in, falling back to WiFi when wireless.
- **Real-Time Ping:** True RTT calculation displayed on the screen.
- **Thermal Safety System:** Red screen alerts and LED warnings if hardware exceeds 85°C.
- **Interactive UI (ESP32):** 
  - **Page 1:** Live Graph + CPU/RAM/GPU % Overview.
  - **Page 2:** Detailed Metrics (Temp, VRAM, Disk, Network Speed).
  - **Page 3:** Top Processes (Live CPU & RAM usage lists).
- **Professional Desktop App (Rust):**
  - **Windowless Mode:** Runs silently in the background (no CMD flicker).
  - **System Tray:** Managed via a custom heartbeat icon in the taskbar.
  - **Unified Settings UI:** Change ESP IP, COM Port, and Auto-Start in a single window.
  - **Live Logs:** Toggle a terminal window from the tray for debugging.
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
1. Install [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor) and keep it running (required for temps).
2. Navigate to `esp_sender_rust/`.
3. Build the project:
   ```bash
   cargo build -r
   ```
4. Run `target/release/Esp_Sender.exe`. It will start hidden in your system tray.
5. **Configuration:** Right-click the tray icon and select **Settings** to configure your ESP's IP and COM port.

### 2. ESP Side (Arduino Receiver)
1. Open `esp_receiver_cyd/esp_receiver_cyd.ino` in Arduino IDE.
2. Install required libraries: `TFT_eSPI`, `XPT2046_Touchscreen`.
3. Configure `TFT_eSPI` for the CYD pinout.
4. Upload the code (Hold the **BOOT** button on the back if it fails to connect).
5. **WiFi Setup:** On first boot, connect your phone to the **"PC-Monitor-Setup"** WiFi hotspot.
6. Open `192.168.4.1` in your browser. Pick your WiFi from the list and enter the password. Credentials are sent securely via **HTTP POST**.

## 📁 Project Structure
- `esp_sender_rust/`: Rust source code for the PC background service.
- `esp_receiver_cyd/`: Arduino source code for the ESP32 display.

## ⚙️ Settings (`settings.json`)
- `com_port`: The serial port for wired connection (e.g., "COM10").
- `esp_ip`: The network address of your ESP32 (e.g., "192.168.0.181:1234").
- `auto_start`: Enable/Disable launching with Windows.

## 🛡️ License
This project is licensed under the **MIT License** - see the [LICENSE](LICENSE) file for details.
