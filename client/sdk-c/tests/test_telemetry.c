#include <assert.h>
#include <string.h>
#include <stdio.h>
#include "extrittio/telemetry.h"
#include "telemetry.pb.h"
#include <pb_decode.h>

static void test_encode_and_decode(void) {
    extrittio_telemetry_t t = {
        .device_id = "dev-001",
        .timestamp = 1700000000000LL,
        .temperature = 22.5f,
        .humidity = 55.0f,
        .battery_level = 98.0f,
    };

    uint8_t buf[512];
    size_t written = 0;
    int rc = extrittio_telemetry_encode(&t, buf, sizeof(buf), &written);
    assert(rc == 0);
    assert(written > 0);

    extrittio_DeviceTelemetry pb = extrittio_DeviceTelemetry_init_zero;
    pb_istream_t stream = pb_istream_from_buffer(buf, written);
    bool ok = pb_decode(&stream, extrittio_DeviceTelemetry_fields, &pb);
    assert(ok);
    assert(strcmp(pb.device_id, "dev-001") == 0);
    assert(pb.timestamp == 1700000000000LL);
    assert(pb.temperature == 22.5f);
    assert(pb.humidity == 55.0f);
    assert(pb.battery_level == 98.0f);
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
