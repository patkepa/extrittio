#include "extrittio/telemetry.h"
#include "extrittio/topics.h"
#include "telemetry.pb.h"
#include <pb_encode.h>
#include <string.h>

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

    pb_ostream_t stream = pb_ostream_from_buffer(buf, len);
    if (!pb_encode(&stream, extrittio_DeviceTelemetry_fields, &pb)) {
        return -1;
    }
    *written = stream.bytes_written;
    return 0;
}

int extrittio_telemetry_publish(z_loaned_session_t *session,
                                 const extrittio_telemetry_t *t) {
    uint8_t buf[512];
    size_t written = 0;
    if (extrittio_telemetry_encode(t, buf, sizeof(buf), &written) != 0) {
        return -1;
    }

    char topic[128];
    if (extrittio_topic_telemetry(topic, sizeof(topic), t->device_id) < 0) {
        return -1;
    }

    z_view_keyexpr_t ke;
    z_view_keyexpr_from_str(&ke, topic);

    z_owned_bytes_t payload;
    z_bytes_copy_from_buf(&payload, buf, written);

    z_put_options_t opts;
    z_put_options_default(&opts);

    int rc = z_put(session, z_loan(ke), z_move(payload), &opts);
    return rc == 0 ? 0 : -2;
}
