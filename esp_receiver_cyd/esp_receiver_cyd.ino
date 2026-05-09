#include <TFT_eSPI.h>
#include <SPI.h>
#include <XPT2046_Touchscreen.h> 
#include <WiFi.h>
#include <WiFiUdp.h>
#include <WebServer.h>
#include <DNSServer.h>
#include <Preferences.h>

/* 
 * PC MONITOR - CYD EDITION V1.11
 * FEATURES: 
 *   - Pro AMD Ryzen/Radeon Support
 *   - Smart WiFi Portal (No Hardcoding)
 *   - Top CPU & RAM Processes
 *   - Auto-Sleep & Thermal Safety
 *   - Real-Time Ping & Clock Sync
 */

// --- HARDWARE PINS ---
#define XPT2046_IRQ 36
#define XPT2046_MOSI 32
#define XPT2046_MISO 39
#define XPT2046_CLK 25
#define XPT2046_CS 33
#define BL_PIN 21      
#define LED_RED 4
#define LED_GREEN 16
#define LED_BLUE 17

// --- CONFIG ---
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

// --- STATE ---
const float EMA_ALPHA = 0.25f;
unsigned long lastPacketTime = 0;
bool connected = false;
int brightnessLevel = 3; 
bool isSleeping = false;
bool thermalAlert = false;
int viewMode = 0; // 0: Graph, 1: Dashboard, 2: Processes

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

// --- WIFI PORTAL ---
void handlePortal() {
  String html = "<html><body><h1>PC Monitor Setup</h1><form action='/save'>SSID: <input name='s'><br>Pass: <input name='p'><br><input type='submit'></form></body></html>";
  server.send(200, "text/html", html);
}

void handleSave() {
  String s = server.arg("s");
  String p = server.arg("p");
  prefs.begin("wifi", false);
  prefs.putString("ssid", s);
  prefs.putString("pass", p);
  prefs.end();
  server.send(200, "text/html", "Saved! Restarting...");
  delay(2000);
  ESP.restart();
}

void startPortal() {
  WiFi.mode(WIFI_AP);
  WiFi.softAP("PC-Monitor-Setup");
  dnsServer.start(53, "*", WiFi.softAPIP());
  server.on("/", handlePortal);
  server.on("/save", handleSave);
  server.begin();
  tft.fillScreen(TFT_BLACK);
  tft.setTextColor(TFT_YELLOW);
  tft.drawCentreString("WIFI PORTAL ACTIVE", 160, 80, 4);
  tft.setTextColor(TFT_WHITE);
  tft.drawCentreString("Connect to: PC-Monitor-Setup", 160, 120, 2);
  while(true) { dnsServer.processNextRequest(); server.handleClient(); delay(10); }
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

  tft.init();
  tft.setRotation(1);
  tft.fillScreen(TFT_BLACK);
  graphSprite.setColorDepth(8);
  graphSprite.createSprite(320, 140); 

  tft.setTextColor(TFT_CYAN);
  tft.drawCentreString("PC MONITOR", 160, 80, 4);
  tft.setTextColor(TFT_WHITE);
  tft.drawCentreString("V1.11 EDITION", 160, 120, 2);

  prefs.begin("wifi", true);
  String ssid = prefs.getString("ssid", "");
  String pass = prefs.getString("pass", "");
  prefs.end();

  if (ssid != "") {
    WiFi.begin(ssid.c_str(), pass.c_str());
    unsigned long start = millis();
    while (WiFi.status() != WL_CONNECTED && millis() - start < 10000) { delay(500); tft.print("."); }
  }

  if (WiFi.status() != WL_CONNECTED) {
    if (ssid == "") startPortal();
    else {
      tft.fillScreen(TFT_BLACK);
      tft.setTextColor(TFT_RED);
      tft.drawCentreString("WIFI FAILED", 160, 100, 4);
      tft.setTextColor(TFT_WHITE);
      tft.drawCentreString("Touch for Portal or use USB", 160, 140, 2);
      delay(3000);
    }
  } else {
    udp.begin(udpPort);
    setLED(false, true, false); 
  }
}

float smooth(float cur, float target) {
  if (cur == 0) return target;
  return (EMA_ALPHA * target) + ((1.0f - EMA_ALPHA) * cur);
}

void drawGraphView() {
  tft.fillRect(0, 0, 320, 35, thermalAlert ? TFT_RED : 0x1082); 
  tft.setTextColor(TFT_GREEN); tft.drawString("C:" + String((int)cpuEMA) + "%", 5, 8, 4);
  tft.setTextColor(TFT_MAGENTA); tft.drawString("G:" + String((int)gpuEMA) + "%", 110, 8, 4);
  tft.setTextColor(TFT_CYAN); tft.drawString("R:" + String((int)ramEMA) + "%", 215, 8, 4);

  graphSprite.fillSprite(TFT_BLACK);
  for(int i=0; i<=3; i++) graphSprite.drawFastHLine(0, i*35, 320, 0x18E3); 
  for (int x = 0; x < 319; x++) {
    int i1 = (graphIdx + x) % 320; int i2 = (graphIdx + x + 1) % 320;
    graphSprite.drawLine(x, 130-(graphCPU[i1]*1.2), x+1, 130-(graphCPU[i2]*1.2), TFT_GREEN);
    graphSprite.drawLine(x, 130-(graphGPU[i1]*1.2), x+1, 130-(graphGPU[i2]*1.2), TFT_MAGENTA);
    graphSprite.drawLine(x, 130-(graphRAM[i1]*1.2), x+1, 130-(graphRAM[i2]*1.2), TFT_CYAN);
  }
  graphSprite.pushSprite(0, 35);
  tft.fillRect(0, 175, 320, 65, TFT_BLACK);
  tft.setTextColor(TFT_WHITE); tft.drawCentreString(currentTime, 160, 185, 4);
  tft.setTextColor(thermalAlert ? TFT_RED : TFT_CYAN); tft.drawCentreString(activeApp, 160, 215, 2);
}

void drawDashboardView() {
  tft.fillRect(0, 0, 320, 240, 0x0000); 
  tft.setTextColor(TFT_CYAN); tft.drawCentreString("SYSTEM STATUS", 160, 5, 4);
  tft.drawFastHLine(0, 35, 320, TFT_WHITE);
  tft.setTextColor(TFT_ORANGE); tft.drawString("CPU TMP: " + String((int)cTempEMA) + "C", 10, 50, 4);
  tft.drawString("GPU TMP: " + String((int)gTempEMA) + "C", 10, 85, 4);
  tft.setTextColor(TFT_YELLOW); tft.drawString("DISK: " + String((int)diskEMA) + "%", 170, 50, 4);
  tft.drawString("VRAM: " + String((int)vramEMA) + "%", 170, 85, 4);
  tft.setTextColor(TFT_CYAN); tft.drawString("DL: " + String((int)netDEMA) + " KB/s", 10, 130, 4);
  tft.drawString("UP: " + String((int)netUEMA) + " KB/s", 10, 165, 4);
  tft.setTextColor(TFT_GREEN); tft.drawString("PING: " + String(currentPing) + " ms", 170, 130, 4);
  tft.setTextColor(TFT_WHITE); tft.drawString("TIME: " + currentTime, 10, 210, 4);
}

void drawProcessesView() {
  tft.fillRect(0, 0, 320, 240, 0x0000);
  tft.setTextColor(TFT_GREEN); tft.drawCentreString("TOP CPU", 80, 5, 4);
  tft.setTextColor(TFT_CYAN); tft.drawCentreString("TOP RAM", 240, 5, 4);
  tft.drawFastVLine(160, 0, 240, TFT_WHITE);
  tft.drawFastHLine(0, 35, 320, TFT_WHITE);
  
  tft.setTextSize(1);
  int y = 50;
  char buf[256]; topCpuStr.toCharArray(buf, 256);
  char* p = strtok(buf, ";");
  while(p != NULL) { tft.drawString(String(p), 10, y, 2); y += 30; p = strtok(NULL, ";"); }
  
  y = 50;
  topRamStr.toCharArray(buf, 256);
  p = strtok(buf, ";");
  while(p != NULL) { tft.drawString(String(p) + " GB", 170, y, 2); y += 30; p = strtok(NULL, ";"); }
}

void processIncoming(String line) {
  char buf[1024]; line.toCharArray(buf, 1024);
  char* p = strtok(buf, "|"); int i = 0; String pts[16];
  while (p != NULL && i < 16) { pts[i++] = String(p); p = strtok(NULL, "|"); }
  if (i < 15) return;

  cpuEMA = smooth(cpuEMA, pts[1].toFloat());
  ramEMA = smooth(ramEMA, pts[2].toFloat());
  gpuEMA = smooth(gpuEMA, pts[3].toFloat());
  cTempEMA = pts[4].toFloat(); gTempEMA = pts[5].toFloat();
  diskEMA = pts[6].toFloat(); netDEMA = pts[7].toFloat(); netUEMA = pts[8].toFloat();
  vramEMA = pts[9].toFloat(); currentPing = pts[10].toInt();
  currentTime = pts[11]; activeApp = pts[12];
  String seq_ts = pts[13];
  topCpuStr = pts[14]; topRamStr = pts[15];

  if (WiFi.status() == WL_CONNECTED) {
    udp.beginPacket(udp.remoteIP(), replyPort);
    udp.print("PONG|" + seq_ts);
    udp.endPacket();
  }

  graphCPU[graphIdx] = cpuEMA; graphGPU[graphIdx] = gpuEMA; graphRAM[graphIdx] = ramEMA;
  graphIdx = (graphIdx + 1) % 320;
  lastPacketTime = millis();
  connected = true;
  if (isSleeping) { isSleeping = false; setBrightness(brightnessLevel); }
  thermalAlert = (cTempEMA > 85 || gTempEMA > 85);
  setLED(thermalAlert, !thermalAlert, false);
}

void loop() {
  if (touch.touched()) {
    TS_Point p = touch.getPoint();
    if (p.z > 200) {
      viewMode = (viewMode + 1) % 3;
      tft.fillScreen(TFT_BLACK);
      delay(300);
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
  } else {
    tft.setTextColor(TFT_RED, TFT_BLACK);
    tft.drawCentreString("OFFLINE", 160, 110, 4);
    setLED(false, false, true);
  }
  
  if (millis() - lastPacketTime > 60000 && !isSleeping) { isSleeping = true; setBrightness(brightnessLevel); }
  if (millis() - lastPacketTime > 4000) connected = false;
}
