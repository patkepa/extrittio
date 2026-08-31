#include "extrittio/provisioning.h"

#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

typedef struct {
    uint8_t bytes[256];
    uint32_t length;
    bool begun;
    bool committed;
    bool aborted;
} storage_context_t;

static void write_be32(uint8_t *bytes, uint32_t value) {
    bytes[0] = (uint8_t)(value >> 24U);
    bytes[1] = (uint8_t)(value >> 16U);
    bytes[2] = (uint8_t)(value >> 8U);
    bytes[3] = (uint8_t)value;
}

static void header(uint8_t *frame, uint8_t opcode, uint32_t transfer_id) {
    frame[0] = 'E';
    frame[1] = 'X';
    frame[2] = EXTRITTIO_PROVISIONING_PROTOCOL_VERSION;
    frame[3] = opcode;
    write_be32(frame + 4U, transfer_id);
}

static int begin(uint32_t length, void *user_data) {
    storage_context_t *context = user_data;
    context->length = length;
    context->begun = true;
    context->committed = false;
    context->aborted = false;
    memset(context->bytes, 0, sizeof(context->bytes));
    return length <= sizeof(context->bytes) ? 0 : -1;
}

static int write_chunk(uint32_t offset, const uint8_t *data, size_t length,
                       void *user_data) {
    storage_context_t *context = user_data;
    if ((size_t)offset + length > sizeof(context->bytes)) {
        return -1;
    }
    memcpy(context->bytes + offset, data, length);
    return 0;
}

static int commit(uint32_t length, const uint8_t *digest, void *user_data) {
    storage_context_t *context = user_data;
    (void)digest;
    context->committed = length == context->length;
    return context->committed ? 0 : -1;
}

static void abort_transfer(void *user_data) {
    storage_context_t *context = user_data;
    context->aborted = true;
}

static void digest_for(const uint8_t *payload, size_t length, uint8_t *digest) {
    extrittio_sha256_t sha256;
    extrittio_sha256_init(&sha256);
    extrittio_sha256_update(&sha256, payload, length);
    extrittio_sha256_final(&sha256, digest);
}

int main(void) {
    const uint8_t payload[] = "provisioned-device";
    const uint32_t transfer_id = 0x01020304U;
    uint8_t digest[EXTRITTIO_SHA256_DIGEST_SIZE];
    digest_for(payload, sizeof(payload) - 1U, digest);

    uint8_t start[EXTRITTIO_PROVISIONING_START_FRAME_SIZE] = {0};
    header(start, EXTRITTIO_PROVISIONING_START_OPCODE, transfer_id);
    write_be32(start + 8U, sizeof(payload) - 1U);
    memcpy(start + 12U, digest, sizeof(digest));

    uint8_t data_one[17] = {0};
    header(data_one, EXTRITTIO_PROVISIONING_DATA_OPCODE, transfer_id);
    write_be32(data_one + 8U, 0U);
    memcpy(data_one + 12U, payload, 5U);

    uint8_t data_two[12U + sizeof(payload) - 1U - 5U];
    memset(data_two, 0, sizeof(data_two));
    header(data_two, EXTRITTIO_PROVISIONING_DATA_OPCODE, transfer_id);
    write_be32(data_two + 8U, 5U);
    memcpy(data_two + 12U, payload + 5U, sizeof(payload) - 1U - 5U);

    uint8_t commit_frame[EXTRITTIO_PROVISIONING_COMMIT_FRAME_SIZE] = {0};
    header(commit_frame, EXTRITTIO_PROVISIONING_COMMIT_OPCODE, transfer_id);
    memcpy(commit_frame + 8U, digest, sizeof(digest));

    extrittio_provisioning_storage_t storage = {
        .begin = begin,
        .write = write_chunk,
        .commit = commit,
        .abort = abort_transfer,
    };
    storage_context_t context = {0};
    extrittio_provisioning_transfer_t transfer;
    extrittio_provisioning_transfer_init(&transfer, sizeof(context.bytes));

    assert(extrittio_provisioning_transfer_handle(
               &transfer, start, sizeof(start), &storage, &context) ==
           EXTRITTIO_PROVISIONING_OK);
    assert(extrittio_provisioning_transfer_handle(
               &transfer, data_one, sizeof(data_one), &storage, &context) ==
           EXTRITTIO_PROVISIONING_OK);
    assert(extrittio_provisioning_transfer_handle(
               &transfer, data_two, sizeof(data_two), &storage, &context) ==
           EXTRITTIO_PROVISIONING_OK);
    assert(extrittio_provisioning_transfer_handle(
               &transfer, commit_frame, sizeof(commit_frame), &storage, &context) ==
           EXTRITTIO_PROVISIONING_OK);
    assert(context.committed);
    assert(memcmp(context.bytes, payload, sizeof(payload) - 1U) == 0);
    assert(!transfer.active);

    context = (storage_context_t){0};
    assert(extrittio_provisioning_transfer_handle(
               &transfer, start, sizeof(start), &storage, &context) ==
           EXTRITTIO_PROVISIONING_OK);
    write_be32(data_one + 8U, 1U);
    assert(extrittio_provisioning_transfer_handle(
               &transfer, data_one, sizeof(data_one), &storage, &context) ==
           EXTRITTIO_PROVISIONING_ERR_OFFSET);
    assert(context.aborted);
    assert(!transfer.active);

    extrittio_provisioning_transfer_init(&transfer, 4U);
    assert(extrittio_provisioning_transfer_handle(
               &transfer, start, sizeof(start), &storage, &context) ==
           EXTRITTIO_PROVISIONING_ERR_LENGTH);
    return 0;
}
