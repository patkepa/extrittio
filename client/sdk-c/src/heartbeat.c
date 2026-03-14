#include "extrittio/heartbeat.h"
#include "extrittio/topics.h"
#include "telemetry.pb.h"
#include <pb_encode.h>
#include <string.h>

int extrittio_heartbeat_encode(const extrittio_heartbeat_t *h,
                                uint8_t *buf, size_t len, size_t *written) {
    extrittio_DeviceHeartbeat pb = extrittio_DeviceHeartbeat_init_zero;
    strncpy(pb.device_id, h->device_id, sizeof(pb.device_id) - 1);
    pb.timestamp = h->timestamp;
    strncpy(pb.status, h->status, sizeof(pb.status) - 1);
    strncpy(pb.firmware, h->firmware, sizeof(pb.firmware) - 1);
    pb.uptime_seconds = h->uptime_seconds;

    pb_ostream_t stream = pb_ostream_from_buffer(buf, len);
    if (!pb_encode(&stream, extrittio_DeviceHeartbeat_fields, &pb)) {
        return -1;
    }
    *written = stream.bytes_written;
    return 0;
}

int extrittio_heartbeat_publish(z_loaned_session_t *session,
                                 const extrittio_heartbeat_t *h) {
    uint8_t buf[512];
    size_t written = 0;
    if (extrittio_heartbeat_encode(h, buf, sizeof(buf), &written) != 0) {
        return -1;
    }

    char topic[128];
    if (extrittio_topic_heartbeat(topic, sizeof(topic), h->device_id) < 0) {
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
