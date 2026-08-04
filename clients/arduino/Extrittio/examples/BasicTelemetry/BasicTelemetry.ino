#include <WiFi.h>
#include <Extrittio.h>

const char *WIFI_SSID = "your-wifi-ssid";
const char *WIFI_PASSWORD = "your-wifi-password";

const char *DEVICE_ID = "arduino-esp32-001";
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

void onCommand(const ExtrittioCommand &command, void *) {
  if (command.command == "restart") {
    extrittio.respondToCommand(command.correlationId.c_str(), "succeeded", "{\"restarting\":true}");
    delay(250);
    ESP.restart();
  }

  extrittio.respondToCommand(command.correlationId.c_str(), "succeeded", "{}");
}

void setup() {
  Serial.begin(115200);
  connectWiFi();

  extrittio.onCommand(onCommand);
  extrittio.setHeartbeatInterval(30000);

  if (!extrittio.begin(DEVICE_ID, FIRMWARE_VERSION, ZENOH_ENDPOINT)) {
    Serial.println(extrittio.lastError());
  }
}

void loop() {
  extrittio.loop();

  if (millis() - lastTelemetryAt >= 5000) {
    lastTelemetryAt = millis();

    ExtrittioMetadata metadata[] = {
      {"source", "arduino"},
      {"board", "esp32"}
    };

    ExtrittioTelemetry telemetry;
    telemetry.temperature = 21.0f + random(-100, 100) / 100.0f;
    telemetry.humidity = 45.0f + random(-200, 200) / 100.0f;
    telemetry.batteryLevel = 95.0f;
    telemetry.metadata = metadata;
    telemetry.metadataCount = 2;

    if (!extrittio.publishTelemetry(telemetry)) {
      Serial.println(extrittio.lastError());
    }
  }
}
