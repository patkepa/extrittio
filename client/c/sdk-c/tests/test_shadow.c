#include <assert.h>
#include <string.h>
#include <stdio.h>
#include "telemetry.pb.h"
#include <pb_encode.h>
#include <pb_decode.h>

static void test_shadow_report_encode(void) {
    extrittio_ShadowReport pb = extrittio_ShadowReport_init_zero;
    strncpy(pb.device_id, "dev-001", sizeof(pb.device_id) - 1);
    pb.timestamp = 1700000000000LL;
    strncpy(pb.state_json, "{\"temperature\":22.5}", sizeof(pb.state_json) - 1);
    pb.version = 3;

    uint8_t buf[2048];
    pb_ostream_t stream = pb_ostream_from_buffer(buf, sizeof(buf));
    bool ok = pb_encode(&stream, extrittio_ShadowReport_fields, &pb);
    assert(ok);
    assert(stream.bytes_written > 0);

    extrittio_ShadowReport decoded = extrittio_ShadowReport_init_zero;
    pb_istream_t istream = pb_istream_from_buffer(buf, stream.bytes_written);
    ok = pb_decode(&istream, extrittio_ShadowReport_fields, &decoded);
    assert(ok);
    assert(strcmp(decoded.device_id, "dev-001") == 0);
    assert(decoded.timestamp == 1700000000000LL);
    assert(strcmp(decoded.state_json, "{\"temperature\":22.5}") == 0);
    assert(decoded.version == 3);
}

static void test_shadow_get_encode(void) {
    extrittio_ShadowGet pb = extrittio_ShadowGet_init_zero;
    strncpy(pb.device_id, "dev-001", sizeof(pb.device_id) - 1);

    uint8_t buf[128];
    pb_ostream_t stream = pb_ostream_from_buffer(buf, sizeof(buf));
    bool ok = pb_encode(&stream, extrittio_ShadowGet_fields, &pb);
    assert(ok);

    extrittio_ShadowGet decoded = extrittio_ShadowGet_init_zero;
    pb_istream_t istream = pb_istream_from_buffer(buf, stream.bytes_written);
    ok = pb_decode(&istream, extrittio_ShadowGet_fields, &decoded);
    assert(ok);
    assert(strcmp(decoded.device_id, "dev-001") == 0);
}

static void test_shadow_delta_decode(void) {
    extrittio_ShadowDelta pb = extrittio_ShadowDelta_init_zero;
    strncpy(pb.device_id, "dev-001", sizeof(pb.device_id) - 1);
    strncpy(pb.delta_json, "{\"interval\":30}", sizeof(pb.delta_json) - 1);
    pb.version = 5;

    uint8_t buf[2048];
    pb_ostream_t stream = pb_ostream_from_buffer(buf, sizeof(buf));
    bool ok = pb_encode(&stream, extrittio_ShadowDelta_fields, &pb);
    assert(ok);

    extrittio_ShadowDelta decoded = extrittio_ShadowDelta_init_zero;
    pb_istream_t istream = pb_istream_from_buffer(buf, stream.bytes_written);
    ok = pb_decode(&istream, extrittio_ShadowDelta_fields, &decoded);
    assert(ok);
    assert(strcmp(decoded.device_id, "dev-001") == 0);
    assert(strcmp(decoded.delta_json, "{\"interval\":30}") == 0);
    assert(decoded.version == 5);
}

int main(void) {
    test_shadow_report_encode();
    test_shadow_get_encode();
    test_shadow_delta_decode();
    printf("All shadow tests passed.\n");
    return 0;
}
