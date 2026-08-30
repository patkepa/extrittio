#include <assert.h>
#include <string.h>
#include <stdio.h>
#include <stdbool.h>
#include "extrittio/telemetry.h"
#include "telemetry.pb.h"
#include <pb_decode.h>

typedef struct {
    char *buf;
    size_t len;
} string_decode_ctx_t;

typedef struct {
    int count;
    char key[64];
    char value[128];
} metadata_decode_ctx_t;

static bool decode_string(pb_istream_t *stream, const pb_field_t *field, void **arg) {
    (void)field;
    string_decode_ctx_t *ctx = (string_decode_ctx_t *)(*arg);
    size_t n = stream->bytes_left;
    if (n >= ctx->len) {
        n = ctx->len - 1;
    }
    if (!pb_read(stream, (uint8_t *)ctx->buf, n)) {
        return false;
    }
    ctx->buf[n] = '\0';
    if (stream->bytes_left > 0) {
        uint8_t discard[16];
        while (stream->bytes_left > 0) {
            size_t chunk = stream->bytes_left > sizeof(discard) ? sizeof(discard) : stream->bytes_left;
            if (!pb_read(stream, discard, chunk)) {
                return false;
            }
        }
    }
    return true;
}

static bool decode_metadata(pb_istream_t *stream, const pb_field_t *field, void **arg) {
    (void)field;
    metadata_decode_ctx_t *ctx = (metadata_decode_ctx_t *)(*arg);
    extrittio_DeviceTelemetry_MetadataEntry entry =
        extrittio_DeviceTelemetry_MetadataEntry_init_zero;

    string_decode_ctx_t key_ctx = {.buf = ctx->key, .len = sizeof(ctx->key)};
    string_decode_ctx_t value_ctx = {.buf = ctx->value, .len = sizeof(ctx->value)};
    entry.key.funcs.decode = decode_string;
    entry.key.arg = &key_ctx;
    entry.value.funcs.decode = decode_string;
    entry.value.arg = &value_ctx;

    if (!pb_decode(stream, extrittio_DeviceTelemetry_MetadataEntry_fields, &entry)) {
        return false;
    }

    ctx->count++;
    return true;
}

static void test_encode_and_decode(void) {
    extrittio_metadata_entry_t metadata[] = {
        {.key = "sensor_profile", .value = "environmental"},
    };
    extrittio_telemetry_t t = {
        .device_id = "dev-001",
        .timestamp = 1700000000000LL,
        .temperature = 22.5f,
        .humidity = 55.0f,
        .battery_level = 98.0f,
        .latitude = 52.1,
        .longitude = 21.0,
        .metadata_count = 1,
        .metadata = metadata,
    };

    uint8_t buf[512];
    size_t written = 0;
    int rc = extrittio_telemetry_encode(&t, buf, sizeof(buf), &written);
    assert(rc == 0);
    assert(written > 0);

    extrittio_DeviceTelemetry pb = extrittio_DeviceTelemetry_init_zero;
    metadata_decode_ctx_t metadata_ctx = {0};
    pb.metadata.funcs.decode = decode_metadata;
    pb.metadata.arg = &metadata_ctx;
    pb_istream_t stream = pb_istream_from_buffer(buf, written);
    bool ok = pb_decode(&stream, extrittio_DeviceTelemetry_fields, &pb);
    assert(ok);
    assert(strcmp(pb.device_id, "dev-001") == 0);
    assert(pb.timestamp == 1700000000000LL);
    assert(pb.temperature == 22.5f);
    assert(pb.humidity == 55.0f);
    assert(pb.battery_level == 98.0f);
    assert(pb.latitude > 52.09 && pb.latitude < 52.11);
    assert(pb.longitude > 20.99 && pb.longitude < 21.01);
    assert(metadata_ctx.count == 1);
    assert(strcmp(metadata_ctx.key, "sensor_profile") == 0);
    assert(strcmp(metadata_ctx.value, "environmental") == 0);
}

static void test_encode_buffer_too_small(void) {
    extrittio_telemetry_t t = {
        .device_id = "dev-001",
        .timestamp = 1700000000000LL,
        .temperature = 22.5f,
        .humidity = 55.0f,
        .battery_level = 98.0f,
    };

    uint8_t buf[2];
    size_t written = 0;
    int rc = extrittio_telemetry_encode(&t, buf, sizeof(buf), &written);
    assert(rc == -1);
}

int main(void) {
    test_encode_and_decode();
    test_encode_buffer_too_small();
    printf("All telemetry tests passed.\n");
    return 0;
}
