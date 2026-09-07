#include "ota_handler.h"
#include "extrittio/shadow.h"
#include "extrittio/time_utils.h"

#include "esp_log.h"
#include "esp_ota_ops.h"
#include "esp_http_client.h"
#include "esp_crt_bundle.h"
#include "nvs.h"
#include "esp_timer.h"
#include "esp_system.h"
#include "mbedtls/sha256.h"
#include <string.h>
#include <stdio.h>

static const char *TAG = "ota";
static bool boot_confirmed = false;
static esp_timer_handle_t boot_watchdog;
static void boot_timeout(void *arg) { (void)arg; esp_restart(); }
void ota_start_boot_watchdog(void) {
    const esp_partition_t *running = esp_ota_get_running_partition();
    esp_ota_img_states_t state;
    if (running && esp_ota_get_state_partition(running, &state) == ESP_OK && state == ESP_OTA_IMG_PENDING_VERIFY) {
        const esp_timer_create_args_t args = {.callback = boot_timeout, .name = "ota_health"};
        ESP_ERROR_CHECK(esp_timer_create(&args, &boot_watchdog));
        ESP_ERROR_CHECK(esp_timer_start_once(boot_watchdog, 120000000));
    }
}
typedef struct {
    uint32_t address;
    extrittio_ota_payload_t payload;
} ota_checkpoint_t;

static bool read_checkpoint(ota_checkpoint_t *checkpoint) {
    nvs_handle_t nvs;
    if (nvs_open("extrittio_ota", NVS_READONLY, &nvs) != ESP_OK) return false;
    size_t size = sizeof(*checkpoint);
    bool ok = nvs_get_blob(nvs, "attempt", checkpoint, &size) == ESP_OK && size == sizeof(*checkpoint);
    nvs_close(nvs);
    return ok;
}
static bool write_checkpoint(const ota_checkpoint_t *checkpoint) {
    nvs_handle_t nvs;
    if (nvs_open("extrittio_ota", NVS_READWRITE, &nvs) != ESP_OK) return false;
    bool ok = nvs_set_blob(nvs, "attempt", checkpoint, sizeof(*checkpoint)) == ESP_OK && nvs_commit(nvs) == ESP_OK;
    nvs_close(nvs);
    return ok;
}
const char *ota_current_version(const char *fallback) {
    static char version[64];
    if (version[0]) return version;
    ota_checkpoint_t checkpoint;
    const esp_partition_t *running = esp_ota_get_running_partition();
    const char *value = fallback;
    if (running && read_checkpoint(&checkpoint) && checkpoint.address == running->address) value = checkpoint.payload.firmware_version;
    snprintf(version, sizeof(version), "%s", value);
    return version;
}


static void report_ota_status(z_loaned_session_t *session,
                               const char *device_id,
                               const char *status,
                               const char *fw_version,
                               int64_t fw_update_id,
                               int64_t deployment_id,
                               const char *error) {
    char ota_json[512];
    int n = extrittio_ota_build_status_json(ota_json, sizeof(ota_json),
                                             status, fw_version,
                                             fw_update_id, deployment_id, error);
    if (n < 0) return;

    char state_json[1024];
    snprintf(state_json, sizeof(state_json), "{\"ota\":%s}", ota_json);

    extrittio_shadow_report_publish(session, device_id,
                                     extrittio_now_millis(), state_json, 0);
}

void ota_handle(z_loaned_session_t *session,
                const char *device_id,
                const char *current_fw,
                const extrittio_ota_payload_t *payload) {
    ota_checkpoint_t prior;
    if (read_checkpoint(&prior) && prior.payload.deployment_id == payload->deployment_id) {
        if (boot_confirmed) ota_confirm_boot(session, device_id);
        return;
    }
    if (!boot_confirmed) {
        report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
            payload->firmware_version, payload->firmware_update_id, payload->deployment_id,
            "Device startup health check is not complete");
        return;
    }

    ESP_LOGI(TAG, "Starting OTA: %s -> %s", current_fw, payload->firmware_version);

    report_ota_status(session, device_id, EXTRITTIO_OTA_DOWNLOADING,
                      payload->firmware_version, payload->firmware_update_id, payload->deployment_id, NULL);

    const esp_partition_t *update_partition = esp_ota_get_next_update_partition(NULL);
    if (!update_partition) {
        report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
                          payload->firmware_version, payload->firmware_update_id, payload->deployment_id,
                          "No OTA partition");
        return;
    }

    esp_ota_handle_t ota_handle_val = 0;
    esp_err_t err = esp_ota_begin(update_partition, OTA_WITH_SEQUENTIAL_WRITES, &ota_handle_val);
    if (err != ESP_OK) {
        report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
                          payload->firmware_version, payload->firmware_update_id, payload->deployment_id,
                          "esp_ota_begin failed");
        return;
    }

    esp_http_client_config_t http_config = {
        .url = payload->firmware_url,
        .timeout_ms = 30000,
        .crt_bundle_attach = esp_crt_bundle_attach,
    };
    esp_http_client_handle_t client = esp_http_client_init(&http_config);
    if (!client) {
        esp_ota_abort(ota_handle_val);
        report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED, payload->firmware_version,
            payload->firmware_update_id, payload->deployment_id, "HTTP client allocation failed");
        return;
    }
    err = esp_http_client_open(client, 0);
    if (err != ESP_OK) {
        esp_http_client_cleanup(client);
        esp_ota_abort(ota_handle_val);
        report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
                          payload->firmware_version, payload->firmware_update_id, payload->deployment_id,
                          "HTTP connect failed");
        return;
    }
    if (esp_http_client_fetch_headers(client) < 0 || esp_http_client_get_status_code(client) != 200) {
        esp_http_client_cleanup(client);
        esp_ota_abort(ota_handle_val);
        report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED, payload->firmware_version,
            payload->firmware_update_id, payload->deployment_id, "HTTP response failed");
        return;
    }

    mbedtls_sha256_context sha_ctx;
    mbedtls_sha256_init(&sha_ctx);
    mbedtls_sha256_starts(&sha_ctx, 0);

    char buf[4096];
    int total = 0;
    int read_len;
    while ((read_len = esp_http_client_read(client, buf, sizeof(buf))) > 0) {
        err = esp_ota_write(ota_handle_val, buf, read_len);
        if (err != ESP_OK) {
            esp_http_client_cleanup(client);
            esp_ota_abort(ota_handle_val);
            mbedtls_sha256_free(&sha_ctx);
            report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
                              payload->firmware_version, payload->firmware_update_id, payload->deployment_id,
                              "OTA write failed");
            return;
        }
        mbedtls_sha256_update(&sha_ctx, (const unsigned char *)buf, read_len);
        total += read_len;
    }
    esp_http_client_cleanup(client);

    ESP_LOGI(TAG, "Downloaded %d bytes", total);

    report_ota_status(session, device_id, EXTRITTIO_OTA_VERIFYING,
                      payload->firmware_version, payload->firmware_update_id, payload->deployment_id, NULL);

    if (payload->sha256[0] != '\0') {
        unsigned char hash[32];
        mbedtls_sha256_finish(&sha_ctx, hash);
        char hex[65] = {0};
        for (int i = 0; i < 32; i++) {
            sprintf(hex + i * 2, "%02x", hash[i]);
        }

        if (strcasecmp(hex, payload->sha256) != 0) {
            esp_ota_abort(ota_handle_val);
            mbedtls_sha256_free(&sha_ctx);
            report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
                              payload->firmware_version, payload->firmware_update_id, payload->deployment_id,
                              "SHA-256 mismatch");
            return;
        }
        ESP_LOGI(TAG, "SHA-256 verified");
    }
    mbedtls_sha256_free(&sha_ctx);

    report_ota_status(session, device_id, EXTRITTIO_OTA_INSTALLING,
                      payload->firmware_version, payload->firmware_update_id, payload->deployment_id, NULL);

    err = esp_ota_end(ota_handle_val);
    if (err != ESP_OK) {
        report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
                          payload->firmware_version, payload->firmware_update_id, payload->deployment_id,
                          "esp_ota_end failed");
        return;
    }

    ota_checkpoint_t checkpoint = {.address = update_partition->address, .payload = *payload};
    if (!write_checkpoint(&checkpoint)) {
        report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED, payload->firmware_version,
            payload->firmware_update_id, payload->deployment_id, "Failed to persist OTA attempt");
        return;
    }
    err = esp_ota_set_boot_partition(update_partition);
    if (err != ESP_OK) {
        report_ota_status(session, device_id, EXTRITTIO_OTA_FAILED,
                          payload->firmware_version, payload->firmware_update_id, payload->deployment_id,
                          "set boot partition failed");
        return;
    }

    report_ota_status(session, device_id, EXTRITTIO_OTA_REBOOTING,
                      payload->firmware_version, payload->firmware_update_id, payload->deployment_id, NULL);

    ESP_LOGI(TAG, "OTA complete, restarting...");
    esp_restart();
}

void ota_confirm_boot(z_loaned_session_t *session, const char *device_id) {
    const esp_partition_t *running = esp_ota_get_running_partition();
    if (!running) return;
    esp_ota_img_states_t state;
    if (esp_ota_get_state_partition(running, &state) == ESP_OK && state == ESP_OTA_IMG_PENDING_VERIFY
        && esp_ota_mark_app_valid_cancel_rollback() != ESP_OK) return;
    boot_confirmed = true;
    if (boot_watchdog) { esp_timer_stop(boot_watchdog); esp_timer_delete(boot_watchdog); boot_watchdog = NULL; }
    ota_checkpoint_t checkpoint;
    if (!read_checkpoint(&checkpoint)) return;
    bool success = checkpoint.address == running->address;
    report_ota_status(session, device_id, success ? EXTRITTIO_OTA_SUCCESS : EXTRITTIO_OTA_FAILED,
        checkpoint.payload.firmware_version, checkpoint.payload.firmware_update_id,
        checkpoint.payload.deployment_id, success ? NULL : "New image failed to boot; rolled back");
}
