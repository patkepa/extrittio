#include "extrittio/sha256.h"

#include <assert.h>
#include <stdint.h>
#include <string.h>

static uint8_t hex_nibble(char value) {
    if (value >= '0' && value <= '9') {
        return (uint8_t)(value - '0');
    }
    return (uint8_t)(value - 'a' + 10);
}

static void decode_hex(const char *hex, uint8_t *bytes, size_t length) {
    for (size_t index = 0; index < length; index++) {
        bytes[index] = (uint8_t)((hex_nibble(hex[index * 2U]) << 4U) |
                                 hex_nibble(hex[index * 2U + 1U]));
    }
}

int main(void) {
    const uint8_t input[] = "abc";
    uint8_t expected[EXTRITTIO_SHA256_DIGEST_SIZE];
    decode_hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
               expected, sizeof(expected));

    extrittio_sha256_t context;
    uint8_t digest[EXTRITTIO_SHA256_DIGEST_SIZE];
    extrittio_sha256_init(&context);
    extrittio_sha256_update(&context, input, 1);
    extrittio_sha256_update(&context, input + 1, 2);
    extrittio_sha256_final(&context, digest);
    assert(extrittio_sha256_equal(digest, expected));

    digest[0] ^= 1U;
    assert(!extrittio_sha256_equal(digest, expected));
    assert(!extrittio_sha256_equal(NULL, expected));
    return 0;
}
