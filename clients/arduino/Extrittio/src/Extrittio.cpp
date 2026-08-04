#include "Extrittio.h"

#include "ExtrittioProtobuf.h"

#include <ArduinoJson.h>
#include <HTTPClient.h>
#include <Update.h>
#include <WiFiClient.h>
#include <WiFiClientSecure.h>
#include <mbedtls/sha256.h>
#include <time.h>

namespace {

constexpr size_t ProtoBufSize = 2048;
constexpr size_t TopicBufSize = 160;

bool isHttpsUrl(const String &url) {
    return url.startsWith("https://");
}

String sha256ToHex(const uint8_t hash[32]) {
    static const char *hex = "0123456789abcdef";
    String out;
    out.reserve(64);
    for (size_t i = 0; i < 32; ++i) {
        out += hex[(hash[i] >> 4) & 0x0f];
        out += hex[hash[i] & 0x0f];
    }
    return out;
}

String updateErrorString() {
    String err;
    err.reserve(96);
    err += "Update failed: ";
    err += Update.errorString();
    return err;
}

}  // namespace

ExtrittioClient::ExtrittioClient()
    : _tlsCaCertPem(nullptr),
      _sessionOpen(false),
      _tasksStarted(false),
      _shadowSubscribed(false),
      _commandSubscribed(false),
      _fotaEnabled(false),
      _heartbeatIntervalMs(30000),
      _lastHeartbeatMs(0),
      _fotaReadTimeoutMs(30000),
      _commandCallback(nullptr),
      _commandUserData(nullptr),
      _shadowCallback(nullptr),
      _shadowUserData(nullptr),
      _otaEventCallback(nullptr),
      _otaEventUserData(nullptr) {}

ExtrittioClient::~ExtrittioClient() {
    end();
}

bool ExtrittioClient::begin(const char *deviceId,
                            const char *firmwareVersion,
                            const char *zenohEndpoint) {
    if (deviceId == nullptr || firmwareVersion == nullptr || zenohEndpoint == nullptr) {
        setError("deviceId, firmwareVersion, and zenohEndpoint are required");
        return false;
    }

    _deviceId = deviceId;
    _firmwareVersion = firmwareVersion;
    _zenohEndpoint = zenohEndpoint;
    _lastError = "";

    z_owned_config_t config;
    z_config_default(&config);
    zp_config_insert(z_loan_mut(config), Z_CONFIG_CONNECT_KEY, zenohEndpoint);

    if (z_open(&_session, z_move(config), nullptr) != 0) {
        setError("failed to open Zenoh session");
        return false;
    }
    _sessionOpen = true;

    if (zp_start_read_task(z_loan_mut(_session), nullptr) != 0 ||
        zp_start_lease_task(z_loan_mut(_session), nullptr) != 0) {
        setError("failed to start Zenoh-Pico read/lease tasks");
        end();
        return false;
    }
    _tasksStarted = true;

    if (!subscribeShadowDeltas()) {
        end();
        return false;
    }
    if (!subscribeCommands()) {
        end();
        return false;
    }

    publishHeartbeat();
    requestShadow();
    _lastHeartbeatMs = millis();
    return true;
}

void ExtrittioClient::end() {
    if (!_sessionOpen) {
        return;
    }

    if (_shadowSubscribed) {
        z_drop(z_move(_shadowSubscriber));
        _shadowSubscribed = false;
    }
    if (_commandSubscribed) {
        z_drop(z_move(_commandSubscriber));
        _commandSubscribed = false;
    }
    z_drop(z_move(_session));
    _sessionOpen = false;
    _tasksStarted = false;
}

void ExtrittioClient::loop() {
    if (!_sessionOpen) {
        return;
    }

    const uint32_t now = millis();
    if (now - _lastHeartbeatMs >= _heartbeatIntervalMs) {
        publishHeartbeat();
        _lastHeartbeatMs = now;
    }
}

bool ExtrittioClient::connected() const {
    return _sessionOpen;
}

const char *ExtrittioClient::lastError() const {
    return _lastError.c_str();
}

void ExtrittioClient::setHeartbeatInterval(uint32_t intervalMs) {
    _heartbeatIntervalMs = intervalMs;
}

void ExtrittioClient::setFotaReadTimeout(uint32_t timeoutMs) {
    _fotaReadTimeoutMs = timeoutMs;
}

void ExtrittioClient::setTlsCaCert(const char *caCertPem) {
    _tlsCaCertPem = caCertPem;
}

void ExtrittioClient::enableFota(bool enabled) {
    _fotaEnabled = enabled;
}

void ExtrittioClient::onCommand(ExtrittioCommandCallback callback, void *userData) {
    _commandCallback = callback;
    _commandUserData = userData;
}

void ExtrittioClient::onShadowDelta(ExtrittioShadowDeltaCallback callback, void *userData) {
    _shadowCallback = callback;
    _shadowUserData = userData;
}

void ExtrittioClient::onOtaEvent(ExtrittioOtaEventCallback callback, void *userData) {
    _otaEventCallback = callback;
    _otaEventUserData = userData;
}

bool ExtrittioClient::publishTelemetry(const ExtrittioTelemetry &telemetry) {
    if (!_sessionOpen) {
        setError("Zenoh session is not open");
        return false;
    }

    uint8_t buf[ProtoBufSize];
    size_t written = 0;
    if (!extrittio_proto::encodeTelemetry(_deviceId.c_str(),
                                          timestampMillis(),
                                          telemetry,
                                          buf,
                                          sizeof(buf),
                                          &written)) {
        setError("failed to encode telemetry");
        return false;
    }

    char topic[TopicBufSize];
    if (!buildTopic(topic, sizeof(topic), "telemetry")) {
        return false;
    }
    return publishProto(topic, buf, written);
}

bool ExtrittioClient::publishTelemetry(float temperature,
                                       float humidity,
                                       float batteryLevel) {
    ExtrittioTelemetry telemetry;
    telemetry.temperature = temperature;
    telemetry.humidity = humidity;
    telemetry.batteryLevel = batteryLevel;
    return publishTelemetry(telemetry);
}

bool ExtrittioClient::publishHeartbeat(const char *status) {
    if (!_sessionOpen) {
        setError("Zenoh session is not open");
        return false;
    }

    uint8_t buf[512];
    size_t written = 0;
    if (!extrittio_proto::encodeHeartbeat(_deviceId.c_str(),
                                          timestampMillis(),
                                          status,
                                          _firmwareVersion.c_str(),
                                          uptimeSeconds(),
                                          buf,
                                          sizeof(buf),
                                          &written)) {
        setError("failed to encode heartbeat");
        return false;
    }

    char topic[TopicBufSize];
    if (!buildTopic(topic, sizeof(topic), "heartbeat")) {
        return false;
    }
    return publishProto(topic, buf, written);
}

bool ExtrittioClient::publishLog(const char *level, const char *message) {
    if (!_sessionOpen) {
        setError("Zenoh session is not open");
        return false;
    }

    uint8_t buf[768];
    size_t written = 0;
    if (!extrittio_proto::encodeLog(_deviceId.c_str(),
                                    timestampMillis(),
                                    level,
                                    message,
                                    buf,
                                    sizeof(buf),
                                    &written)) {
        setError("failed to encode log");
        return false;
    }

    char topic[TopicBufSize];
    if (!buildTopic(topic, sizeof(topic), "logs")) {
        return false;
    }
    return publishProto(topic, buf, written);
}

bool ExtrittioClient::publishShadowReport(const char *stateJson, int64_t version) {
    if (!_sessionOpen) {
        setError("Zenoh session is not open");
        return false;
    }

    uint8_t buf[ProtoBufSize];
    size_t written = 0;
    if (!extrittio_proto::encodeShadowReport(_deviceId.c_str(),
                                             timestampMillis(),
                                             stateJson,
                                             version,
                                             buf,
                                             sizeof(buf),
                                             &written)) {
        setError("failed to encode shadow report");
        return false;
    }

    char topic[TopicBufSize];
    if (!buildTopic(topic, sizeof(topic), "shadow/report")) {
        return false;
    }
    return publishProto(topic, buf, written);
}

bool ExtrittioClient::requestShadow() {
    if (!_sessionOpen) {
        setError("Zenoh session is not open");
        return false;
    }

    uint8_t buf[128];
    size_t written = 0;
    if (!extrittio_proto::encodeShadowGet(_deviceId.c_str(), buf, sizeof(buf), &written)) {
        setError("failed to encode shadow get");
        return false;
    }

    char topic[TopicBufSize];
    if (!buildTopic(topic, sizeof(topic), "shadow/get")) {
        return false;
    }
    return publishProto(topic, buf, written);
}

bool ExtrittioClient::respondToCommand(const char *correlationId,
                                       const char *status,
                                       const char *payloadJson) {
    if (!_sessionOpen) {
        setError("Zenoh session is not open");
        return false;
    }

    uint8_t buf[1024];
    size_t written = 0;
    if (!extrittio_proto::encodeCommandResponse(_deviceId.c_str(),
                                                correlationId,
                                                status,
                                                payloadJson,
                                                timestampMillis(),
                                                buf,
                                                sizeof(buf),
                                                &written)) {
        setError("failed to encode command response");
        return false;
    }

    char topic[TopicBufSize];
    if (!buildTopic(topic, sizeof(topic), "commands/response")) {
        return false;
    }
    return publishProto(topic, buf, written);
}

void ExtrittioClient::shadowSampleHandler(z_loaned_sample_t *sample, void *arg) {
    if (arg == nullptr) {
        return;
    }
    static_cast<ExtrittioClient *>(arg)->handleShadowSample(sample);
}

void ExtrittioClient::commandSampleHandler(z_loaned_sample_t *sample, void *arg) {
    if (arg == nullptr) {
        return;
    }
    static_cast<ExtrittioClient *>(arg)->handleCommandSample(sample);
}

void ExtrittioClient::handleShadowSample(z_loaned_sample_t *sample) {
    z_owned_slice_t slice;
    z_bytes_to_slice(z_sample_payload(sample), &slice);
    const z_loaned_slice_t *loaned = z_slice_loan(&slice);

    ExtrittioShadowDelta delta;
    const bool ok = extrittio_proto::decodeShadowDelta(z_slice_data(loaned),
                                                       z_slice_len(loaned),
                                                       &delta);
    z_slice_drop(z_slice_move(&slice));

    if (!ok) {
        setError("failed to decode shadow delta");
        return;
    }

    bool fotaHandled = false;
    if (_fotaEnabled) {
        fotaHandled = maybeHandleFota(delta);
    }

    if (_shadowCallback != nullptr) {
        _shadowCallback(delta, _shadowUserData);
    }

    if (!fotaHandled && delta.deltaJson.length() > 0) {
        publishShadowReport(delta.deltaJson.c_str(), delta.version);
    }
}

void ExtrittioClient::handleCommandSample(z_loaned_sample_t *sample) {
    z_owned_slice_t slice;
    z_bytes_to_slice(z_sample_payload(sample), &slice);
    const z_loaned_slice_t *loaned = z_slice_loan(&slice);

    ExtrittioCommand command;
    const bool ok = extrittio_proto::decodeCommand(z_slice_data(loaned),
                                                   z_slice_len(loaned),
                                                   &command);
    z_slice_drop(z_slice_move(&slice));

    if (!ok) {
        setError("failed to decode command");
        return;
    }

    if (_commandCallback != nullptr) {
        _commandCallback(command, _commandUserData);
    }
}

bool ExtrittioClient::subscribeShadowDeltas() {
    if (!_sessionOpen || _shadowSubscribed) {
        return _shadowSubscribed;
    }

    char topic[TopicBufSize];
    if (!buildTopic(topic, sizeof(topic), "shadow/delta")) {
        return false;
    }

    z_view_keyexpr_t keyexpr;
    z_view_keyexpr_from_str(&keyexpr, topic);

    z_owned_closure_sample_t closure;
    z_closure(&closure, shadowSampleHandler, nullptr, this);

    if (z_declare_subscriber(z_loan(_session),
                             &_shadowSubscriber,
                             z_loan(keyexpr),
                             z_move(closure),
                             nullptr) != 0) {
        setError("failed to subscribe to shadow deltas");
        return false;
    }
    _shadowSubscribed = true;
    return true;
}

bool ExtrittioClient::subscribeCommands() {
    if (!_sessionOpen || _commandSubscribed) {
        return _commandSubscribed;
    }

    char topic[TopicBufSize];
    if (!buildTopic(topic, sizeof(topic), "commands")) {
        return false;
    }

    z_view_keyexpr_t keyexpr;
    z_view_keyexpr_from_str(&keyexpr, topic);

    z_owned_closure_sample_t closure;
    z_closure(&closure, commandSampleHandler, nullptr, this);

    if (z_declare_subscriber(z_loan(_session),
                             &_commandSubscriber,
                             z_loan(keyexpr),
                             z_move(closure),
                             nullptr) != 0) {
        setError("failed to subscribe to commands");
        return false;
    }
    _commandSubscribed = true;
    return true;
}

bool ExtrittioClient::publishProto(const char *topic,
                                   const uint8_t *payload,
                                   size_t length) {
    z_view_keyexpr_t keyexpr;
    z_view_keyexpr_from_str(&keyexpr, topic);

    z_owned_bytes_t bytes;
    z_bytes_copy_from_buf(&bytes, payload, length);

    z_put_options_t options;
    z_put_options_default(&options);

    if (z_put(z_loan(_session), z_loan(keyexpr), z_move(bytes), &options) != 0) {
        setError(String("failed to publish to ") + topic);
        return false;
    }
    return true;
}

bool ExtrittioClient::buildTopic(char *buf, size_t len, const char *suffix) const {
    const int written = snprintf(buf,
                                 len,
                                 "extrittio/devices/%s/%s",
                                 _deviceId.c_str(),
                                 suffix);
    if (written < 0 || static_cast<size_t>(written) >= len) {
        const_cast<ExtrittioClient *>(this)->setError("topic buffer too small");
        return false;
    }
    return true;
}

int64_t ExtrittioClient::timestampMillis() const {
    const time_t wallClock = time(nullptr);
    if (wallClock > 1600000000) {
        return static_cast<int64_t>(wallClock) * 1000;
    }
    return static_cast<int64_t>(millis());
}

int64_t ExtrittioClient::uptimeSeconds() const {
    return static_cast<int64_t>(millis() / 1000UL);
}

void ExtrittioClient::setError(const String &error) {
    _lastError = error;
}

bool ExtrittioClient::parseOtaPayload(const char *deltaJson, ExtrittioOtaPayload *out) const {
    StaticJsonDocument<1536> doc;
    const DeserializationError err = deserializeJson(doc, deltaJson);
    if (err) {
        return false;
    }

    JsonVariant otaVariant = doc["ota"];
    if (!otaVariant.is<JsonObject>()) {
        return false;
    }
    JsonObject ota = otaVariant.as<JsonObject>();

    const char *firmwareVersion = ota["firmware_version"];
    const char *firmwareUrl = ota["firmware_url"];
    if (firmwareVersion == nullptr || firmwareUrl == nullptr) {
        return false;
    }

    out->firmwareVersion = firmwareVersion;
    out->firmwareUrl = firmwareUrl;
    out->firmwareUpdateId = ota["firmware_update_id"] | 0;
    out->sha256 = ota["sha256"] | "";
    return true;
}

bool ExtrittioClient::maybeHandleFota(const ExtrittioShadowDelta &delta) {
    ExtrittioOtaPayload payload;
    if (!parseOtaPayload(delta.deltaJson.c_str(), &payload)) {
        return false;
    }
    performFota(payload, delta.deltaJson.c_str(), delta.version);
    return true;
}

bool ExtrittioClient::performFota(const ExtrittioOtaPayload &payload,
                                  const char *syncReportedStateJson,
                                  int64_t syncVersion) {
    if (payload.firmwareVersion == _firmwareVersion) {
        emitOtaEvent(EXTRITTIO_OTA_SUCCESS, payload, "already running requested firmware");
        reportOtaStatus(EXTRITTIO_OTA_SUCCESS, payload, nullptr);
        publishShadowReport(syncReportedStateJson, syncVersion);
        return true;
    }

    emitOtaEvent(EXTRITTIO_OTA_DOWNLOADING, payload, nullptr);
    if (!reportOtaStatus(EXTRITTIO_OTA_DOWNLOADING, payload, nullptr)) {
        return false;
    }

    HTTPClient http;
    WiFiClient plainClient;
    WiFiClientSecure secureClient;

    if (isHttpsUrl(payload.firmwareUrl)) {
        if (_tlsCaCertPem != nullptr && strlen(_tlsCaCertPem) > 0) {
            secureClient.setCACert(_tlsCaCertPem);
        } else {
            secureClient.setInsecure();
        }
        if (!http.begin(secureClient, payload.firmwareUrl)) {
            reportOtaStatus(EXTRITTIO_OTA_FAILED, payload, "HTTP begin failed");
            return false;
        }
    } else if (!http.begin(plainClient, payload.firmwareUrl)) {
        reportOtaStatus(EXTRITTIO_OTA_FAILED, payload, "HTTP begin failed");
        return false;
    }

    const int code = http.GET();
    if (code != HTTP_CODE_OK) {
        String err = "HTTP GET failed: ";
        err += code;
        http.end();
        reportOtaStatus(EXTRITTIO_OTA_FAILED, payload, err.c_str());
        return false;
    }

    const int contentLength = http.getSize();
    if (!Update.begin(contentLength > 0 ? contentLength : UPDATE_SIZE_UNKNOWN)) {
        String err = updateErrorString();
        http.end();
        reportOtaStatus(EXTRITTIO_OTA_FAILED, payload, err.c_str());
        return false;
    }

    WiFiClient *stream = http.getStreamPtr();
    uint8_t buf[1024];
    int remaining = contentLength;
    unsigned long lastDataAt = millis();

    mbedtls_sha256_context shaCtx;
    mbedtls_sha256_init(&shaCtx);
    mbedtls_sha256_starts(&shaCtx, 0);

    while ((http.connected() || stream->available() > 0) &&
           (contentLength < 0 || remaining > 0)) {
        const int available = stream->available();
        if (available <= 0) {
            if (millis() - lastDataAt > _fotaReadTimeoutMs) {
                mbedtls_sha256_free(&shaCtx);
                Update.abort();
                http.end();
                reportOtaStatus(EXTRITTIO_OTA_FAILED, payload, "firmware download timed out");
                return false;
            }
            delay(1);
            continue;
        }

        int toRead = min(available, static_cast<int>(sizeof(buf)));
        if (contentLength >= 0) {
            toRead = min(toRead, remaining);
        }

        const int read = stream->readBytes(buf, toRead);
        if (read <= 0) {
            continue;
        }

        lastDataAt = millis();
        if (Update.write(buf, read) != static_cast<size_t>(read)) {
            mbedtls_sha256_free(&shaCtx);
            String err = updateErrorString();
            Update.abort();
            http.end();
            reportOtaStatus(EXTRITTIO_OTA_FAILED, payload, err.c_str());
            return false;
        }
        mbedtls_sha256_update(&shaCtx, buf, read);

        if (contentLength >= 0) {
            remaining -= read;
        }
    }

    http.end();

    if (contentLength >= 0 && remaining != 0) {
        mbedtls_sha256_free(&shaCtx);
        Update.abort();
        reportOtaStatus(EXTRITTIO_OTA_FAILED, payload, "firmware download incomplete");
        return false;
    }

    emitOtaEvent(EXTRITTIO_OTA_VERIFYING, payload, nullptr);
    reportOtaStatus(EXTRITTIO_OTA_VERIFYING, payload, nullptr);

    uint8_t hash[32];
    mbedtls_sha256_finish(&shaCtx, hash);
    mbedtls_sha256_free(&shaCtx);

    if (payload.sha256.length() > 0) {
        const String actualSha = sha256ToHex(hash);
        if (!actualSha.equalsIgnoreCase(payload.sha256)) {
            Update.abort();
            reportOtaStatus(EXTRITTIO_OTA_FAILED, payload, "SHA-256 mismatch");
            return false;
        }
    }

    emitOtaEvent(EXTRITTIO_OTA_INSTALLING, payload, nullptr);
    reportOtaStatus(EXTRITTIO_OTA_INSTALLING, payload, nullptr);

    if (!Update.end(true)) {
        String err = updateErrorString();
        reportOtaStatus(EXTRITTIO_OTA_FAILED, payload, err.c_str());
        return false;
    }

    emitOtaEvent(EXTRITTIO_OTA_SUCCESS, payload, nullptr);
    reportOtaStatus(EXTRITTIO_OTA_SUCCESS, payload, nullptr);

    delay(300);
    publishShadowReport(syncReportedStateJson, syncVersion);
    delay(500);
    ESP.restart();
    return true;
}

bool ExtrittioClient::reportOtaStatus(const char *status,
                                      const ExtrittioOtaPayload &payload,
                                      const char *error) {
    StaticJsonDocument<768> doc;
    JsonObject ota = doc.createNestedObject("ota");
    ota["status"] = status;
    ota["firmware_version"] = payload.firmwareVersion;
    if (payload.firmwareUpdateId != 0) {
        ota["firmware_update_id"] = payload.firmwareUpdateId;
    }
    if (error != nullptr && strlen(error) > 0) {
        ota["error"] = error;
    }

    String stateJson;
    serializeJson(doc, stateJson);
    return publishShadowReport(stateJson.c_str(), 0);
}

void ExtrittioClient::emitOtaEvent(const char *status,
                                   const ExtrittioOtaPayload &payload,
                                   const char *detail) {
    if (_otaEventCallback != nullptr) {
        _otaEventCallback(status, payload, detail, _otaEventUserData);
    }
}
