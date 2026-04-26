#include "extrittio/commands.h"
#include "extrittio/topics.h"
#include "telemetry.pb.h"
#include <pb_encode.h>
#include <pb_decode.h>
#include <cJSON.h>
#include <string.h>

int extrittio_command_response_publish(z_loaned_session_t *session,
                                       const char *device_id,
                                       const char *correlation_id,
                                       const char *status,
                                       const char *payload,
                                       int64_t timestamp) {
    extrittio_DeviceCommandResponse pb = extrittio_DeviceCommandResponse_init_zero;
    strncpy(pb.correlation_id, correlation_id, sizeof(pb.correlation_id) - 1);
    strncpy(pb.device_id, device_id, sizeof(pb.device_id) - 1);
    strncpy(pb.status, status, sizeof(pb.status) - 1);
    strncpy(pb.payload, payload, sizeof(pb.payload) - 1);
    pb.timestamp = timestamp;

    uint8_t buf[512];
    pb_ostream_t stream = pb_ostream_from_buffer(buf, sizeof(buf));
    if (!pb_encode(&stream, extrittio_DeviceCommandResponse_fields, &pb)) {
        return -1;
    }

    char topic[128];
    if (extrittio_topic_commands_response(topic, sizeof(topic), device_id) < 0) {
        return -1;
    }

    z_view_keyexpr_t ke;
    z_view_keyexpr_from_str(&ke, topic);

    z_owned_bytes_t bytes;
    z_bytes_copy_from_buf(&bytes, buf, stream.bytes_written);

    z_put_options_t opts;
    z_put_options_default(&opts);

    int rc = z_put(session, z_loan(ke), z_move(bytes), &opts);
    return rc == 0 ? 0 : -2;
}

static char *params_to_json(const extrittio_DeviceCommand *pb) {
    cJSON *obj = cJSON_CreateObject();
    if (!obj) return NULL;

    for (pb_size_t i = 0; i < pb->params_count; i++) {
        cJSON_AddStringToObject(obj, pb->params[i].key, pb->params[i].value);
    }

    char *json = cJSON_PrintUnformatted(obj);
    cJSON_Delete(obj);
    return json;
}

typedef struct {
    extrittio_command_cb cb;
    void *user_data;
} command_ctx_t;

static void command_handler(z_loaned_sample_t *sample, void *arg) {
    command_ctx_t *ctx = (command_ctx_t *)arg;

    z_owned_slice_t slice;
    z_bytes_to_slice(z_sample_payload(sample), &slice);
    const z_loaned_slice_t *s = z_slice_loan(&slice);

    extrittio_DeviceCommand pb = extrittio_DeviceCommand_init_zero;
    pb_istream_t stream = pb_istream_from_buffer(z_slice_data(s), z_slice_len(s));
    if (!pb_decode(&stream, extrittio_DeviceCommand_fields, &pb)) {
        z_slice_drop(z_slice_move(&slice));
        return;
    }
    z_slice_drop(z_slice_move(&slice));

    char *params_json = params_to_json(&pb);

    extrittio_command_t cmd = {
        .command = pb.command,
        .params_json = params_json ? params_json : "{}",
        .correlation_id = pb.correlation_id,
    };

    ctx->cb(&cmd, ctx->user_data);

    if (params_json) {
        cJSON_free(params_json);
    }
}

int extrittio_command_subscribe(z_loaned_session_t *session,
                                const char *device_id,
                                extrittio_command_cb cb,
                                void *user_data,
                                z_owned_subscriber_t *sub) {
    char topic[128];
    if (extrittio_topic_commands(topic, sizeof(topic), device_id) < 0) {
        return -3;
    }

    static command_ctx_t ctx;
    ctx.cb = cb;
    ctx.user_data = user_data;

    z_view_keyexpr_t ke;
    z_view_keyexpr_from_str(&ke, topic);

    z_owned_closure_sample_t closure;
    z_closure(&closure, command_handler, NULL, &ctx);

    int rc = z_declare_subscriber(session, sub, z_loan(ke), z_move(closure), NULL);
    return rc == 0 ? 0 : -3;
}
