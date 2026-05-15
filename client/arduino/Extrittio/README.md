# Extrittio Arduino Client

ESP32 Arduino client for Extrittio. It publishes protobuf messages over
Zenoh-Pico and supports shadow-driven FOTA using Arduino ESP32 `Update`.

## Supported Target

- ESP32 Arduino core
- PlatformIO or Arduino IDE
- Zenoh-Pico transport dependency
- ArduinoJson dependency

UNO/AVR-class Arduino boards are not supported. They do not have the memory,
network stack, or flash partition model needed for this protocol and FOTA path.

## PlatformIO Setup

Add the library folder and Zenoh-Pico dependency to `platformio.ini`:

```ini
[env:esp32dev]
platform = espressif32
board = esp32dev
framework = arduino
lib_extra_dirs = ../../client/arduino
lib_deps =
  bblanchon/ArduinoJson
  https://github.com/eclipse-zenoh/zenoh-pico
```

Set an OTA-capable partition scheme in your board configuration. For many ESP32
boards this means selecting a partition table with two app slots.

## Arduino IDE Setup

Install ArduinoJson from Library Manager. Install Zenoh-Pico manually into your
Arduino libraries folder, then copy or symlink `client/arduino/Extrittio` into
the same folder.

## Minimal Sketch

```cpp
#include <WiFi.h>
#include <Extrittio.h>

ExtrittioClient extrittio;

void setup() {
  WiFi.begin("ssid", "password");
  while (WiFi.status() != WL_CONNECTED) {
    delay(250);
  }

  extrittio.begin(
    "arduino-esp32-001",
    "v1.0.0-arduino",
    "tcp/192.0.2.20:7447"
  );
}

void loop() {
  extrittio.loop();
  extrittio.publishTelemetry(22.0f, 45.0f, 98.0f);
  delay(5000);
}
```

## FOTA Flow

Enable FOTA before calling `begin()`:

```cpp
extrittio.enableFota(true);
extrittio.begin(DEVICE_ID, FIRMWARE_VERSION, ZENOH_ENDPOINT);
```

When Extrittio deploys firmware, the backend publishes an `ota` shadow delta.
The client downloads `firmware_url`, verifies `sha256` if present, writes the
next OTA partition, reports `downloading`, `verifying`, `installing`, and
`success`/`failed` through shadow reports, then restarts.

For uploaded firmware blobs, make sure `EXTRITTIO_PUBLIC_URL` on the backend is
an HTTP or HTTPS URL reachable by the ESP32, for example:

```bash
EXTRITTIO_PUBLIC_URL=http://192.0.2.20:8080
```

The download endpoint is already exempt from bearer auth in the backend so the
device can fetch the binary directly.

## API Summary

- `begin(deviceId, firmwareVersion, zenohEndpoint)`
- `loop()`
- `publishTelemetry(...)`
- `publishHeartbeat(...)`
- `publishLog(level, message)`
- `publishShadowReport(json, version)`
- `requestShadow()`
- `respondToCommand(correlationId, status, payloadJson)`
- `onCommand(callback)`
- `onShadowDelta(callback)`
- `enableFota(true)`
- `onOtaEvent(callback)`

## Notes

The Arduino client manually encodes and decodes Extrittio protobuf messages. It
does not require nanopb in the Arduino build. The message schema must stay in
sync with `common/src/protos/telemetry.proto`.
