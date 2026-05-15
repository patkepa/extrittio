#include "ExtrittioProtobuf.h"

#include <string.h>

namespace {

constexpr uint8_t WireVarint = 0;
constexpr uint8_t WireFixed64 = 1;
constexpr uint8_t WireLengthDelimited = 2;
constexpr uint8_t WireFixed32 = 5;

class ProtoWriter {
public:
    ProtoWriter(uint8_t *buf, size_t len) : _buf(buf), _len(len), _pos(0) {}

    bool writeTag(uint32_t field, uint8_t wire) {
        return writeVarint((static_cast<uint64_t>(field) << 3) | wire);
    }

    bool writeString(uint32_t field, const char *value) {
        if (value == nullptr) {
            value = "";
        }
        const size_t valueLen = strlen(value);
        return writeTag(field, WireLengthDelimited) && writeVarint(valueLen) &&
               writeBytes(reinterpret_cast<const uint8_t *>(value), valueLen);
    }

    bool writeInt64(uint32_t field, int64_t value) {
        return writeTag(field, WireVarint) && writeVarint(static_cast<uint64_t>(value));
    }

    bool writeFloat(uint32_t field, float value) {
        uint32_t bits;
        memcpy(&bits, &value, sizeof(bits));
        return writeTag(field, WireFixed32) && writeFixed32(bits);
    }

    bool writeDouble(uint32_t field, double value) {
        uint64_t bits;
        memcpy(&bits, &value, sizeof(bits));
        return writeTag(field, WireFixed64) && writeFixed64(bits);
    }

    bool writeSubmessage(uint32_t field, const uint8_t *data, size_t len) {
        return writeTag(field, WireLengthDelimited) && writeVarint(len) && writeBytes(data, len);
    }

    size_t size() const { return _pos; }

private:
    bool writeVarint(uint64_t value) {
        while (value >= 0x80) {
            if (!writeByte(static_cast<uint8_t>(value | 0x80))) {
                return false;
            }
            value >>= 7;
        }
        return writeByte(static_cast<uint8_t>(value));
    }

    bool writeFixed32(uint32_t value) {
        return writeByte(static_cast<uint8_t>(value)) &&
               writeByte(static_cast<uint8_t>(value >> 8)) &&
               writeByte(static_cast<uint8_t>(value >> 16)) &&
               writeByte(static_cast<uint8_t>(value >> 24));
    }

    bool writeFixed64(uint64_t value) {
        for (int i = 0; i < 8; ++i) {
            if (!writeByte(static_cast<uint8_t>(value >> (8 * i)))) {
                return false;
            }
        }
        return true;
    }

    bool writeBytes(const uint8_t *data, size_t len) {
        if (_pos + len > _len) {
            return false;
        }
        memcpy(_buf + _pos, data, len);
        _pos += len;
        return true;
    }

    bool writeByte(uint8_t value) {
        if (_pos >= _len) {
            return false;
        }
        _buf[_pos++] = value;
        return true;
    }

    uint8_t *_buf;
    size_t _len;
    size_t _pos;
};

class ProtoReader {
public:
    ProtoReader(const uint8_t *data, size_t len) : _data(data), _len(len), _pos(0) {}

    bool done() const { return _pos >= _len; }

    bool readTag(uint32_t *field, uint8_t *wire) {
        uint64_t raw = 0;
        if (!readVarint(&raw)) {
            return false;
        }
        *field = static_cast<uint32_t>(raw >> 3);
        *wire = static_cast<uint8_t>(raw & 0x07);
        return *field != 0;
    }

    bool readString(String *out) {
        uint64_t valueLen = 0;
        if (!readVarint(&valueLen) || _pos + valueLen > _len) {
            return false;
        }
        out->remove(0);
        out->reserve(static_cast<unsigned int>(valueLen));
        for (uint64_t i = 0; i < valueLen; ++i) {
            *out += static_cast<char>(_data[_pos + i]);
        }
        _pos += static_cast<size_t>(valueLen);
        return true;
    }

    bool readLengthDelimited(const uint8_t **data, size_t *len) {
        uint64_t valueLen = 0;
        if (!readVarint(&valueLen) || _pos + valueLen > _len) {
            return false;
        }
        *data = _data + _pos;
        *len = static_cast<size_t>(valueLen);
        _pos += static_cast<size_t>(valueLen);
        return true;
    }

    bool readInt64(int64_t *out) {
        uint64_t raw = 0;
        if (!readVarint(&raw)) {
            return false;
        }
        *out = static_cast<int64_t>(raw);
        return true;
    }

    bool skip(uint8_t wire) {
        uint64_t len = 0;
        switch (wire) {
            case WireVarint:
                return readVarint(&len);
            case WireFixed64:
                return skipBytes(8);
            case WireLengthDelimited:
                return readVarint(&len) && skipBytes(static_cast<size_t>(len));
            case WireFixed32:
                return skipBytes(4);
            default:
                return false;
        }
    }

private:
    bool readVarint(uint64_t *out) {
        uint64_t value = 0;
        uint8_t shift = 0;
        while (_pos < _len && shift < 64) {
            const uint8_t byte = _data[_pos++];
            value |= static_cast<uint64_t>(byte & 0x7f) << shift;
            if ((byte & 0x80) == 0) {
                *out = value;
                return true;
            }
            shift += 7;
        }
        return false;
    }

    bool skipBytes(size_t count) {
        if (_pos + count > _len) {
            return false;
        }
        _pos += count;
        return true;
    }

    const uint8_t *_data;
    size_t _len;
    size_t _pos;
};

bool writeMetadataEntry(const ExtrittioMetadata &entry, uint8_t *buf, size_t len, size_t *written) {
    ProtoWriter writer(buf, len);
    if (!writer.writeString(1, entry.key) || !writer.writeString(2, entry.value)) {
        return false;
    }
    *written = writer.size();
    return true;
}

String jsonEscape(const String &value) {
    String escaped;
    escaped.reserve(value.length() + 8);
    for (size_t i = 0; i < value.length(); ++i) {
        const char c = value.charAt(i);
        switch (c) {
            case '"':
                escaped += "\\\"";
                break;
            case '\\':
                escaped += "\\\\";
                break;
            case '\b':
                escaped += "\\b";
                break;
            case '\f':
                escaped += "\\f";
                break;
            case '\n':
                escaped += "\\n";
                break;
            case '\r':
                escaped += "\\r";
                break;
            case '\t':
                escaped += "\\t";
                break;
            default:
                if (static_cast<uint8_t>(c) < 0x20) {
                    char tmp[7];
                    snprintf(tmp,
                             sizeof(tmp),
                             "\\u%04x",
                             static_cast<unsigned int>(static_cast<uint8_t>(c)));
                    escaped += tmp;
                } else {
                    escaped += c;
                }
                break;
        }
    }
    return escaped;
}

bool appendParamJson(String *json, const String &key, const String &value, bool *first) {
    if (!*first) {
        *json += ',';
    }
    *first = false;
    *json += '"';
    *json += jsonEscape(key);
    *json += "\":\"";
    *json += jsonEscape(value);
    *json += '"';
    return true;
}

bool decodeCommandParamEntry(const uint8_t *data,
                             size_t len,
                             String *key,
                             String *value) {
    ProtoReader reader(data, len);
    while (!reader.done()) {
        uint32_t field = 0;
        uint8_t wire = 0;
        if (!reader.readTag(&field, &wire)) {
            return false;
        }
        if (field == 1 && wire == WireLengthDelimited) {
            if (!reader.readString(key)) {
                return false;
            }
        } else if (field == 2 && wire == WireLengthDelimited) {
            if (!reader.readString(value)) {
                return false;
            }
        } else if (!reader.skip(wire)) {
            return false;
        }
    }
    return true;
}

}  // namespace

namespace extrittio_proto {

bool encodeTelemetry(const char *deviceId,
                     int64_t timestamp,
                     const ExtrittioTelemetry &telemetry,
                     uint8_t *buf,
                     size_t len,
                     size_t *written) {
    ProtoWriter writer(buf, len);
    if (!writer.writeString(1, deviceId) ||
        !writer.writeInt64(2, timestamp) ||
        !writer.writeFloat(3, telemetry.temperature) ||
        !writer.writeFloat(4, telemetry.humidity) ||
        !writer.writeFloat(5, telemetry.batteryLevel)) {
        return false;
    }

    const size_t metadataCount = telemetry.metadata == nullptr
                                     ? 0
                                     : min(telemetry.metadataCount, static_cast<size_t>(8));
    for (size_t i = 0; i < metadataCount; ++i) {
        uint8_t entry[384];
        size_t entryLen = 0;
        if (!writeMetadataEntry(telemetry.metadata[i], entry, sizeof(entry), &entryLen) ||
            !writer.writeSubmessage(6, entry, entryLen)) {
            return false;
        }
    }

    if (!writer.writeDouble(7, telemetry.latitude) ||
        !writer.writeDouble(8, telemetry.longitude) ||
        !writer.writeFloat(9, telemetry.speed) ||
        !writer.writeFloat(10, telemetry.altitude) ||
        !writer.writeFloat(11, telemetry.heading)) {
        return false;
    }

    *written = writer.size();
    return true;
}

bool encodeHeartbeat(const char *deviceId,
                     int64_t timestamp,
                     const char *status,
                     const char *firmware,
                     int64_t uptimeSeconds,
                     uint8_t *buf,
                     size_t len,
                     size_t *written) {
    ProtoWriter writer(buf, len);
    if (!writer.writeString(1, deviceId) ||
        !writer.writeInt64(2, timestamp) ||
        !writer.writeString(3, status) ||
        !writer.writeString(4, firmware) ||
        !writer.writeInt64(5, uptimeSeconds)) {
        return false;
    }
    *written = writer.size();
    return true;
}

bool encodeShadowReport(const char *deviceId,
                        int64_t timestamp,
                        const char *stateJson,
                        int64_t version,
                        uint8_t *buf,
                        size_t len,
                        size_t *written) {
    ProtoWriter writer(buf, len);
    if (!writer.writeString(1, deviceId) ||
        !writer.writeInt64(2, timestamp) ||
        !writer.writeString(3, stateJson) ||
        !writer.writeInt64(4, version)) {
        return false;
    }
    *written = writer.size();
    return true;
}

bool encodeShadowGet(const char *deviceId,
                     uint8_t *buf,
                     size_t len,
                     size_t *written) {
    ProtoWriter writer(buf, len);
    if (!writer.writeString(1, deviceId)) {
        return false;
    }
    *written = writer.size();
    return true;
}

bool encodeCommandResponse(const char *deviceId,
                           const char *correlationId,
                           const char *status,
                           const char *payloadJson,
                           int64_t timestamp,
                           uint8_t *buf,
                           size_t len,
                           size_t *written) {
    ProtoWriter writer(buf, len);
    if (!writer.writeString(1, correlationId) ||
        !writer.writeString(2, deviceId) ||
        !writer.writeString(3, status) ||
        !writer.writeString(4, payloadJson) ||
        !writer.writeInt64(5, timestamp)) {
        return false;
    }
    *written = writer.size();
    return true;
}

bool encodeLog(const char *deviceId,
               int64_t timestamp,
               const char *level,
               const char *message,
               uint8_t *buf,
               size_t len,
               size_t *written) {
    ProtoWriter writer(buf, len);
    if (!writer.writeString(1, deviceId) ||
        !writer.writeInt64(2, timestamp) ||
        !writer.writeString(3, level) ||
        !writer.writeString(4, message)) {
        return false;
    }
    *written = writer.size();
    return true;
}

bool decodeShadowDelta(const uint8_t *data, size_t len, ExtrittioShadowDelta *out) {
    ProtoReader reader(data, len);
    while (!reader.done()) {
        uint32_t field = 0;
        uint8_t wire = 0;
        if (!reader.readTag(&field, &wire)) {
            return false;
        }
        if (field == 1 && wire == WireLengthDelimited) {
            if (!reader.readString(&out->deviceId)) {
                return false;
            }
        } else if (field == 2 && wire == WireLengthDelimited) {
            if (!reader.readString(&out->deltaJson)) {
                return false;
            }
        } else if (field == 3 && wire == WireVarint) {
            if (!reader.readInt64(&out->version)) {
                return false;
            }
        } else if (!reader.skip(wire)) {
            return false;
        }
    }
    return out->deviceId.length() > 0;
}

bool decodeCommand(const uint8_t *data, size_t len, ExtrittioCommand *out) {
    ProtoReader reader(data, len);
    String paramsJson = "{";
    bool firstParam = true;

    while (!reader.done()) {
        uint32_t field = 0;
        uint8_t wire = 0;
        if (!reader.readTag(&field, &wire)) {
            return false;
        }
        if (field == 1 && wire == WireLengthDelimited) {
            if (!reader.readString(&out->command)) {
                return false;
            }
        } else if (field == 2 && wire == WireLengthDelimited) {
            const uint8_t *entryData = nullptr;
            size_t entryLen = 0;
            if (!reader.readLengthDelimited(&entryData, &entryLen)) {
                return false;
            }
            String key;
            String value;
            if (!decodeCommandParamEntry(entryData, entryLen, &key, &value)) {
                return false;
            }
            appendParamJson(&paramsJson, key, value, &firstParam);
        } else if (field == 3 && wire == WireLengthDelimited) {
            if (!reader.readString(&out->correlationId)) {
                return false;
            }
        } else if (!reader.skip(wire)) {
            return false;
        }
    }

    paramsJson += '}';
    out->paramsJson = paramsJson;
    return out->command.length() > 0;
}

}  // namespace extrittio_proto
