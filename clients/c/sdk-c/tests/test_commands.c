#include <assert.h>
#include <string.h>
#include <stdio.h>
#include "telemetry.pb.h"
#include <pb_encode.h>
#include <pb_decode.h>

static void test_command_response_encode(void) {
    extrittio_DeviceCommandResponse pb = extrittio_DeviceCommandResponse_init_zero;
    strncpy(pb.correlation_id, "cmd-123", sizeof(pb.correlation_id) - 1);
    strncpy(pb.device_id, "dev-001", sizeof(pb.device_id) - 1);
    strncpy(pb.status, "succeeded", sizeof(pb.status) - 1);
    strncpy(pb.payload, "{\"result\":\"ok\"}", sizeof(pb.payload) - 1);
    pb.timestamp = 1700000000000LL;

    uint8_t buf[512];
    pb_ostream_t stream = pb_ostream_from_buffer(buf, sizeof(buf));
    bool ok = pb_encode(&stream, extrittio_DeviceCommandResponse_fields, &pb);
    assert(ok);

    extrittio_DeviceCommandResponse decoded = extrittio_DeviceCommandResponse_init_zero;
    pb_istream_t istream = pb_istream_from_buffer(buf, stream.bytes_written);
    ok = pb_decode(&istream, extrittio_DeviceCommandResponse_fields, &decoded);
    assert(ok);
    assert(strcmp(decoded.correlation_id, "cmd-123") == 0);
    assert(strcmp(decoded.device_id, "dev-001") == 0);
    assert(strcmp(decoded.status, "succeeded") == 0);
    assert(strcmp(decoded.payload, "{\"result\":\"ok\"}") == 0);
    assert(decoded.timestamp == 1700000000000LL);
}

static void test_device_command_decode(void) {
    extrittio_DeviceCommand pb = extrittio_DeviceCommand_init_zero;
    strncpy(pb.command, "restart", sizeof(pb.command) - 1);
    strncpy(pb.correlation_id, "cmd-456", sizeof(pb.correlation_id) - 1);
    pb.params_count = 1;
    strncpy(pb.params[0].key, "delay", sizeof(pb.params[0].key) - 1);
    strncpy(pb.params[0].value, "5", sizeof(pb.params[0].value) - 1);

    uint8_t buf[512];
    pb_ostream_t stream = pb_ostream_from_buffer(buf, sizeof(buf));
    bool ok = pb_encode(&stream, extrittio_DeviceCommand_fields, &pb);
    assert(ok);

    extrittio_DeviceCommand decoded = extrittio_DeviceCommand_init_zero;
    pb_istream_t istream = pb_istream_from_buffer(buf, stream.bytes_written);
    ok = pb_decode(&istream, extrittio_DeviceCommand_fields, &decoded);
    assert(ok);
    assert(strcmp(decoded.command, "restart") == 0);
    assert(strcmp(decoded.correlation_id, "cmd-456") == 0);
    assert(decoded.params_count == 1);
    assert(strcmp(decoded.params[0].key, "delay") == 0);
    assert(strcmp(decoded.params[0].value, "5") == 0);
}

int main(void) {
    test_command_response_encode();
    test_device_command_decode();
    printf("All commands tests passed.\n");
    return 0;
}
