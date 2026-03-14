#include "extrittio/shadow.h"
#include "extrittio/topics.h"
#include "telemetry.pb.h"
#include <pb_encode.h>
#include <pb_decode.h>
#include <string.h>

int extrittio_shadow_report_publish(z_loaned_session_t *session,
                                     const char *device_id,
                                     int64_t timestamp,
                                     const char *state_json,
                                     int64_t version) {
    extrittio_ShadowReport pb = extrittio_ShadowReport_init_zero;
    strncpy(pb.device_id, device_id, sizeof(pb.device_id) - 1);
    pb.timestamp = timestamp;
    strncpy(pb.state_json, state_json, sizeof(pb.state_json) - 1);
    pb.version = version;

    uint8_t buf[2048];
    pb_ostream_t stream = pb_ostream_from_buffer(buf, sizeof(buf));
    if (!pb_encode(&stream, extrittio_ShadowReport_fields, &pb)) {
        return -1;
    }

    char topic[128];
    if (extrittio_topic_shadow_report(topic, sizeof(topic), device_id) < 0) {
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

int extrittio_shadow_get_publish(z_loaned_session_t *session,
                                  const char *device_id) {
    extrittio_ShadowGet pb = extrittio_ShadowGet_init_zero;
    strncpy(pb.device_id, device_id, sizeof(pb.device_id) - 1);

    uint8_t buf[128];
    pb_ostream_t stream = pb_ostream_from_buffer(buf, sizeof(buf));
    if (!pb_encode(&stream, extrittio_ShadowGet_fields, &pb)) {
        return -1;
    }

    char topic[128];
    if (extrittio_topic_shadow_get(topic, sizeof(topic), device_id) < 0) {
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

typedef struct {
    extrittio_shadow_delta_cb cb;
    void *user_data;
} shadow_delta_ctx_t;

static void shadow_delta_handler(z_loaned_sample_t *sample, void *arg) {
    shadow_delta_ctx_t *ctx = (shadow_delta_ctx_t *)arg;

    z_owned_slice_t slice;
    z_bytes_to_slice(z_sample_payload(sample), &slice);

    extrittio_ShadowDelta pb = extrittio_ShadowDelta_init_zero;
    pb_istream_t stream = pb_istream_from_buffer(z_slice_data(z_loan(slice)), z_slice_len(z_loan(slice)));
    if (!pb_decode(&stream, extrittio_ShadowDelta_fields, &pb)) {
        z_slice_drop(z_slice_move(&slice));
        return;
    }

    ctx->cb(pb.device_id, pb.delta_json, pb.version, ctx->user_data);
    z_slice_drop(z_slice_move(&slice));
}

int extrittio_shadow_delta_subscribe(z_loaned_session_t *session,
                                      const char *device_id,
                                      extrittio_shadow_delta_cb cb,
                                      void *user_data,
                                      z_owned_subscriber_t *sub) {
    char topic[128];
    if (extrittio_topic_shadow_delta(topic, sizeof(topic), device_id) < 0) {
        return -3;
    }

    static shadow_delta_ctx_t ctx;
    ctx.cb = cb;
    ctx.user_data = user_data;

    z_view_keyexpr_t ke;
    z_view_keyexpr_from_str(&ke, topic);

    z_owned_closure_sample_t closure;
    z_closure(&closure, shadow_delta_handler, NULL, &ctx);

    int rc = z_declare_subscriber(session, sub, z_loan(ke), z_move(closure), NULL);
    return rc == 0 ? 0 : -3;
}
