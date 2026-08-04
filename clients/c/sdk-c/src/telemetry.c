#include "extrittio/telemetry.h"
#include "extrittio/topics.h"
#include "telemetry.pb.h"
#include <pb_encode.h>
#include <stdlib.h>
#include <string.h>

#define EXTRITTIO_TELEMETRY_PUBLISH_BUF_SIZE 8192

typedef struct {
    size_t count;
    const extrittio_metadata_entry_t *entries;
} metadata_encode_ctx_t;

static bool encode_string(pb_ostream_t *stream, const pb_field_t *field, void *const *arg) {
    const char *value = (const char *)(*arg);
    if (value == NULL) {
        value = "";
    }

    if (!pb_encode_tag_for_field(stream, field)) {
        return false;
    }
    return pb_encode_string(stream, (const uint8_t *)value, strlen(value));
}

static bool encode_metadata(pb_ostream_t *stream, const pb_field_t *field, void *const *arg) {
    const metadata_encode_ctx_t *ctx = (const metadata_encode_ctx_t *)(*arg);
    if (ctx == NULL || ctx->entries == NULL) {
        return true;
    }

    size_t count = ctx->count;
    if (count > EXTRITTIO_TELEMETRY_METADATA_MAX) {
        count = EXTRITTIO_TELEMETRY_METADATA_MAX;
    }

    for (size_t i = 0; i < count; i++) {
        extrittio_DeviceTelemetry_MetadataEntry entry =
            extrittio_DeviceTelemetry_MetadataEntry_init_zero;
        entry.key.funcs.encode = encode_string;
        entry.key.arg = (void *)ctx->entries[i].key;
        entry.value.funcs.encode = encode_string;
        entry.value.arg = (void *)ctx->entries[i].value;

        if (!pb_encode_tag_for_field(stream, field)) {
            return false;
        }
        if (!pb_encode_submessage(stream, extrittio_DeviceTelemetry_MetadataEntry_fields, &entry)) {
            return false;
        }
    }

    return true;
}

int extrittio_telemetry_encode(const extrittio_telemetry_t *t,
                                uint8_t *buf, size_t len, size_t *written) {
    extrittio_DeviceTelemetry pb = extrittio_DeviceTelemetry_init_zero;
    strncpy(pb.device_id, t->device_id, sizeof(pb.device_id) - 1);
    pb.timestamp = t->timestamp;
    pb.temperature = t->temperature;
    pb.humidity = t->humidity;
    pb.battery_level = t->battery_level;
    pb.latitude = t->latitude;
    pb.longitude = t->longitude;
    pb.speed = t->speed;
    pb.altitude = t->altitude;
    pb.heading = t->heading;

    metadata_encode_ctx_t metadata_ctx = {
        .count = t->metadata_count,
        .entries = t->metadata,
    };
    if (t->metadata_count > 0 && t->metadata != NULL) {
        pb.metadata.funcs.encode = encode_metadata;
        pb.metadata.arg = &metadata_ctx;
    }

    pb_ostream_t stream = pb_ostream_from_buffer(buf, len);
    if (!pb_encode(&stream, extrittio_DeviceTelemetry_fields, &pb)) {
        return -1;
    }
    *written = stream.bytes_written;
    return 0;
}

int extrittio_telemetry_publish(z_loaned_session_t *session,
                                 const extrittio_telemetry_t *t) {
    uint8_t *buf = malloc(EXTRITTIO_TELEMETRY_PUBLISH_BUF_SIZE);
    if (buf == NULL) {
        return -1;
    }

    size_t written = 0;
    if (extrittio_telemetry_encode(t, buf, EXTRITTIO_TELEMETRY_PUBLISH_BUF_SIZE, &written) != 0) {
        free(buf);
        return -1;
    }

    char topic[128];
    if (extrittio_topic_telemetry(topic, sizeof(topic), t->device_id) < 0) {
        free(buf);
        return -1;
    }

    z_view_keyexpr_t ke;
    z_view_keyexpr_from_str(&ke, topic);

    z_owned_bytes_t payload;
    z_bytes_copy_from_buf(&payload, buf, written);

    z_put_options_t opts;
    z_put_options_default(&opts);

    int rc = z_put(session, z_loan(ke), z_move(payload), &opts);
    free(buf);
    return rc == 0 ? 0 : -2;
}
