#ifndef EXTRITTIO_ARDUINO_PROTOBUF_H
#define EXTRITTIO_ARDUINO_PROTOBUF_H

#include "Extrittio.h"

namespace extrittio_proto {

bool encodeTelemetry(const char *deviceId,
                     int64_t timestamp,
                     const ExtrittioTelemetry &telemetry,
                     uint8_t *buf,
                     size_t len,
                     size_t *written);

bool encodeHeartbeat(const char *deviceId,
                     int64_t timestamp,
                     const char *status,
                     const char *firmware,
                     int64_t uptimeSeconds,
                     uint8_t *buf,
                     size_t len,
                     size_t *written);

bool encodeShadowReport(const char *deviceId,
                        int64_t timestamp,
                        const char *stateJson,
                        int64_t version,
                        uint8_t *buf,
                        size_t len,
                        size_t *written);

bool encodeShadowGet(const char *deviceId,
                     uint8_t *buf,
                     size_t len,
                     size_t *written);

bool encodeCommandResponse(const char *deviceId,
                           const char *correlationId,
                           const char *status,
                           const char *payloadJson,
                           int64_t timestamp,
                           uint8_t *buf,
                           size_t len,
                           size_t *written);

bool encodeLog(const char *deviceId,
               int64_t timestamp,
               const char *level,
               const char *message,
               uint8_t *buf,
               size_t len,
               size_t *written);

bool decodeShadowDelta(const uint8_t *data, size_t len, ExtrittioShadowDelta *out);
bool decodeCommand(const uint8_t *data, size_t len, ExtrittioCommand *out);

}  // namespace extrittio_proto

#endif
