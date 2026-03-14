#include "extrittio/topics.h"
#include <stdio.h>

#define TOPIC_PREFIX "extrittio/devices/"

static int build_topic(char *buf, size_t len, const char *device_id, const char *suffix) {
    int n = snprintf(buf, len, "%s%s/%s", TOPIC_PREFIX, device_id, suffix);
    if (n < 0 || (size_t)n >= len) {
        return -1;
    }
    return n;
}

int extrittio_topic_telemetry(char *buf, size_t len, const char *device_id) {
    return build_topic(buf, len, device_id, "telemetry");
}

int extrittio_topic_heartbeat(char *buf, size_t len, const char *device_id) {
    return build_topic(buf, len, device_id, "heartbeat");
}

int extrittio_topic_shadow_report(char *buf, size_t len, const char *device_id) {
    return build_topic(buf, len, device_id, "shadow/report");
}

int extrittio_topic_shadow_delta(char *buf, size_t len, const char *device_id) {
    return build_topic(buf, len, device_id, "shadow/delta");
}

int extrittio_topic_shadow_get(char *buf, size_t len, const char *device_id) {
    return build_topic(buf, len, device_id, "shadow/get");
}

int extrittio_topic_commands(char *buf, size_t len, const char *device_id) {
    return build_topic(buf, len, device_id, "commands");
}

int extrittio_topic_commands_response(char *buf, size_t len, const char *device_id) {
    return build_topic(buf, len, device_id, "commands/response");
}

int extrittio_topic_logs(char *buf, size_t len, const char *device_id) {
    return build_topic(buf, len, device_id, "logs");
}
