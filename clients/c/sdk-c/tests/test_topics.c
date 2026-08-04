#include <assert.h>
#include <string.h>
#include <stdio.h>
#include "extrittio/topics.h"

static void test_telemetry_topic(void) {
    char buf[128];
    int n = extrittio_topic_telemetry(buf, sizeof(buf), "dev-001");
    assert(n > 0);
    assert(strcmp(buf, "extrittio/devices/dev-001/telemetry") == 0);
}

static void test_heartbeat_topic(void) {
    char buf[128];
    int n = extrittio_topic_heartbeat(buf, sizeof(buf), "dev-001");
    assert(n > 0);
    assert(strcmp(buf, "extrittio/devices/dev-001/heartbeat") == 0);
}

static void test_shadow_report_topic(void) {
    char buf[128];
    int n = extrittio_topic_shadow_report(buf, sizeof(buf), "dev-001");
    assert(n > 0);
    assert(strcmp(buf, "extrittio/devices/dev-001/shadow/report") == 0);
}

static void test_shadow_delta_topic(void) {
    char buf[128];
    int n = extrittio_topic_shadow_delta(buf, sizeof(buf), "dev-001");
    assert(n > 0);
    assert(strcmp(buf, "extrittio/devices/dev-001/shadow/delta") == 0);
}

static void test_shadow_get_topic(void) {
    char buf[128];
    int n = extrittio_topic_shadow_get(buf, sizeof(buf), "dev-001");
    assert(n > 0);
    assert(strcmp(buf, "extrittio/devices/dev-001/shadow/get") == 0);
}

static void test_commands_topic(void) {
    char buf[128];
    int n = extrittio_topic_commands(buf, sizeof(buf), "dev-001");
    assert(n > 0);
    assert(strcmp(buf, "extrittio/devices/dev-001/commands") == 0);
}

static void test_commands_response_topic(void) {
    char buf[128];
    int n = extrittio_topic_commands_response(buf, sizeof(buf), "dev-001");
    assert(n > 0);
    assert(strcmp(buf, "extrittio/devices/dev-001/commands/response") == 0);
}

static void test_logs_topic(void) {
    char buf[128];
    int n = extrittio_topic_logs(buf, sizeof(buf), "dev-001");
    assert(n > 0);
    assert(strcmp(buf, "extrittio/devices/dev-001/logs") == 0);
}

static void test_buffer_too_small(void) {
    char buf[5];
    int n = extrittio_topic_telemetry(buf, sizeof(buf), "dev-001");
    assert(n < 0);
}

int main(void) {
    test_telemetry_topic();
    test_heartbeat_topic();
    test_shadow_report_topic();
    test_shadow_delta_topic();
    test_shadow_get_topic();
    test_commands_topic();
    test_commands_response_topic();
    test_logs_topic();
    test_buffer_too_small();
    printf("All topics tests passed.\n");
    return 0;
}
