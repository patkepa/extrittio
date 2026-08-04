#include "extrittio/logs.h"
#include "extrittio/topics.h"
#include "telemetry.pb.h"
#include <pb_encode.h>
#include <string.h>

int extrittio_log_publish(z_loaned_session_t *session,
                           const char *device_id,
                           int64_t timestamp,
                           const char *level,
                           const char *message) {
    extrittio_DeviceLog pb = extrittio_DeviceLog_init_zero;
    strncpy(pb.device_id, device_id, sizeof(pb.device_id) - 1);
    pb.timestamp = timestamp;
    strncpy(pb.level, level, sizeof(pb.level) - 1);
    strncpy(pb.message, message, sizeof(pb.message) - 1);

    uint8_t buf[512];
    pb_ostream_t stream = pb_ostream_from_buffer(buf, sizeof(buf));
    if (!pb_encode(&stream, extrittio_DeviceLog_fields, &pb)) {
        return -1;
    }

    char topic[128];
    if (extrittio_topic_logs(topic, sizeof(topic), device_id) < 0) {
        return -1;
    }

    z_view_keyexpr_t ke;
    z_view_keyexpr_from_str(&ke, topic);

    z_owned_bytes_t payload;
    z_bytes_copy_from_buf(&payload, buf, stream.bytes_written);

    z_put_options_t opts;
    z_put_options_default(&opts);

    int rc = z_put(session, z_loan(ke), z_move(payload), &opts);
    return rc == 0 ? 0 : -2;
}
