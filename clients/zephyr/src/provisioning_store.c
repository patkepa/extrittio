#include "extrittio_zephyr/provisioning_store.h"

#include <errno.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include <mbedtls/pk.h>
#include <mbedtls/version.h>
#include <mbedtls/x509_crt.h>
#include <zephyr/drivers/flash.h>
#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>
#include <zephyr/random/random.h>
#include <zephyr/settings/settings.h>
#include <zephyr/storage/flash_map.h>

#include "extrittio_zephyr/identity.h"

LOG_MODULE_REGISTER(extrittio_store, CONFIG_LOG_DEFAULT_LEVEL);

#define BOOTSTRAP_MAGIC 0x58425445U
#define BOOTSTRAP_HEADER_VERSION 1U
#define BOOTSTRAP_DATA_OFFSET 64U
#define BOOTSTRAP_SLOT_COUNT 2U
#define WRITE_BUFFER_MAX 32U

struct bootstrap_header {
    uint32_t magic;
    uint16_t version;
    uint16_t reserved;
    uint32_t generation;
    uint32_t payload_length;
    uint8_t digest[EXTRITTIO_SHA256_DIGEST_SIZE];
};

struct staging_state {
    const struct flash_area *area;
    uint8_t slot;
    uint32_t payload_length;
    uint32_t accepted_length;
    uint32_t persisted_length;
    size_t write_block;
    size_t pending_length;
    uint8_t pending[WRITE_BUFFER_MAX];
    bool active;
};

static const struct flash_area *slots[BOOTSTRAP_SLOT_COUNT];
static struct staging_state staging;
static uint8_t preferred_slot = UINT8_MAX;

static int settings_set(const char *name, size_t length,
                        settings_read_cb read_cb, void *callback_argument) {
    if (strcmp(name, "active") != 0 || length != sizeof(preferred_slot)) {
        return -ENOENT;
    }
    ssize_t result = read_cb(callback_argument, &preferred_slot,
                             sizeof(preferred_slot));
    return result == sizeof(preferred_slot) ? 0 : -EIO;
}

SETTINGS_STATIC_HANDLER_DEFINE(extrittio_bootstrap, "extrittio/bootstrap",
                               NULL, settings_set, NULL, NULL);

static bool slot_header_valid(uint8_t slot, struct bootstrap_header *header) {
    if (slot >= BOOTSTRAP_SLOT_COUNT || slots[slot] == NULL ||
        flash_area_read(slots[slot], 0, header, sizeof(*header)) != 0) {
        return false;
    }
    if (header->magic != BOOTSTRAP_MAGIC ||
        header->version != BOOTSTRAP_HEADER_VERSION ||
        header->payload_length == 0U ||
        header->payload_length > CONFIG_EXTRITTIO_BOOTSTRAP_BUFFER_SIZE ||
        header->payload_length > slots[slot]->fa_size - BOOTSTRAP_DATA_OFFSET) {
        return false;
    }

    extrittio_sha256_t sha256;
    extrittio_sha256_init(&sha256);
    uint8_t buffer[256];
    uint32_t offset = 0;
    while (offset < header->payload_length) {
        size_t length = MIN(sizeof(buffer), header->payload_length - offset);
        if (flash_area_read(slots[slot], BOOTSTRAP_DATA_OFFSET + offset,
                            buffer, length) != 0) {
            return false;
        }
        extrittio_sha256_update(&sha256, buffer, length);
        offset += (uint32_t)length;
    }
    uint8_t digest[EXTRITTIO_SHA256_DIGEST_SIZE];
    extrittio_sha256_final(&sha256, digest);
    return extrittio_sha256_equal(digest, header->digest);
}

static int select_active_slot(uint8_t *selected,
                              struct bootstrap_header *selected_header) {
    struct bootstrap_header headers[BOOTSTRAP_SLOT_COUNT];
    bool valid[BOOTSTRAP_SLOT_COUNT];
    for (uint8_t slot = 0; slot < BOOTSTRAP_SLOT_COUNT; slot++) {
        valid[slot] = slot_header_valid(slot, &headers[slot]);
    }

    if (preferred_slot < BOOTSTRAP_SLOT_COUNT && valid[preferred_slot]) {
        *selected = preferred_slot;
        *selected_header = headers[preferred_slot];
        return 0;
    }
    if (!valid[0] && !valid[1]) {
        return -ENOENT;
    }
    *selected = valid[0] && (!valid[1] ||
                            headers[0].generation >= headers[1].generation)
                    ? 0U
                    : 1U;
    *selected_header = headers[*selected];
    return 0;
}

static int random_bytes(void *context, unsigned char *output, size_t length) {
    ARG_UNUSED(context);
    return sys_csrand_get(output, length);
}

static int validate_credentials(const extrittio_bootstrap_t *bootstrap) {
    int result;
    mbedtls_x509_crt device_certificate;
    mbedtls_x509_crt ca_certificate;
    mbedtls_pk_context private_key;
    mbedtls_x509_crt_init(&device_certificate);
    mbedtls_x509_crt_init(&ca_certificate);
    mbedtls_pk_init(&private_key);

    result = mbedtls_x509_crt_parse(
        &device_certificate,
        (const unsigned char *)bootstrap->credentials.certificate_pem,
        strlen(bootstrap->credentials.certificate_pem) + 1U);
    if (result == 0) {
        result = mbedtls_x509_crt_parse(
            &ca_certificate,
            (const unsigned char *)bootstrap->credentials.ca_pem,
            strlen(bootstrap->credentials.ca_pem) + 1U);
    }
    if (result == 0) {
#if MBEDTLS_VERSION_MAJOR >= 3
        result = mbedtls_pk_parse_key(
            &private_key,
            (const unsigned char *)bootstrap->credentials.private_key_pem,
            strlen(bootstrap->credentials.private_key_pem) + 1U,
            NULL, 0, random_bytes, NULL);
#else
        result = mbedtls_pk_parse_key(
            &private_key,
            (const unsigned char *)bootstrap->credentials.private_key_pem,
            strlen(bootstrap->credentials.private_key_pem) + 1U,
            NULL, 0);
#endif
    }
    if (result == 0) {
        result = mbedtls_pk_check_pair(&device_certificate.pk, &private_key);
    }

    mbedtls_pk_free(&private_key);
    mbedtls_x509_crt_free(&ca_certificate);
    mbedtls_x509_crt_free(&device_certificate);
    return result == 0 ? 0 : -EINVAL;
}

static int read_and_validate(const struct flash_area *area, uint32_t length,
                             extrittio_bootstrap_t *bootstrap) {
    char *json = k_malloc((size_t)length + 1U);
    if (json == NULL) {
        return -ENOMEM;
    }
    int result = flash_area_read(area, BOOTSTRAP_DATA_OFFSET, json, length);
    if (result == 0) {
        json[length] = '\0';
        result = extrittio_bootstrap_parse(json, length, bootstrap);
    }
    memset(json, 0, (size_t)length + 1U);
    k_free(json);
    if (result != EXTRITTIO_BOOTSTRAP_OK) {
        return -EINVAL;
    }
    if (extrittio_bootstrap_validate_factory(
            bootstrap, extrittio_factory_device_id()) !=
        EXTRITTIO_BOOTSTRAP_OK) {
        extrittio_bootstrap_clear(bootstrap);
        return -EPERM;
    }
    result = validate_credentials(bootstrap);
    if (result != 0) {
        extrittio_bootstrap_clear(bootstrap);
    }
    return result;
}

static int staging_flush(void) {
    if (staging.pending_length == 0U) {
        return 0;
    }
    size_t useful_length = staging.pending_length;
    memset(staging.pending + staging.pending_length, 0xff,
           staging.write_block - staging.pending_length);
    int result = flash_area_write(
        staging.area, BOOTSTRAP_DATA_OFFSET + staging.persisted_length,
        staging.pending, staging.write_block);
    if (result == 0) {
        staging.persisted_length += (uint32_t)useful_length;
        staging.pending_length = 0U;
    }
    return result;
}

static int storage_begin(uint32_t payload_length, void *user_data) {
    ARG_UNUSED(user_data);
    struct bootstrap_header active_header;
    uint8_t active_slot;
    if (select_active_slot(&active_slot, &active_header) == 0) {
        staging.slot = active_slot == 0U ? 1U : 0U;
    } else {
        staging.slot = 0U;
    }

    staging.area = slots[staging.slot];
    staging.payload_length = payload_length;
    staging.accepted_length = 0U;
    staging.persisted_length = 0U;
    staging.pending_length = 0U;
    staging.write_block = flash_get_write_block_size(staging.area->fa_dev);
    if (staging.write_block == 0U || staging.write_block > WRITE_BUFFER_MAX ||
        payload_length > staging.area->fa_size - BOOTSTRAP_DATA_OFFSET) {
        return -ENOSPC;
    }
    int result = flash_area_erase(staging.area, 0, staging.area->fa_size);
    staging.active = result == 0;
    return result;
}

static int storage_write(uint32_t offset, const uint8_t *data, size_t length,
                         void *user_data) {
    ARG_UNUSED(user_data);
    if (!staging.active || offset != staging.accepted_length ||
        offset + length > staging.payload_length) {
        return -EINVAL;
    }

    while (length > 0U) {
        size_t available = staging.write_block - staging.pending_length;
        size_t copied = MIN(available, length);
        memcpy(staging.pending + staging.pending_length, data, copied);
        staging.pending_length += copied;
        staging.accepted_length += (uint32_t)copied;
        data += copied;
        length -= copied;
        if (staging.pending_length == staging.write_block &&
            staging_flush() != 0) {
            return -EIO;
        }
    }
    return 0;
}

static int storage_commit(
    uint32_t payload_length,
    const uint8_t digest[EXTRITTIO_SHA256_DIGEST_SIZE], void *user_data) {
    ARG_UNUSED(user_data);
    if (!staging.active || payload_length != staging.accepted_length ||
        payload_length != staging.payload_length || staging_flush() != 0) {
        return -EINVAL;
    }

    extrittio_bootstrap_t *candidate = k_malloc(sizeof(*candidate));
    if (candidate == NULL) {
        return -ENOMEM;
    }
    int result = read_and_validate(staging.area, payload_length, candidate);
    extrittio_bootstrap_clear(candidate);
    k_free(candidate);
    if (result != 0) {
        LOG_ERR("Staged bootstrap validation failed: %d", result);
        return result;
    }

    struct bootstrap_header current_header;
    uint8_t current_slot;
    uint32_t next_generation = 1U;
    if (select_active_slot(&current_slot, &current_header) == 0) {
        next_generation = current_header.generation + 1U;
    }
    struct bootstrap_header header = {
        .magic = BOOTSTRAP_MAGIC,
        .version = BOOTSTRAP_HEADER_VERSION,
        .generation = next_generation,
        .payload_length = payload_length,
    };
    memcpy(header.digest, digest, sizeof(header.digest));
    result = flash_area_write(staging.area, 0, &header, sizeof(header));
    if (result == 0) {
        result = settings_save_one("extrittio/bootstrap/active", &staging.slot,
                                   sizeof(staging.slot));
    }
    if (result == 0) {
        preferred_slot = staging.slot;
        LOG_INF("Activated bootstrap slot %u generation %u", staging.slot,
                next_generation);
    }
    staging.active = false;
    return result;
}

static void storage_abort(void *user_data) {
    ARG_UNUSED(user_data);
    staging.active = false;
    staging.pending_length = 0U;
}

static const extrittio_provisioning_storage_t callbacks = {
    .begin = storage_begin,
    .write = storage_write,
    .commit = storage_commit,
    .abort = storage_abort,
};

int extrittio_provisioning_store_init(void) {
    int result = flash_area_open(
        FIXED_PARTITION_ID(extrittio_bootstrap_a_partition), &slots[0]);
    if (result == 0) {
        result = flash_area_open(
            FIXED_PARTITION_ID(extrittio_bootstrap_b_partition), &slots[1]);
    }
    if (result != 0) {
        LOG_ERR("Could not open bootstrap flash slots: %d", result);
        return result;
    }
    result = settings_subsys_init();
    if (result != 0 && result != -EALREADY) {
        return result;
    }
    result = settings_load_subtree("extrittio/bootstrap");
    return result == -ENOENT ? 0 : result;
}

bool extrittio_provisioning_store_has_bootstrap(void) {
    uint8_t slot;
    struct bootstrap_header header;
    return select_active_slot(&slot, &header) == 0;
}

int extrittio_provisioning_store_load(extrittio_bootstrap_t *bootstrap) {
    if (bootstrap == NULL) {
        return -EINVAL;
    }
    uint8_t slot;
    struct bootstrap_header header;
    int result = select_active_slot(&slot, &header);
    if (result != 0) {
        return result;
    }
    return read_and_validate(slots[slot], header.payload_length, bootstrap);
}

const extrittio_provisioning_storage_t *extrittio_provisioning_store_callbacks(
    void) {
    return &callbacks;
}
