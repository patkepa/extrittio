#include <WiFi.h>
#include <Extrittio.h>

const char *WIFI_SSID = "your-wifi-ssid";
const char *WIFI_PASSWORD = "your-wifi-password";

const char *DEVICE_ID = "arduino-esp32-fota-001";
const char *FIRMWARE_VERSION = "v1.0.0-arduino";
const char *ZENOH_ENDPOINT = "tcp/192.0.2.20:7447";

ExtrittioClient extrittio;

unsigned long lastTelemetryAt = 0;

void connectWiFi() {
  WiFi.mode(WIFI_STA);
  WiFi.begin(WIFI_SSID, WIFI_PASSWORD);
  while (WiFi.status() != WL_CONNECTED) {
    delay(250);
  }
}

void onOtaEvent(const char *status,
                const ExtrittioOtaPayload &payload,
                const char *detail,
                void *) {
  Serial.print("OTA ");
  Serial.print(status);
  Serial.print(" -> ");
  Serial.println(payload.firmwareVersion);
  if (detail != nullptr) {
    Serial.println(detail);
  }
}

void setup() {
  Serial.begin(115200);
  connectWiFi();

  extrittio.enableFota(true);
  extrittio.onOtaEvent(onOtaEvent);
  extrittio.setHeartbeatInterval(30000);

  if (!extrittio.begin(DEVICE_ID, FIRMWARE_VERSION, ZENOH_ENDPOINT)) {
    Serial.println(extrittio.lastError());
  }
}

void loop() {
  extrittio.loop();

  if (millis() - lastTelemetryAt >= 10000) {
    lastTelemetryAt = millis();
    extrittio.publishTelemetry(23.0f, 50.0f, 100.0f);
  }
}
