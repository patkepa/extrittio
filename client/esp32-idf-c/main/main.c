#include <string.h>
#include <stdlib.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_log.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "nvs_flash.h"
#include "sdkconfig.h"

#include "wifi.h"
#include "ota_handler.h"

#include "extrittio/extrittio.h"
#include <zenoh-pico.h>

static const char *TAG = "main";

static z_loaned_session_t *g_session = NULL;

static void shadow_delta_callback(const char *device_id,
                                   const char *delta_json,
                                   int64_t version,
                                   void *user_data) {
    ESP_LOGI(TAG, "Shadow delta v%lld: %s", (long long)version, delta_json);

    extrittio_ota_payload_t ota;
    if (extrittio_ota_parse_from_delta(delta_json, &ota)) {
        ota_handle(g_session, CONFIG_EXTRITTIO_DEVICE_ID,
                   CONFIG_EXTRITTIO_FIRMWARE_VERSION, &ota);
    }

    extrittio_shadow_report_publish(g_session, CONFIG_EXTRITTIO_DEVICE_ID,
                                     extrittio_now_millis(), delta_json, version);
}

static void command_callback(const extrittio_command_t *cmd, void *user_data) {
    ESP_LOGI(TAG, "Command: %s (id=%s) params=%s",
             cmd->command, cmd->correlation_id, cmd->params_json);

    extrittio_command_response_publish(
        g_session, CONFIG_EXTRITTIO_DEVICE_ID,
        cmd->correlation_id, "succeeded", "{}", extrittio_now_millis());
}

void app_main(void) {
    esp_err_t ret = nvs_flash_init();
    if (ret == ESP_ERR_NVS_NO_FREE_PAGES ||
        ret == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_ERROR_CHECK(nvs_flash_erase());
        ret = nvs_flash_init();
    }
    ESP_ERROR_CHECK(ret);

    ESP_ERROR_CHECK(wifi_init_sta());

    z_owned_config_t config;
    z_config_default(&config);
    zp_config_insert(z_loan_mut(config), Z_CONFIG_CONNECT_KEY,
                     CONFIG_EXTRITTIO_ZENOH_CONNECT);

    z_owned_session_t session;
    if (z_open(&session, z_move(config), NULL) != 0) {
        ESP_LOGE(TAG, "Failed to open Zenoh session");
        return;
    }

    if (zp_start_read_task(z_loan_mut(session), NULL) != 0 ||
        zp_start_lease_task(z_loan_mut(session), NULL) != 0) {
        ESP_LOGE(TAG, "Failed to start Zenoh tasks");
        z_drop(z_move(session));
        return;
    }

    g_session = z_loan(session);
    ESP_LOGI(TAG, "Zenoh session opened, device=%s", CONFIG_EXTRITTIO_DEVICE_ID);

    z_owned_subscriber_t shadow_sub, cmd_sub;
    extrittio_shadow_delta_subscribe(z_loan(session),
                                      CONFIG_EXTRITTIO_DEVICE_ID,
                                      shadow_delta_callback, NULL, &shadow_sub);
    extrittio_command_subscribe(z_loan(session),
                                CONFIG_EXTRITTIO_DEVICE_ID,
                                command_callback, NULL, &cmd_sub);

    extrittio_shadow_get_publish(z_loan(session), CONFIG_EXTRITTIO_DEVICE_ID);

    int64_t boot_time = esp_timer_get_time();
    extrittio_sensor_state_t sensor;
    extrittio_sensor_init(&sensor);

    int64_t last_heartbeat = 0;

    ESP_LOGI(TAG, "Entering main loop (telemetry=%ds, heartbeat=%ds)",
             CONFIG_EXTRITTIO_TELEMETRY_INTERVAL_S,
             CONFIG_EXTRITTIO_HEARTBEAT_INTERVAL_S);

    while (1) {
        int64_t now = extrittio_now_millis();

        float t_off = ((float)(rand() % 100) / 100.0f) - 0.5f;
        float h_off = ((float)(rand() % 200) / 100.0f) - 1.0f;
        float drain = 0.05f + ((float)(rand() % 100) / 1000.0f);
        extrittio_sensor_step(&sensor, t_off, h_off, drain);

        extrittio_telemetry_t tel = {
            .device_id = CONFIG_EXTRITTIO_DEVICE_ID,
            .timestamp = now,
            .temperature = sensor.temperature,
            .humidity = sensor.humidity,
            .battery_level = sensor.battery,
        };
        extrittio_telemetry_publish(z_loan(session), &tel);

        int64_t hb_interval_ms = CONFIG_EXTRITTIO_HEARTBEAT_INTERVAL_S * 1000LL;
        if (now - last_heartbeat >= hb_interval_ms) {
            int64_t uptime_us = esp_timer_get_time() - boot_time;
            extrittio_heartbeat_t hb = {
                .device_id = CONFIG_EXTRITTIO_DEVICE_ID,
                .timestamp = now,
                .status = EXTRITTIO_STATUS_ONLINE,
                .firmware = CONFIG_EXTRITTIO_FIRMWARE_VERSION,
                .uptime_seconds = uptime_us / 1000000,
            };
            extrittio_heartbeat_publish(z_loan(session), &hb);
            last_heartbeat = now;
        }

        vTaskDelay(pdMS_TO_TICKS(CONFIG_EXTRITTIO_TELEMETRY_INTERVAL_S * 1000));
    }
}
