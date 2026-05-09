#include <TFT_eSPI.h>
#include <SPI.h>
#include <XPT2046_Touchscreen.h> 
#include <WiFi.h>
#include <WiFiUdp.h>

/* 
 * PC MONITOR PRO - CYD EDITION V10.5
 * FEATURES: 
 *   - Pro AMD Ryzen/Radeon Support
 *   - Auto-Switching (Serial/WiFi)
 *   - System Tray Integration (PC Side)
 *   - Safety Thermal Alerts
 */

// --- CONFIGURATION ---
const char* ssid = "YOUR_WIFI_SSID";
const char* password = "YOUR_WIFI_PASSWORD";
const int udpPort = 1234;
const int replyPort = 1235;

// CYD Hardware Pins
#define XPT2046_IRQ 36
#define XPT2046_MOSI 32
#define XPT2046_MISO 39
#define XPT2046_CLK 25
#define XPT2046_CS 33
#define BL_PIN 21      
#define LED_RED 4
#define LED_GREEN 16
#define LED_BLUE 17

SPIClass touchSPI = SPIClass(VSPI);
XPT2046_Touchscreen touch(XPT2046_CS, XPT2046_IRQ);

TFT_eSPI tft = TFT_eSPI();
TFT_eSprite graphSprite = TFT_eSprite(&tft);

WiFiUDP udp;

const float EMA_ALPHA = 0.25f;
unsigned long lastPacketTime = 0;
bool connected = false;
int brightnessLevel = 3; 
bool isSleeping = false;
bool thermalAlert = false;

float cpuEMA = 0, ramEMA = 0, gpuEMA = 0, vramEMA = 0;
float cTempEMA = 0, gTempEMA = 0, diskEMA = 0;
float netDEMA = 0, netUEMA = 0;
String activeApp = "Desktop";
String currentTime = "00:00:00";
int currentPing = 0;

bool graphMode = true;
float graphCPU[320], graphGPU[320], graphRAM[320];
int graphIdx = 0;

void setLED(bool r, bool g, bool b) {
  digitalWrite(LED_RED, !r);
  digitalWrite(LED_GREEN, !g);
  digitalWrite(LED_BLUE, !b);
}

void setBrightness(int level) {
  int duty = 64; 
  if (level == 1) duty = 128;
  else if (level == 2) duty = 192;
  else if (level == 3) duty = 255;
  if (isSleeping) duty = 10; 
  ledcWrite(BL_PIN, duty);
}

void setup() {
  Serial.begin(115200);
  
  pinMode(LED_RED, OUTPUT);
  pinMode(LED_GREEN, OUTPUT);
  pinMode(LED_BLUE, OUTPUT);
  setLED(false, false, false); // All off

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
  
  tft.setTextColor(TFT_CYAN, TFT_BLACK);
  tft.drawCentreString("PC MONITOR PRO", 160, 80, 4);
  tft.setTextColor(TFT_WHITE, TFT_BLACK);
  tft.drawCentreString("V10.5 INITIALIZING", 160, 120, 2);
  
  WiFi.mode(WIFI_STA);
  WiFi.begin(ssid, password);
  WiFi.setSleep(false);
  
  tft.setCursor(20, 170);
  tft.setTextColor(TFT_CYAN);
  tft.print("Link Start: ");
  unsigned long startWifi = millis();
  while (WiFi.status() != WL_CONNECTED && millis() - startWifi < 15000) { 
    delay(500); 
    tft.print("."); 
  }
  
  tft.fillScreen(TFT_BLACK);
  if (WiFi.status() == WL_CONNECTED) {
    udp.begin(udpPort);
    tft.setTextColor(TFT_GREEN);
    tft.drawCentreString("CONNECTED!", 160, 60, 4);
    tft.setTextColor(TFT_WHITE);
    tft.drawCentreString("IP: " + WiFi.localIP().toString(), 160, 110, 4);
  } else {
    tft.setTextColor(TFT_RED);
    tft.drawCentreString("WIFI FAILED", 160, 80, 4);
    tft.setTextColor(TFT_WHITE);
    tft.drawCentreString("RUNNING IN USB MODE", 160, 130, 2);
  }
  delay(2000);
  tft.fillScreen(TFT_BLACK);
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
    int i1 = (graphIdx + x) % 320;
    int i2 = (graphIdx + x + 1) % 320;
    graphSprite.drawLine(x, 130 - (graphCPU[i1]*1.2), x+1, 130 - (graphCPU[i2]*1.2), TFT_GREEN);
    graphSprite.drawLine(x, 130 - (graphGPU[i1]*1.2), x+1, 130 - (graphGPU[i2]*1.2), TFT_MAGENTA);
    graphSprite.drawLine(x, 130 - (graphRAM[i1]*1.2), x+1, 130 - (graphRAM[i2]*1.2), TFT_CYAN);
  }
  graphSprite.pushSprite(0, 35);
  
  tft.fillRect(0, 175, 320, 65, TFT_BLACK);
  tft.setTextColor(TFT_WHITE); tft.drawCentreString(currentTime, 160, 185, 4);
  tft.setTextColor(thermalAlert ? TFT_RED : TFT_CYAN); tft.drawCentreString(thermalAlert ? "OVERHEAT ALERT!" : activeApp, 160, 215, 2);
}

void drawDashboardView() {
  tft.fillRect(0, 0, 320, 240, 0x0000); 
  
  tft.setTextColor(thermalAlert ? TFT_RED : TFT_CYAN);
  tft.drawCentreString(thermalAlert ? "!!! THERMAL ALERT !!!" : "SYSTEM DASHBOARD", 160, 5, 4);
  tft.drawFastHLine(0, 35, 320, TFT_WHITE);

  tft.setTextColor(TFT_ORANGE);
  tft.drawString("CPU TEMP: " + String((int)cTempEMA) + "C", 10, 50, 4);
  tft.drawString("GPU TEMP: " + String((int)gTempEMA) + "C", 10, 85, 4);

  tft.setTextColor(TFT_YELLOW);
  tft.drawString("DISK: " + String((int)diskEMA) + "%", 170, 50, 4);
  tft.drawString("VRAM: " + String((int)vramEMA) + "%", 170, 85, 4);

  tft.setTextColor(TFT_CYAN);
  tft.drawString("DL: " + String((int)netDEMA) + " KB/s", 10, 130, 4);
  tft.drawString("UP: " + String((int)netUEMA) + " KB/s", 10, 165, 4);

  tft.setTextColor(TFT_GREEN);
  tft.drawString("PING: " + String(currentPing) + " ms", 170, 130, 4);
  tft.setTextColor(TFT_WHITE);
  tft.drawString("TIME: " + currentTime, 10, 210, 4);
  
  tft.setTextColor(TFT_CYAN);
  tft.drawRightString(activeApp, 310, 210, 2);
}

void processIncoming(String line) {
  char buf[512];
  line.toCharArray(buf, 512);
  char* p = strtok(buf, "|");
  int i = 0;
  String pts[14];
  while (p != NULL && i < 14) { pts[i++] = String(p); p = strtok(NULL, "|"); }
  if (i < 13) return;

  cpuEMA = smooth(cpuEMA, pts[1].toFloat());
  ramEMA = smooth(ramEMA, pts[2].toFloat());
  gpuEMA = smooth(gpuEMA, pts[3].toFloat());
  cTempEMA = pts[4].toFloat(); gTempEMA = pts[5].toFloat();
  diskEMA = pts[6].toFloat(); 
  netDEMA = pts[7].toFloat(); netUEMA = pts[8].toFloat();
  vramEMA = pts[9].toFloat();
  currentPing = pts[10].toInt();
  currentTime = pts[11];
  activeApp = pts[12];
  String seq_ts = pts[13];

  if (WiFi.status() == WL_CONNECTED) {
    udp.beginPacket(udp.remoteIP(), replyPort);
    udp.print("PONG|" + seq_ts);
    udp.endPacket();
  }

  graphCPU[graphIdx] = cpuEMA;
  graphGPU[graphIdx] = gpuEMA;
  graphRAM[graphIdx] = ramEMA;
  graphIdx = (graphIdx + 1) % 320;

  lastPacketTime = millis();
  connected = true;
  if (isSleeping) { isSleeping = false; setBrightness(brightnessLevel); }

  // Thermal Protection: Alert at 85C, Red Screen at 90C
  if (cTempEMA > 85 || gTempEMA > 85) {
    thermalAlert = true;
    setLED(true, false, false); // LED RED
  } else {
    thermalAlert = false;
    setLED(false, true, false); // LED GREEN
  }
}

void loop() {
  if (touch.touched()) {
    graphMode = !graphMode;
    tft.fillScreen(TFT_BLACK);
    delay(200);
  }

  if (WiFi.status() == WL_CONNECTED) {
    int packetSize = udp.parsePacket();
    if (packetSize) {
      char buf[512];
      int len = udp.read(buf, 512);
      if (len > 0) { buf[len] = 0; processIncoming(String(buf)); }
    }
  }
  if (Serial.available()) { processIncoming(Serial.readStringUntil('\n')); }

  if (connected) {
    if (graphMode) drawGraphView();
    else drawDashboardView();
  } else {
    tft.setTextColor(TFT_RED, TFT_BLACK);
    tft.drawCentreString("OFFLINE", 160, 110, 4);
    setLED(false, false, true); // LED BLUE (Waiting)
  }
  
  if (millis() - lastPacketTime > 60000 && !isSleeping) {
    isSleeping = true;
    setBrightness(brightnessLevel);
  }
  
  if (millis() - lastPacketTime > 4000) connected = false;
}
