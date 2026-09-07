#ifndef EXTRITTIO_ARDUINO_H
#define EXTRITTIO_ARDUINO_H

#include <Arduino.h>
#include <stddef.h>
#include <stdint.h>
#include <atomic>

#if !defined(ARDUINO_ARCH_ESP32)
#error "The Extrittio Arduino client currently supports ESP32 boards only."
#endif

#include <zenoh-pico.h>

#define EXTRITTIO_STATUS_ONLINE "online"
#define EXTRITTIO_STATUS_OFFLINE "offline"
#define EXTRITTIO_STATUS_WARNING "warning"

#define EXTRITTIO_OTA_DOWNLOADING "downloading"
#define EXTRITTIO_OTA_VERIFYING "verifying"
#define EXTRITTIO_OTA_INSTALLING "installing"
#define EXTRITTIO_OTA_REBOOTING "rebooting"
#define EXTRITTIO_OTA_SUCCESS "success"
#define EXTRITTIO_OTA_FAILED "failed"

struct ExtrittioMetadata {
    const char *key = nullptr;
    const char *value = nullptr;
};

struct ExtrittioTelemetry {
    float temperature = 0.0f;
    float humidity = 0.0f;
    float batteryLevel = 0.0f;
    double latitude = 0.0;
    double longitude = 0.0;
    float speed = 0.0f;
    float altitude = 0.0f;
    float heading = 0.0f;
    const ExtrittioMetadata *metadata = nullptr;
    size_t metadataCount = 0;
};

struct ExtrittioCommand {
    String command;
    String paramsJson;
    String correlationId;
};

struct ExtrittioShadowDelta {
    String deviceId;
    String deltaJson;
    int64_t version = 0;
};

struct ExtrittioOtaPayload {
    int64_t deploymentId = 0;
    String firmwareVersion;
    String firmwareUrl;
    int64_t firmwareUpdateId = 0;
    String sha256;
};

typedef void (*ExtrittioCommandCallback)(const ExtrittioCommand &command, void *userData);
typedef void (*ExtrittioShadowDeltaCallback)(const ExtrittioShadowDelta &delta, void *userData);
typedef void (*ExtrittioOtaEventCallback)(const char *status,
                                          const ExtrittioOtaPayload &payload,
                                          const char *detail,
                                          void *userData);

class ExtrittioClient {
public:
    ExtrittioClient();
    ~ExtrittioClient();

    bool begin(const char *deviceId, const char *firmwareVersion, const char *zenohEndpoint);
    void end();
    void loop();

    bool connected() const;
    const char *lastError() const;

    void setHeartbeatInterval(uint32_t intervalMs);
    void setFotaReadTimeout(uint32_t timeoutMs);
    void setTlsCaCert(const char *caCertPem);

    void enableFota(bool enabled = true);

    void onCommand(ExtrittioCommandCallback callback, void *userData = nullptr);
    void onShadowDelta(ExtrittioShadowDeltaCallback callback, void *userData = nullptr);
    void onOtaEvent(ExtrittioOtaEventCallback callback, void *userData = nullptr);

    bool publishTelemetry(const ExtrittioTelemetry &telemetry);
    bool publishTelemetry(float temperature, float humidity, float batteryLevel);
    bool publishHeartbeat(const char *status = EXTRITTIO_STATUS_ONLINE);
    bool publishLog(const char *level, const char *message);
    bool publishShadowReport(const char *stateJson, int64_t version = 0);
    bool requestShadow();
    bool respondToCommand(const char *correlationId,
                          const char *status,
                          const char *payloadJson = "{}");

private:
    String _deviceId;
    String _firmwareVersion;
    String _zenohEndpoint;
    String _lastError;
    const char *_tlsCaCertPem;

    bool _sessionOpen;
    bool _tasksStarted;
    bool _shadowSubscribed;
    bool _commandSubscribed;
    bool _fotaEnabled;
    bool _bootConfirmed = false;
    int64_t _confirmedAttemptId = 0;
    std::atomic<ExtrittioShadowDelta *> _pendingOta{nullptr};
    uint32_t _startupAt = 0;

    uint32_t _heartbeatIntervalMs;
    uint32_t _lastHeartbeatMs;
    uint32_t _fotaReadTimeoutMs;

    z_owned_session_t _session;
    z_owned_subscriber_t _shadowSubscriber;
    z_owned_subscriber_t _commandSubscriber;

    ExtrittioCommandCallback _commandCallback;
    void *_commandUserData;
    ExtrittioShadowDeltaCallback _shadowCallback;
    void *_shadowUserData;
    ExtrittioOtaEventCallback _otaEventCallback;
    void *_otaEventUserData;

    static void shadowSampleHandler(z_loaned_sample_t *sample, void *arg);
    static void commandSampleHandler(z_loaned_sample_t *sample, void *arg);

    void handleShadowSample(z_loaned_sample_t *sample);
    void handleCommandSample(z_loaned_sample_t *sample);

    bool subscribeShadowDeltas();
    bool subscribeCommands();
    bool publishProto(const char *topic, const uint8_t *payload, size_t length);
    bool buildTopic(char *buf, size_t len, const char *suffix) const;
    int64_t timestampMillis() const;
    int64_t uptimeSeconds() const;
    void setError(const String &error);

    bool parseOtaPayload(const char *deltaJson, ExtrittioOtaPayload *out) const;
    void confirmOtaBoot();
    bool maybeHandleFota(const ExtrittioShadowDelta &delta);
    bool performFota(const ExtrittioOtaPayload &payload,
                     const char *syncReportedStateJson,
                     int64_t syncVersion);
    bool reportOtaStatus(const char *status,
                         const ExtrittioOtaPayload &payload,
                         const char *error);
    void emitOtaEvent(const char *status,
                      const ExtrittioOtaPayload &payload,
                      const char *detail);
};

#endif
