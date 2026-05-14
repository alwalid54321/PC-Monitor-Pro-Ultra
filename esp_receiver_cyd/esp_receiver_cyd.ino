#include <FS.h>
typedef fs::FS FS;

#include <TFT_eSPI.h>
#include <SPI.h>
#include <XPT2046_Touchscreen.h> 
#include <WiFi.h>
#include <WiFiUdp.h>
#include <WebServer.h>
#include <DNSServer.h>
#include <Preferences.h>
#include <ESPmDNS.h>
#include <Update.h>
#include "mbedtls/md.h"

using namespace fs;

/* 
 * PC MONITOR V2.0
 * FEATURES: 
 *   - OTA Firmware Updates via Web Browser
 *   - mDNS Auto-Discovery (espmon.local)
 *   - Secure HTTP POST for Credentials
 *   - Pro Diagnostic Offline Screen
 *   - Real-time RSSI Signal Icons
 */

const char* FW_VERSION = "2.0";
const char* MDNS_NAME = "espmon";
bool otaServerStarted = false;

#define XPT2046_IRQ 36
#define XPT2046_MOSI 32
#define XPT2046_MISO 39
#define XPT2046_CLK 25
#define XPT2046_CS 33
#define BL_PIN 21      
#define LED_RED 4
#define LED_GREEN 16
#define LED_BLUE 17

const int udpPort = 1234;
const int replyPort = 1235;
Preferences prefs;
WebServer server(80);
DNSServer dnsServer;

SPIClass touchSPI = SPIClass(VSPI);
XPT2046_Touchscreen touch(XPT2046_CS, XPT2046_IRQ);
TFT_eSPI tft = TFT_eSPI();
TFT_eSprite graphSprite = TFT_eSprite(&tft);
WiFiUDP udp;

// --- COLOR THEMES ---
struct Theme {
  uint16_t bg, accent, cpu, gpu, ram, text, warn, header, grid;
  const char* name;
};

const Theme themes[] = {
  { TFT_BLACK, 0x05B5, TFT_GREEN, TFT_MAGENTA, TFT_CYAN, TFT_WHITE, TFT_RED, 0x0841, 0x18E3, "Default" },
  { 0x0000, 0xF81F, 0xFFE0, 0xF81F, 0x07FF, 0xFFFF, 0xFBE0, 0x1082, 0x2104, "Cyberpunk" },
  { 0x0000, 0x07E0, 0x07E0, 0x03E0, 0x0400, 0x07E0, 0xF800, 0x0200, 0x0320, "Matrix" },
  { 0x0000, 0x5DFF, 0x07FF, 0x001F, 0xAFFF, 0xFFFF, 0xFBE0, 0x0010, 0x10A2, "Arctic" },
};
const int NUM_THEMES = 4;

// --- STATE ---
const float EMA_ALPHA = 0.25f;
unsigned long lastPacketTime = 0;
bool connected = false;
int brightnessLevel = 3; 
bool isSleeping = false;
bool thermalAlert = false;
int viewMode = 0; 
int currentTheme = 0;
unsigned long touchStartTime = 0;
bool touchActive = false;

String hmac_key = "";
bool pairingMode = false;
unsigned long pairingStartTime = 0;

float cpuEMA = 0, ramEMA = 0, gpuEMA = 0, vramEMA = 0;
float cTempEMA = 0, gTempEMA = 0, diskEMA = 0;
float netDEMA = 0, netUEMA = 0;
String activeApp = "PC Monitor";
String currentTime = "00:00:00";
int currentPing = 0;
String topCpuStr = "", topRamStr = "";

float graphCPU[320], graphGPU[320], graphRAM[320];
int graphIdx = 0;

void setLED(bool r, bool g, bool b) {
  digitalWrite(LED_RED, !r); digitalWrite(LED_GREEN, !g); digitalWrite(LED_BLUE, !b);
}

void setBrightness(int level) {
  int duty = (isSleeping) ? 10 : (level == 1 ? 128 : (level == 2 ? 192 : (level == 3 ? 255 : 64)));
  ledcWrite(BL_PIN, duty);
}

#define t() themes[currentTheme]

void saveTheme() {
  prefs.begin("ui", false);
  prefs.putInt("theme", currentTheme);
  prefs.end();
}

void loadTheme() {
  prefs.begin("ui", true);
  currentTheme = prefs.getInt("theme", 0) % NUM_THEMES;
  prefs.end();
}

// --- SECURE WIFI PORTAL ---
String portalError = "";

void handlePortal() {
  String html = "<html><head><meta name='viewport' content='width=device-width, initial-scale=1'><style>";
  html += "body{font-family:sans-serif;background:#0a0a0a;color:#eee;text-align:center;padding:20px;}";
  html += ".card{background:#1a1a1a;padding:20px;border-radius:15px;border:1px solid #00adb5;max-width:400px;margin:auto;}";
  html += "input,select{width:100%;padding:12px;margin:10px 0;background:#222;color:white;border:1px solid #444;border-radius:8px;box-sizing:border-box;}";
  html += "input[type='submit']{background:#00adb5;color:white;border:none;font-weight:bold;font-size:16px;margin-top:20px;}";
  html += ".err{color:#ff4d4d;padding:10px;} h1{color:#00adb5;}";
  html += "</style></head><body><div class='card'><h1>PC Monitor Setup</h1>";
  if (portalError != "") html += "<div class='err'>⚠ " + portalError + "</div>";
  html += "<p>Secure Configuration Portal</p>";
  html += "<form method='POST' action='/save'><h3>1. Select WiFi</h3><select name='s'>";
  int n = WiFi.scanNetworks();
  for (int i = 0; i < n; ++i) { html += "<option value='" + WiFi.SSID(i) + "'>" + WiFi.SSID(i) + " (" + String(WiFi.RSSI(i)) + "dBm)</option>"; }
  html += "</select><input name='sm' placeholder='Manual SSID (Optional)'>";
  html += "<h3>2. Password</h3><input type='password' name='p' placeholder='WiFi Password'>";
  html += "<input type='submit' value='Save & Connect'></form></div></body></html>";
  server.send(200, "text/html", html);
}

void handleSave() {
  String s = server.arg("s");
  if (server.hasArg("sm") && server.arg("sm") != "") s = server.arg("sm");
  String p = server.arg("p");
  
  if (s != "") {
    prefs.begin("wifi", false);
    prefs.putString("ssid", s); prefs.putString("pass", p); prefs.putBool("valid", true);
    prefs.end();
    server.send(200, "text/html", "<html><body style='background:#0a0a0a;color:white;text-align:center;'><h2>Credentials Received</h2><p>Attempting to connect... The portal is now closed.</p></body></html>");
    delay(2000); ESP.restart();
  } else {
    portalError = "SSID cannot be empty.";
    handlePortal();
  }
}

void startPortal(String error) {
  portalError = error;
  WiFi.mode(WIFI_AP);
  WiFi.softAP("PC-Monitor-Setup"); // Removed PIN security as requested
  dnsServer.start(53, "*", WiFi.softAPIP());
  server.on("/", HTTP_GET, handlePortal);
  server.on("/save", HTTP_POST, handleSave); // Use POST method for security
  server.begin();
  
  tft.fillScreen(TFT_BLACK);
  tft.setTextColor(TFT_YELLOW); tft.drawCentreString("WIFI SETUP PORTAL", 160, 40, 4);
  tft.setTextColor(TFT_WHITE);
  tft.drawCentreString("1. Connect to: PC-Monitor-Setup", 160, 90, 2);
  tft.drawCentreString("2. Open browser: 192.168.4.1", 160, 120, 2);
  tft.setTextColor(TFT_CYAN);
  tft.drawCentreString("CREDENTIALS SECURED VIA POST", 160, 160, 2);
  
  if (error != "") { tft.setTextColor(TFT_RED); tft.drawCentreString(error, 160, 200, 2); }
  while(true) { dnsServer.processNextRequest(); server.handleClient(); delay(10); }
}

// --- OTA STATUS PAGE ---
void handleStatusPage() {
  String html = "<html><head><meta name='viewport' content='width=device-width, initial-scale=1'><style>";
  html += "body{font-family:sans-serif;background:#0a0a0a;color:#eee;padding:20px;}";
  html += ".card{background:#1a1a1a;padding:20px;border-radius:15px;border:1px solid #00adb5;max-width:500px;margin:auto;}";
  html += "h1{color:#00adb5;} .row{display:flex;justify-content:space-between;padding:8px 0;border-bottom:1px solid #222;}";
  html += ".label{color:#888;} .value{color:#00adb5;font-weight:bold;}";
  html += "a.btn{display:inline-block;margin-top:15px;padding:12px 25px;background:#00adb5;color:#fff;text-decoration:none;border-radius:8px;font-weight:bold;}";
  html += "</style></head><body><div class='card'>";
  html += "<h1>PC Monitor</h1><p style='color:#888'>Firmware v" + String(FW_VERSION) + "</p>";
  html += "<div class='row'><span class='label'>WiFi</span><span class='value'>" + WiFi.SSID() + "</span></div>";
  html += "<div class='row'><span class='label'>IP</span><span class='value'>" + WiFi.localIP().toString() + "</span></div>";
  html += "<div class='row'><span class='label'>RSSI</span><span class='value'>" + String(WiFi.RSSI()) + " dBm</span></div>";
  html += "<div class='row'><span class='label'>Uptime</span><span class='value'>" + String(millis()/60000) + " min</span></div>";
  html += "<div class='row'><span class='label'>Free Heap</span><span class='value'>" + String(ESP.getFreeHeap()/1024) + " KB</span></div>";
  html += "<div class='row'><span class='label'>mDNS</span><span class='value'>espmon.local</span></div>";
  html += "<a class='btn' href='/update'>Update Firmware</a> ";
  html += "<a class='btn' style='background:#d32f2f;' href='/reset_key'>Reset Security Key</a>";
  html += "</div></body></html>";
  server.send(200, "text/html", html);
}

// --- OTA FIRMWARE UPDATE PAGE ---
void handleOTAPage() {
  String html = "<html><head><meta name='viewport' content='width=device-width, initial-scale=1'><style>";
  html += "body{font-family:sans-serif;background:#0a0a0a;color:#eee;padding:20px;text-align:center;}";
  html += ".card{background:#1a1a1a;padding:25px;border-radius:15px;border:1px solid #00adb5;max-width:500px;margin:auto;}";
  html += "h1{color:#00adb5;} input[type='file']{margin:15px 0;padding:12px;background:#222;color:white;border:1px solid #444;border-radius:8px;width:100%;box-sizing:border-box;}";
  html += "input[type='submit']{padding:12px 30px;background:#00adb5;color:white;border:none;border-radius:8px;font-size:16px;font-weight:bold;width:100%;}";
  html += ".warn{color:#ff9800;font-size:13px;margin-top:15px;} a{color:#00adb5;}";
  html += "</style></head><body><div class='card'>";
  html += "<h1>Firmware Update</h1><p style='color:#888'>Current: v" + String(FW_VERSION) + "</p>";
  html += "<form method='POST' action='/update' enctype='multipart/form-data'>";
  html += "<input type='file' name='firmware' accept='.bin' required>";
  html += "<input type='submit' value='Upload and Flash'></form>";
  html += "<p class='warn'>Export .bin from Arduino IDE: Sketch > Export Compiled Binary</p>";
  html += "<p><a href='/'>Back to Status</a></p>";
  html += "</div></body></html>";
  server.send(200, "text/html", html);
}

void handleOTAResult() {
  server.sendHeader("Connection", "close");
  if (Update.hasError()) {
    server.send(500, "text/html", "<html><body style='background:#0a0a0a;color:white;text-align:center;'><h2 style='color:red'>Update Failed!</h2><a href='/update' style='color:#00adb5'>Try Again</a></body></html>");
  } else {
    server.send(200, "text/html", "<html><body style='background:#0a0a0a;color:white;text-align:center;'><h2 style='color:#00adb5'>Update Success!</h2><p>Rebooting...</p></body></html>");
    delay(1000);
    ESP.restart();
  }
}

void handleOTAUpload() {
  HTTPUpload& upload = server.upload();
  if (upload.status == UPLOAD_FILE_START) {
    Serial.printf("[OTA] Uploading: %s\n", upload.filename.c_str());
    if (!Update.begin(UPDATE_SIZE_UNKNOWN)) { Update.printError(Serial); }
  } else if (upload.status == UPLOAD_FILE_WRITE) {
    if (Update.write(upload.buf, upload.currentSize) != upload.currentSize) { Update.printError(Serial); }
  } else if (upload.status == UPLOAD_FILE_END) {
    if (Update.end(true)) { Serial.printf("[OTA] Success: %u bytes\n", upload.totalSize); }
    else { Update.printError(Serial); }
  }
}

void handleResetKey() {
  prefs.begin("sec", false);
  prefs.remove("key");
  prefs.end();
  server.send(200, "text/html", "<html><body style='background:#0a0a0a;color:white;text-align:center;'><h2 style='color:#00adb5'>Security Key Reset!</h2><p>Rebooting into pairing mode...</p></body></html>");
  delay(1000);
  ESP.restart();
}

void startOTAServer() {
  MDNS.begin(MDNS_NAME);
  MDNS.addService("http", "tcp", 80);
  MDNS.addService("espmon", "udp", udpPort);
  server.on("/", HTTP_GET, handleStatusPage);
  server.on("/update", HTTP_GET, handleOTAPage);
  server.on("/update", HTTP_POST, handleOTAResult, handleOTAUpload);
  server.on("/reset_key", HTTP_GET, handleResetKey);
  server.begin();
  otaServerStarted = true;
  Serial.println("[OTA] Server started at http://espmon.local");
}

void drawSignalIcon(int x, int y) {
  int rssi = WiFi.RSSI();
  uint16_t color = (rssi > -60) ? TFT_GREEN : (rssi > -80 ? TFT_YELLOW : TFT_RED);
  for(int i=0; i<4; i++) {
    int h = (i+1)*4;
    tft.fillRect(x + (i*5), y + 16 - h, 3, h, (rssi > -90 + (i*10)) ? color : 0x3186);
  }
}

void drawOfflineScreen() {
  tft.fillScreen(TFT_BLACK);
  tft.drawRect(10, 10, 300, 220, 0x3186);
  tft.setTextColor(TFT_RED); tft.drawCentreString("OFFLINE", 160, 30, 4);
  tft.setTextColor(TFT_WHITE);
  tft.setCursor(30, 80); tft.println("DIAGNOSTICS:");
  tft.setCursor(40, 105); tft.print("WiFi: "); 
  if (WiFi.status() == WL_CONNECTED) {
    tft.setTextColor(TFT_GREEN); tft.println("Connected");
    tft.setTextColor(TFT_WHITE); tft.setCursor(40, 125); tft.print("IP:   "); tft.println(WiFi.localIP().toString());
    tft.setCursor(40, 145); tft.print("RSSI: "); tft.print(WiFi.RSSI()); tft.println(" dBm");
  } else {
    tft.setTextColor(TFT_RED); tft.println("Disconnected");
  }
  tft.setTextColor(TFT_WHITE);
  tft.setCursor(30, 180); tft.println("ACTION REQUIRED:");
  tft.setTextColor(TFT_CYAN);
  tft.drawCentreString("Start 'Esp Sender.exe' on PC", 160, 205, 2);
}

void setup() {
  Serial.begin(115200);
  pinMode(LED_RED, OUTPUT); pinMode(LED_GREEN, OUTPUT); pinMode(LED_BLUE, OUTPUT);
  setLED(false, false, true); 
  ledcAttach(BL_PIN, 5000, 8);
  setBrightness(brightnessLevel);

  touchSPI.begin(XPT2046_CLK, XPT2046_MISO, XPT2046_MOSI, XPT2046_CS);
  touch.begin(touchSPI);
  touch.setRotation(1);

  tft.init(); tft.setRotation(1); tft.fillScreen(TFT_BLACK);
  graphSprite.setColorDepth(8); graphSprite.createSprite(320, 140); 

  tft.setTextColor(TFT_CYAN); tft.drawCentreString("PC MONITOR", 160, 80, 4);
  tft.setTextColor(TFT_WHITE); tft.drawCentreString("V2.0", 160, 120, 2);

  loadTheme();

  prefs.begin("wifi", true);
  String ssid = prefs.getString("ssid", ""); String pass = prefs.getString("pass", "");
  prefs.end();

  prefs.begin("sec", true);
  hmac_key = prefs.getString("key", "");
  prefs.end();

  if (hmac_key == "") {
    pairingMode = true;
    pairingStartTime = millis();
  }

  if (ssid != "") {
    WiFi.begin(ssid.c_str(), pass.c_str());
    unsigned long start = millis();
    while (WiFi.status() != WL_CONNECTED && millis() - start < 10000) { delay(500); tft.print("."); }
  }

  if (WiFi.status() != WL_CONNECTED) {
    if (ssid == "") startPortal("");
    else startPortal("WiFi Auth Failed");
  } else {
    udp.begin(udpPort);
    startOTAServer();
    setLED(false, true, false); 
    tft.fillScreen(TFT_BLACK);
    tft.setTextColor(TFT_GREEN); tft.drawCentreString("SYNCED", 160, 90, 4);
    tft.setTextColor(TFT_WHITE); tft.drawCentreString(WiFi.localIP().toString(), 160, 130, 2);
    tft.setTextColor(0x05B5); tft.drawCentreString("OTA: espmon.local/update", 160, 160, 2);
    delay(2000); tft.fillScreen(TFT_BLACK);
  }
}

float smooth(float cur, float target) {
  if (cur == 0) return target;
  return (EMA_ALPHA * target) + ((1.0f - EMA_ALPHA) * cur);
}

void drawGraphView() {
  tft.fillRect(0, 0, 320, 35, thermalAlert ? t().warn : t().header); 
  tft.setTextColor(t().cpu); tft.drawString("C:" + String((int)cpuEMA) + "%", 5, 8, 4);
  tft.setTextColor(t().gpu); tft.drawString("G:" + String((int)gpuEMA) + "%", 100, 8, 4);
  tft.setTextColor(t().ram); tft.drawString("R:" + String((int)ramEMA) + "%", 195, 8, 4);
  drawSignalIcon(295, 10);

  graphSprite.fillSprite(t().bg);
  for(int i=0; i<=3; i++) graphSprite.drawFastHLine(0, i*35, 320, t().grid); 
  for (int x = 0; x < 319; x++) {
    int i1 = (graphIdx + x) % 320; int i2 = (graphIdx + x + 1) % 320;
    graphSprite.drawLine(x, 130-(graphCPU[i1]*1.2), x+1, 130-(graphCPU[i2]*1.2), t().cpu);
    graphSprite.drawLine(x, 130-(graphGPU[i1]*1.2), x+1, 130-(graphGPU[i2]*1.2), t().gpu);
    graphSprite.drawLine(x, 130-(graphRAM[i1]*1.2), x+1, 130-(graphRAM[i2]*1.2), t().ram);
  }
  graphSprite.pushSprite(0, 35);
  tft.fillRect(0, 175, 320, 65, t().bg);
  tft.setTextColor(t().text); tft.drawCentreString(currentTime, 160, 185, 4);
  tft.setTextColor(thermalAlert ? t().warn : t().accent); tft.drawCentreString(activeApp, 160, 215, 2);
}

void drawDashboardView() {
  tft.fillRect(0, 0, 320, 240, t().bg); 
  tft.setTextColor(t().accent); tft.drawCentreString("SYSTEM STATUS", 160, 5, 4);
  tft.drawFastHLine(0, 35, 320, t().accent);
  tft.setTextColor(t().warn); tft.drawString("CPU TMP: " + String((int)cTempEMA) + "C", 10, 50, 4);
  tft.drawString("GPU TMP: " + String((int)gTempEMA) + "C", 10, 85, 4);
  tft.setTextColor(t().gpu); tft.drawString("DISK: " + String((int)diskEMA) + "%", 170, 50, 4);
  tft.drawString("VRAM: " + String((int)vramEMA) + "%", 170, 85, 4);
  tft.setTextColor(t().accent); tft.drawString("DL: " + String((int)netDEMA) + " KB/s", 10, 130, 4);
  tft.drawString("UP: " + String((int)netUEMA) + " KB/s", 10, 165, 4);
  tft.setTextColor(t().cpu); tft.drawString("PING: " + String(currentPing) + " ms", 170, 130, 4);
  tft.setTextColor(t().text); tft.drawString("TIME: " + currentTime, 10, 210, 4);
}

void drawProcessesView() {
  tft.fillRect(0, 0, 320, 240, t().bg);
  tft.setTextColor(t().cpu); tft.drawCentreString("TOP CPU", 80, 5, 4);
  tft.setTextColor(t().ram); tft.drawCentreString("TOP RAM", 240, 5, 4);
  tft.drawFastVLine(160, 0, 240, t().accent);
  tft.drawFastHLine(0, 35, 320, t().accent);
  tft.setTextColor(t().text);
  int y = 50;
  char buf[256]; topCpuStr.toCharArray(buf, 256);
  char* p = strtok(buf, ";");
  while(p != NULL) { tft.drawString(String(p) + "%", 10, y, 2); y += 30; p = strtok(NULL, ";"); }
  y = 50; topRamStr.toCharArray(buf, 256); p = strtok(buf, ";");
  while(p != NULL) { tft.drawString(String(p) + " GB", 170, y, 2); y += 30; p = strtok(NULL, ";"); }
}

void processIncoming(String line) {
  if (pairingMode) {
    if (millis() - pairingStartTime >= 60000) {
      pairingMode = false; // window closed
    } else if (line.startsWith("PAIR|")) {
      hmac_key = line.substring(5);
      hmac_key.trim();
      prefs.begin("sec", false);
      prefs.putString("key", hmac_key);
      prefs.end();
      pairingMode = false;
      Serial.println("[SEC] Paired successfully!");
      tft.fillScreen(TFT_GREEN);
      tft.setTextColor(TFT_BLACK); tft.drawCentreString("SECURELY PAIRED", 160, 100, 4);
      delay(2000); tft.fillScreen(t().bg);
      return;
    }
  }

  if (!line.startsWith("STAT|")) return; // Basic Security: Only process valid telemetry

  if (hmac_key != "") {
    int hmacIdx = line.lastIndexOf("|HMAC=");
    if (hmacIdx == -1) return; // Drop unencrypted packet
    
    String payload = line.substring(0, hmacIdx);
    String receivedMac = line.substring(hmacIdx + 6);
    receivedMac.trim();

    mbedtls_md_context_t ctx;
    mbedtls_md_init(&ctx);
    mbedtls_md_setup(&ctx, mbedtls_md_info_from_type(MBEDTLS_MD_SHA256), 1);
    mbedtls_md_hmac_starts(&ctx, (const unsigned char *) hmac_key.c_str(), hmac_key.length());
    mbedtls_md_hmac_update(&ctx, (const unsigned char *) payload.c_str(), payload.length());
    unsigned char hmacResult[32];
    mbedtls_md_hmac_finish(&ctx, hmacResult);
    mbedtls_md_free(&ctx);

    String calcMac = "";
    for(int i=0; i<32; i++) {
        char str[3]; sprintf(str, "%02x", (int)hmacResult[i]);
        calcMac += str;
    }
    
    if (calcMac != receivedMac) return; // Drop invalid signature
    
    line = payload; // Truncate to just the payload for normal parsing
  }
  
  char buf[1024]; line.toCharArray(buf, 1024);
  char* p = strtok(buf, "|"); int i = 0; String pts[20];
  while (p != NULL && i < 20) { pts[i++] = String(p); p = strtok(NULL, "|"); }
  if (i < 15) return;
  
  cpuEMA = smooth(cpuEMA, pts[1].toFloat()); ramEMA = smooth(ramEMA, pts[2].toFloat());
  gpuEMA = smooth(gpuEMA, pts[3].toFloat()); cTempEMA = pts[4].toFloat(); gTempEMA = pts[5].toFloat();
  diskEMA = pts[6].toFloat(); netDEMA = pts[7].toFloat(); netUEMA = pts[8].toFloat();
  vramEMA = pts[9].toFloat(); currentPing = pts[10].toInt(); currentTime = pts[11];
  activeApp = pts[12]; String seq_ts = pts[13]; topCpuStr = pts[14]; topRamStr = pts[15];
  
  int cpuLimit = 85; int gpuLimit = 85;
  if (i >= 18) {
      cpuLimit = pts[16].toInt();
      gpuLimit = pts[17].toInt();
  }

  if (WiFi.status() == WL_CONNECTED) { udp.beginPacket(udp.remoteIP(), replyPort); udp.print("PONG|" + seq_ts); udp.endPacket(); }
  graphCPU[graphIdx] = cpuEMA; graphGPU[graphIdx] = gpuEMA; graphRAM[graphIdx] = ramEMA;
  graphIdx = (graphIdx + 1) % 320;
  lastPacketTime = millis(); connected = true;
  if (isSleeping) { isSleeping = false; setBrightness(brightnessLevel); }
  thermalAlert = (cTempEMA > cpuLimit || gTempEMA > gpuLimit);
  setLED(thermalAlert, !thermalAlert, false);
}

void loop() {
  // Handle OTA web server requests
  if (otaServerStarted) { server.handleClient(); }

  // Touch: short tap = switch page, long press (1.5s) = switch theme
  if (touch.touched()) {
    TS_Point p = touch.getPoint();
    if (p.z > 200) {
      if (!touchActive) { touchActive = true; touchStartTime = millis(); }
    }
  } else {
    if (touchActive) {
      unsigned long held = millis() - touchStartTime;
      if (held >= 1500) {
        // Long press: cycle theme
        currentTheme = (currentTheme + 1) % NUM_THEMES;
        saveTheme();
        tft.fillScreen(t().bg);
        tft.setTextColor(t().accent); tft.drawCentreString(t().name, 160, 100, 4);
        delay(800); tft.fillScreen(t().bg);
      } else {
        // Short tap: cycle page
        viewMode = (viewMode + 1) % 3; tft.fillScreen(t().bg);
      }
      touchActive = false;
      delay(200);
    }
  }
  if (WiFi.status() == WL_CONNECTED) {
    int packetSize = udp.parsePacket();
    if (packetSize) { char buf[1024]; int len = udp.read(buf, 1024); if (len > 0) { buf[len] = 0; processIncoming(String(buf)); } }
  }
  if (Serial.available()) { processIncoming(Serial.readStringUntil('\n')); }
  if (connected) {
    if (viewMode == 0) drawGraphView();
    else if (viewMode == 1) drawDashboardView();
    else drawProcessesView();
  } else if (pairingMode) {
    // Blink blue LED during pairing window
    setLED(false, false, (millis() / 500) % 2 == 0);
    tft.fillScreen(TFT_BLACK);
    tft.setTextColor(TFT_CYAN); tft.drawCentreString("PAIRING MODE", 160, 80, 4);
    tft.setTextColor(TFT_WHITE); tft.drawCentreString("Waiting for PC...", 160, 130, 2);
    int remaining = (60000 - (millis() - pairingStartTime)) / 1000;
    tft.drawCentreString(String(remaining) + "s remaining", 160, 160, 2);
    if (millis() - pairingStartTime >= 60000) { pairingMode = false; }
  } else { drawOfflineScreen(); setLED(false, false, true); }
  if (millis() - lastPacketTime > 60000 && !isSleeping) { isSleeping = true; setBrightness(brightnessLevel); }
  if (millis() - lastPacketTime > 4000) connected = false;
}
