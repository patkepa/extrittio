#ifndef EXTRITTIO_SHA256_H
#define EXTRITTIO_SHA256_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define EXTRITTIO_SHA256_DIGEST_SIZE 32

typedef struct {
    uint32_t state[8];
    uint64_t total_bytes;
    uint8_t block[64];
    size_t block_length;
} extrittio_sha256_t;

void extrittio_sha256_init(extrittio_sha256_t *context);
void extrittio_sha256_update(extrittio_sha256_t *context,
                             const uint8_t *data,
                             size_t length);
void extrittio_sha256_final(extrittio_sha256_t *context,
                            uint8_t digest[EXTRITTIO_SHA256_DIGEST_SIZE]);
bool extrittio_sha256_equal(
    const uint8_t left[EXTRITTIO_SHA256_DIGEST_SIZE],
    const uint8_t right[EXTRITTIO_SHA256_DIGEST_SIZE]);

#ifdef __cplusplus
}
#endif

#endif /* EXTRITTIO_SHA256_H */
