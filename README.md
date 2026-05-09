# PC Monitor Pro (Ultra Edition)

A high-performance, real-time PC monitoring system using a Rust-based sender and an ESP32 (CYD) receiver. Optimized for AMD hardware with dual-mode communication (Serial & WiFi).

## 🚀 Key Features
- **AMD Ryzen/Radeon Native Support:** Accurate temperature and usage tracking via LibreHardwareMonitor.
- **Zero-Lag Graphing:** Sprite-based rendering on the ESP32 for ultra-smooth 60FPS updates.
- **Smart Auto-Switching:** Automatically prefers USB Serial connection when plugged in, falling back to WiFi when wireless.
- **Real-Time Ping:** True RTT calculation displayed on the screen.
- **Thermal Safety System:** Red screen alerts and LED warnings if hardware exceeds 85°C.
- **Interactive UI:** 
  - Page 1: Live Graph + CPU/RAM/GPU % Overview.
  - Page 2: Detailed Metrics (Temp, VRAM, Disk, Network Speed).
- **System Tray Integration:** Starts hidden on your PC with a professional taskbar icon.
- **Burn-in Protection:** Auto-dims the screen after 60s of inactivity.

## 🛠️ Hardware Requirements
- **ESP32-2432S028R** (Cheap Yellow Display / CYD)
- Windows PC (AMD or Intel/NVIDIA)

## 📦 Setup Instructions

### 1. PC Side (Rust Sender)
1. Install [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor) and keep it running (required for temps).
2. Navigate to `esp_sender_rust/`.
3. Build the project:
   ```bash
   cargo build -r
   ```
4. Run the executable in `target/release/esp_sender_rust.exe`. It will start hidden in your system tray.

### 2. ESP Side (Arduino Receiver)
1. Open `esp_receiver_cyd/esp_receiver_cyd.ino` in Arduino IDE.
2. **Critical:** Update your WiFi SSID and Password in the `CONFIGURATION` section.
3. Ensure you have the following libraries installed:
   - `TFT_eSPI` (Configure for CYD pins)
   - `XPT2046_Touchscreen`
4. Upload to your ESP32 (Hold **BOOT** button if it fails to connect).

## 📁 Project Structure
- `esp_sender_rust/`: Rust source code for the PC background service.
- `esp_receiver_cyd/`: Arduino source code for the ESP32 display.

## 🛡️ License
MIT License. Free to use and modify.
