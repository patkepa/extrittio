#ifndef EXTRITTIO_PROVISIONING_H
#define EXTRITTIO_PROVISIONING_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "extrittio/sha256.h"

#ifdef __cplusplus
extern "C" {
#endif

#define EXTRITTIO_PROVISIONING_PROTOCOL_VERSION 1
#define EXTRITTIO_PROVISIONING_HEADER_SIZE 8
#define EXTRITTIO_PROVISIONING_START_OPCODE 0x01
#define EXTRITTIO_PROVISIONING_DATA_OPCODE 0x02
#define EXTRITTIO_PROVISIONING_COMMIT_OPCODE 0x03
#define EXTRITTIO_PROVISIONING_START_FRAME_SIZE 44
#define EXTRITTIO_PROVISIONING_COMMIT_FRAME_SIZE 40

typedef enum {
    EXTRITTIO_PROVISIONING_OK = 0,
    EXTRITTIO_PROVISIONING_ERR_ARGUMENT = -1,
    EXTRITTIO_PROVISIONING_ERR_FRAME = -2,
    EXTRITTIO_PROVISIONING_ERR_VERSION = -3,
    EXTRITTIO_PROVISIONING_ERR_OPCODE = -4,
    EXTRITTIO_PROVISIONING_ERR_STATE = -5,
    EXTRITTIO_PROVISIONING_ERR_TRANSFER_ID = -6,
    EXTRITTIO_PROVISIONING_ERR_LENGTH = -7,
    EXTRITTIO_PROVISIONING_ERR_OFFSET = -8,
    EXTRITTIO_PROVISIONING_ERR_DIGEST = -9,
    EXTRITTIO_PROVISIONING_ERR_STORAGE = -10,
} extrittio_provisioning_result_t;

typedef int (*extrittio_provisioning_begin_cb)(uint32_t payload_length,
                                               void *user_data);
typedef int (*extrittio_provisioning_write_cb)(uint32_t offset,
                                               const uint8_t *data,
                                               size_t length,
                                               void *user_data);
typedef int (*extrittio_provisioning_commit_cb)(
    uint32_t payload_length,
    const uint8_t digest[EXTRITTIO_SHA256_DIGEST_SIZE],
    void *user_data);
typedef void (*extrittio_provisioning_abort_cb)(void *user_data);

typedef struct {
    extrittio_provisioning_begin_cb begin;
    extrittio_provisioning_write_cb write;
    extrittio_provisioning_commit_cb commit;
    extrittio_provisioning_abort_cb abort;
} extrittio_provisioning_storage_t;

typedef struct {
    uint32_t maximum_payload_length;
    uint32_t transfer_id;
    uint32_t payload_length;
    uint32_t next_offset;
    uint8_t expected_digest[EXTRITTIO_SHA256_DIGEST_SIZE];
    extrittio_sha256_t sha256;
    bool active;
} extrittio_provisioning_transfer_t;

void extrittio_provisioning_transfer_init(
    extrittio_provisioning_transfer_t *transfer,
    uint32_t maximum_payload_length);

void extrittio_provisioning_transfer_abort(
    extrittio_provisioning_transfer_t *transfer,
    const extrittio_provisioning_storage_t *storage,
    void *user_data);

extrittio_provisioning_result_t extrittio_provisioning_transfer_handle(
    extrittio_provisioning_transfer_t *transfer,
    const uint8_t *frame,
    size_t frame_length,
    const extrittio_provisioning_storage_t *storage,
    void *user_data);

#ifdef __cplusplus
}
#endif

#endif /* EXTRITTIO_PROVISIONING_H */
