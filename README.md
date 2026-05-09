# PC Monitor (v1.11)

A high-performance, real-time PC monitoring system using a Rust-based sender and an ESP32 (CYD) receiver. Optimized for AMD hardware with dual-mode communication (Serial & WiFi).

![ESP32-2432S028R](https://raw.githubusercontent.com/witnessmenow/ESP32-Cheap-Yellow-Display/main/images/CYD-Front.jpg)
*Target Hardware: ESP32-2432S028R (Cheap Yellow Display)*

## 🚀 Key Features
- **AMD Ryzen/Radeon Native Support:** Accurate temperature and usage tracking via LibreHardwareMonitor.
- **Zero-Lag Graphing:** Sprite-based rendering on the ESP32 for ultra-smooth 60FPS updates.
- **Smart Auto-Switching:** Automatically prefers USB Serial connection when plugged in, falling back to WiFi when wireless.
- **Real-Time Ping:** True RTT calculation displayed on the screen.
- **Thermal Safety System:** Red screen alerts and LED warnings if hardware exceeds 85°C.
- **Interactive UI (ESP32):** 
  - Page 1: Live Graph + CPU/RAM/GPU % Overview.
  - Page 2: Detailed Metrics (Temp, VRAM, Disk, Network Speed).
  - Page 3: Top Processes (CPU & RAM usage).
- **Professional Desktop App (Rust):**
  - **System Tray Integration:** Starts hidden on your PC with a custom heartbeat icon.
  - **Unified Settings UI:** Right-click the tray icon to change ESP IP, COM Port, and Auto-Start toggle in a single window.
  - **Live Logs:** Toggle a terminal window from the tray to see real-time data packets.
  - **Auto-Start on Boot:** Easily enable/disable launching with Windows from the settings menu.
- **Burn-in Protection:** Auto-dims the screen after 60s of inactivity.

## 🛠️ Hardware Requirements
- **ESP32-2432S028R** (Also known as the **Cheap Yellow Display / CYD**)
  - 2.8" TFT Touchscreen (320x240)
  - Integrated ESP32-WROOM
  - Onboard LDR (Light Sensor) and RGB LED
- Windows PC (AMD or Intel/NVIDIA)

## 📦 Setup Instructions

### 1. PC Side (Rust Sender)
1. Install [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor) and keep it running (required for temps).
2. Navigate to `esp_sender_rust/`.
3. Build the project:
   ```bash
   cargo build -r
   ```
4. Run the executable `target/release/Esp_Sender.exe`. It will start hidden in your system tray.
5. **Configuration:** Right-click the tray icon and select **Settings** to configure your ESP's IP and COM port. These are saved to `settings.json`.

### 2. ESP Side (Arduino Receiver)
1. Open `esp_receiver_cyd/esp_receiver_cyd.ino` in Arduino IDE.
2. Ensure you have the following libraries installed:
   - `TFT_eSPI` (Configure for CYD pins)
   - `XPT2046_Touchscreen`
3. Upload to your ESP32 (Hold **BOOT** button if it fails to connect).
4. **WiFi Setup:** On first boot, the ESP will start a WiFi Portal named **"PC-Monitor-Setup"**. Connect with your phone and enter your WiFi credentials.

## 📁 Project Structure
- `esp_sender_rust/`: Rust source code for the PC background service.
- `esp_receiver_cyd/`: Arduino source code for the ESP32 display.

## ⚙️ Configuration (`settings.json`)
The Rust sender stores your preferences locally:
- `com_port`: The serial port for wired connection (e.g., "COM10").
- `esp_ip`: The network address of your ESP32 (e.g., "192.168.0.181:1234").
- `auto_start`: Boolean to enable/disable launching with Windows.

## 🛡️ License
MIT License. Free to use and modify.
