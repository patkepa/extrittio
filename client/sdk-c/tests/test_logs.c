#include <assert.h>
#include <string.h>
#include <stdio.h>
#include "telemetry.pb.h"
#include <pb_encode.h>
#include <pb_decode.h>

static void test_device_log_encode(void) {
    extrittio_DeviceLog pb = extrittio_DeviceLog_init_zero;
    strncpy(pb.device_id, "dev-001", sizeof(pb.device_id) - 1);
    pb.timestamp = 1700000000000LL;
    strncpy(pb.level, "INFO", sizeof(pb.level) - 1);
    strncpy(pb.message, "Sensor initialized", sizeof(pb.message) - 1);

    uint8_t buf[512];
    pb_ostream_t stream = pb_ostream_from_buffer(buf, sizeof(buf));
    bool ok = pb_encode(&stream, extrittio_DeviceLog_fields, &pb);
    assert(ok);

    extrittio_DeviceLog decoded = extrittio_DeviceLog_init_zero;
    pb_istream_t istream = pb_istream_from_buffer(buf, stream.bytes_written);
    ok = pb_decode(&istream, extrittio_DeviceLog_fields, &decoded);
    assert(ok);
    assert(strcmp(decoded.device_id, "dev-001") == 0);
    assert(strcmp(decoded.level, "INFO") == 0);
    assert(strcmp(decoded.message, "Sensor initialized") == 0);
}

int main(void) {
    test_device_log_encode();
    printf("All logs tests passed.\n");
    return 0;
}
