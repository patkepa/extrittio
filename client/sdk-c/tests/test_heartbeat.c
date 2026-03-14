#include <assert.h>
#include <string.h>
#include <stdio.h>
#include "extrittio/heartbeat.h"
#include "telemetry.pb.h"
#include <pb_decode.h>

static void test_encode_and_decode(void) {
    extrittio_heartbeat_t h = {
        .device_id = "dev-001",
        .timestamp = 1700000000000LL,
        .status = "online",
        .firmware = "v1.0.0",
        .uptime_seconds = 3600,
    };

    uint8_t buf[512];
    size_t written = 0;
    int rc = extrittio_heartbeat_encode(&h, buf, sizeof(buf), &written);
    assert(rc == 0);
    assert(written > 0);

    extrittio_DeviceHeartbeat pb = extrittio_DeviceHeartbeat_init_zero;
    pb_istream_t stream = pb_istream_from_buffer(buf, written);
    bool ok = pb_decode(&stream, extrittio_DeviceHeartbeat_fields, &pb);
    assert(ok);
    assert(strcmp(pb.device_id, "dev-001") == 0);
    assert(pb.timestamp == 1700000000000LL);
    assert(strcmp(pb.status, "online") == 0);
    assert(strcmp(pb.firmware, "v1.0.0") == 0);
    assert(pb.uptime_seconds == 3600);
}

int main(void) {
    test_encode_and_decode();
    printf("All heartbeat tests passed.\n");
    return 0;
}
