#include <assert.h>
#include <string.h>
#include <stdio.h>
#include "extrittio/ota.h"

static void test_parse_full_delta(void) {
    const char *delta = "{\"ota\":{\"firmware_version\":\"v2.0.0\","
                        "\"firmware_url\":\"http://example.com/fw.bin\","
                        "\"firmware_update_id\":42,"
                        "\"sha256\":\"abcdef1234567890\"}}";

    extrittio_ota_payload_t out;
    bool ok = extrittio_ota_parse_from_delta(delta, &out);
    assert(ok);
    assert(strcmp(out.firmware_version, "v2.0.0") == 0);
    assert(strcmp(out.firmware_url, "http://example.com/fw.bin") == 0);
    assert(out.firmware_update_id == 42);
    assert(strcmp(out.sha256, "abcdef1234567890") == 0);
}

static void test_parse_no_ota_key(void) {
    const char *delta = "{\"interval\":30}";
    extrittio_ota_payload_t out;
    bool ok = extrittio_ota_parse_from_delta(delta, &out);
    assert(!ok);
}

static void test_parse_missing_required_fields(void) {
    const char *delta = "{\"ota\":{\"firmware_version\":\"v2.0.0\"}}";
    extrittio_ota_payload_t out;
    bool ok = extrittio_ota_parse_from_delta(delta, &out);
    assert(!ok);
}

static void test_parse_optional_fields_absent(void) {
    const char *delta = "{\"ota\":{\"firmware_version\":\"v2.0.0\","
                        "\"firmware_url\":\"http://example.com/fw.bin\"}}";
    extrittio_ota_payload_t out;
    bool ok = extrittio_ota_parse_from_delta(delta, &out);
    assert(ok);
    assert(out.firmware_update_id == 0);
    assert(out.sha256[0] == '\0');
}

static void test_build_status_json(void) {
    char buf[512];
    int n = extrittio_ota_build_status_json(buf, sizeof(buf),
                                             "downloading", "v2.0.0", 42, NULL);
    assert(n > 0);
    assert(strstr(buf, "\"status\":\"downloading\"") != NULL);
    assert(strstr(buf, "\"firmware_version\":\"v2.0.0\"") != NULL);
    assert(strstr(buf, "\"firmware_update_id\":42") != NULL);
    assert(strstr(buf, "\"error\"") == NULL);
}

static void test_build_status_json_with_error(void) {
    char buf[512];
    int n = extrittio_ota_build_status_json(buf, sizeof(buf),
                                             "failed", "v2.0.0", 42, "download timeout");
    assert(n > 0);
    assert(strstr(buf, "\"error\":\"download timeout\"") != NULL);
}

static void test_is_terminal(void) {
    assert(extrittio_ota_is_terminal("success"));
    assert(extrittio_ota_is_terminal("SUCCESS"));
    assert(extrittio_ota_is_terminal("Success"));
    assert(extrittio_ota_is_terminal("failed"));
    assert(extrittio_ota_is_terminal("FAILED"));
    assert(!extrittio_ota_is_terminal("downloading"));
    assert(!extrittio_ota_is_terminal("verifying"));
    assert(!extrittio_ota_is_terminal("installing"));
    assert(!extrittio_ota_is_terminal(""));
}

int main(void) {
    test_parse_full_delta();
    test_parse_no_ota_key();
    test_parse_missing_required_fields();
    test_parse_optional_fields_absent();
    test_build_status_json();
    test_build_status_json_with_error();
    test_is_terminal();
    printf("All OTA tests passed.\n");
    return 0;
}
