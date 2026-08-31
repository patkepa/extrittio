#include "extrittio/provisioning.h"

#include <string.h>

static uint32_t read_be32(const uint8_t *bytes) {
    return ((uint32_t)bytes[0] << 24U) | ((uint32_t)bytes[1] << 16U) |
           ((uint32_t)bytes[2] << 8U) | (uint32_t)bytes[3];
}

static void clear_transfer(extrittio_provisioning_transfer_t *transfer) {
    uint32_t maximum = transfer->maximum_payload_length;
    memset(transfer, 0, sizeof(*transfer));
    transfer->maximum_payload_length = maximum;
}

static void fail_transfer(extrittio_provisioning_transfer_t *transfer,
                          const extrittio_provisioning_storage_t *storage,
                          void *user_data) {
    if (transfer->active && storage->abort != NULL) {
        storage->abort(user_data);
    }
    clear_transfer(transfer);
}

void extrittio_provisioning_transfer_init(
    extrittio_provisioning_transfer_t *transfer,
    uint32_t maximum_payload_length) {
    if (transfer == NULL) {
        return;
    }
    memset(transfer, 0, sizeof(*transfer));
    transfer->maximum_payload_length = maximum_payload_length;
}

void extrittio_provisioning_transfer_abort(
    extrittio_provisioning_transfer_t *transfer,
    const extrittio_provisioning_storage_t *storage,
    void *user_data) {
    if (transfer == NULL || storage == NULL) {
        return;
    }
    fail_transfer(transfer, storage, user_data);
}

static extrittio_provisioning_result_t handle_start(
    extrittio_provisioning_transfer_t *transfer,
    const uint8_t *frame,
    size_t frame_length,
    const extrittio_provisioning_storage_t *storage,
    void *user_data) {
    if (frame_length != EXTRITTIO_PROVISIONING_START_FRAME_SIZE) {
        return EXTRITTIO_PROVISIONING_ERR_FRAME;
    }
    uint32_t payload_length = read_be32(frame + EXTRITTIO_PROVISIONING_HEADER_SIZE);
    if (payload_length == 0U || payload_length > transfer->maximum_payload_length) {
        return EXTRITTIO_PROVISIONING_ERR_LENGTH;
    }

    if (transfer->active) {
        fail_transfer(transfer, storage, user_data);
    }
    if (storage->begin == NULL || storage->write == NULL || storage->commit == NULL ||
        storage->begin(payload_length, user_data) != 0) {
        clear_transfer(transfer);
        return EXTRITTIO_PROVISIONING_ERR_STORAGE;
    }

    transfer->active = true;
    transfer->transfer_id = read_be32(frame + 4U);
    transfer->payload_length = payload_length;
    transfer->next_offset = 0U;
    memcpy(transfer->expected_digest, frame + 12U,
           EXTRITTIO_SHA256_DIGEST_SIZE);
    extrittio_sha256_init(&transfer->sha256);
    return EXTRITTIO_PROVISIONING_OK;
}

static extrittio_provisioning_result_t handle_data(
    extrittio_provisioning_transfer_t *transfer,
    const uint8_t *frame,
    size_t frame_length,
    const extrittio_provisioning_storage_t *storage,
    void *user_data) {
    if (!transfer->active) {
        return EXTRITTIO_PROVISIONING_ERR_STATE;
    }
    if (frame_length <= 12U) {
        fail_transfer(transfer, storage, user_data);
        return EXTRITTIO_PROVISIONING_ERR_FRAME;
    }
    uint32_t offset = read_be32(frame + EXTRITTIO_PROVISIONING_HEADER_SIZE);
    size_t data_length = frame_length - 12U;
    if (offset != transfer->next_offset) {
        fail_transfer(transfer, storage, user_data);
        return EXTRITTIO_PROVISIONING_ERR_OFFSET;
    }
    if (data_length > (size_t)(transfer->payload_length - transfer->next_offset)) {
        fail_transfer(transfer, storage, user_data);
        return EXTRITTIO_PROVISIONING_ERR_LENGTH;
    }
    if (storage->write(offset, frame + 12U, data_length, user_data) != 0) {
        fail_transfer(transfer, storage, user_data);
        return EXTRITTIO_PROVISIONING_ERR_STORAGE;
    }
    extrittio_sha256_update(&transfer->sha256, frame + 12U, data_length);
    transfer->next_offset += (uint32_t)data_length;
    return EXTRITTIO_PROVISIONING_OK;
}

static extrittio_provisioning_result_t handle_commit(
    extrittio_provisioning_transfer_t *transfer,
    const uint8_t *frame,
    size_t frame_length,
    const extrittio_provisioning_storage_t *storage,
    void *user_data) {
    if (!transfer->active) {
        return EXTRITTIO_PROVISIONING_ERR_STATE;
    }
    if (frame_length != EXTRITTIO_PROVISIONING_COMMIT_FRAME_SIZE) {
        fail_transfer(transfer, storage, user_data);
        return EXTRITTIO_PROVISIONING_ERR_FRAME;
    }
    if (transfer->next_offset != transfer->payload_length) {
        fail_transfer(transfer, storage, user_data);
        return EXTRITTIO_PROVISIONING_ERR_LENGTH;
    }
    const uint8_t *commit_digest = frame + EXTRITTIO_PROVISIONING_HEADER_SIZE;
    if (!extrittio_sha256_equal(commit_digest, transfer->expected_digest)) {
        fail_transfer(transfer, storage, user_data);
        return EXTRITTIO_PROVISIONING_ERR_DIGEST;
    }
    uint8_t calculated_digest[EXTRITTIO_SHA256_DIGEST_SIZE];
    extrittio_sha256_final(&transfer->sha256, calculated_digest);
    if (!extrittio_sha256_equal(calculated_digest, transfer->expected_digest)) {
        memset(calculated_digest, 0, sizeof(calculated_digest));
        fail_transfer(transfer, storage, user_data);
        return EXTRITTIO_PROVISIONING_ERR_DIGEST;
    }
    uint32_t payload_length = transfer->payload_length;
    if (storage->commit(payload_length, calculated_digest, user_data) != 0) {
        memset(calculated_digest, 0, sizeof(calculated_digest));
        fail_transfer(transfer, storage, user_data);
        return EXTRITTIO_PROVISIONING_ERR_STORAGE;
    }
    memset(calculated_digest, 0, sizeof(calculated_digest));
    clear_transfer(transfer);
    return EXTRITTIO_PROVISIONING_OK;
}

extrittio_provisioning_result_t extrittio_provisioning_transfer_handle(
    extrittio_provisioning_transfer_t *transfer,
    const uint8_t *frame,
    size_t frame_length,
    const extrittio_provisioning_storage_t *storage,
    void *user_data) {
    if (transfer == NULL || frame == NULL || storage == NULL || frame_length < 8U ||
        transfer->maximum_payload_length == 0U) {
        return EXTRITTIO_PROVISIONING_ERR_ARGUMENT;
    }
    if (frame[0] != (uint8_t)'E' || frame[1] != (uint8_t)'X') {
        return EXTRITTIO_PROVISIONING_ERR_FRAME;
    }
    if (frame[2] != EXTRITTIO_PROVISIONING_PROTOCOL_VERSION) {
        return EXTRITTIO_PROVISIONING_ERR_VERSION;
    }

    uint8_t opcode = frame[3];
    uint32_t transfer_id = read_be32(frame + 4U);
    if (opcode != EXTRITTIO_PROVISIONING_START_OPCODE) {
        if (!transfer->active) {
            return EXTRITTIO_PROVISIONING_ERR_STATE;
        }
        if (transfer_id != transfer->transfer_id) {
            fail_transfer(transfer, storage, user_data);
            return EXTRITTIO_PROVISIONING_ERR_TRANSFER_ID;
        }
    }

    switch (opcode) {
    case EXTRITTIO_PROVISIONING_START_OPCODE:
        return handle_start(transfer, frame, frame_length, storage, user_data);
    case EXTRITTIO_PROVISIONING_DATA_OPCODE:
        return handle_data(transfer, frame, frame_length, storage, user_data);
    case EXTRITTIO_PROVISIONING_COMMIT_OPCODE:
        return handle_commit(transfer, frame, frame_length, storage, user_data);
    default:
        return EXTRITTIO_PROVISIONING_ERR_OPCODE;
    }
}
