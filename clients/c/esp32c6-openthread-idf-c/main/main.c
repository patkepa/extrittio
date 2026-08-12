#include <ctype.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "esp_event.h"
#include "esp_log.h"
#include "esp_netif.h"
#include "esp_openthread.h"
#include "esp_openthread_lock.h"
#include "esp_openthread_netif_glue.h"
#include "esp_openthread_types.h"
#include "esp_timer.h"
#include "esp_vfs_eventfd.h"
#include "freertos/FreeRTOS.h"
#include "freertos/event_groups.h"
#include "freertos/task.h"
#include "nvs_flash.h"
#include "openthread/dataset.h"
#include "openthread/error.h"
#include "sdkconfig.h"

#include "extrittio/extrittio.h"
#include <zenoh-pico.h>

#define THREAD_ATTACHED_BIT BIT0

static const char *TAG = "extrittio_thread";
static EventGroupHandle_t s_thread_events;

static int hex_nibble(char value)
{
    if (value >= '0' && value <= '9') {
        return value - '0';
    }
    value = (char)tolower((unsigned char)value);
    if (value >= 'a' && value <= 'f') {
        return value - 'a' + 10;
    }
    return -1;
}

/* Accept whitespace so an exported dataset can be pasted in grouped form. */
static esp_err_t parse_dataset_tlvs(const char *hex, otOperationalDatasetTlvs *dataset)
{
    size_t written = 0;
    int high = -1;

    memset(dataset, 0, sizeof(*dataset));
    for (const char *cursor = hex; *cursor != '\0'; cursor++) {
        if (isspace((unsigned char)*cursor)) {
            continue;
        }
        int nibble = hex_nibble(*cursor);
        if (nibble < 0) {
            ESP_LOGE(TAG, "Thread dataset contains a non-hex character");
            return ESP_ERR_INVALID_ARG;
        }
        if (high < 0) {
            high = nibble;
            continue;
        }
        if (written == OT_OPERATIONAL_DATASET_MAX_LENGTH) {
            ESP_LOGE(TAG, "Thread dataset is longer than %d bytes", OT_OPERATIONAL_DATASET_MAX_LENGTH);
            return ESP_ERR_INVALID_SIZE;
        }
        dataset->mTlvs[written++] = (uint8_t)((high << 4) | nibble);
        high = -1;
    }

    if (high >= 0 || written == 0) {
        ESP_LOGE(TAG, "Thread dataset must be a non-empty, even-length hex string");
        return ESP_ERR_INVALID_ARG;
    }
    dataset->mLength = (uint8_t)written;
    return ESP_OK;
}

static void thread_event_handler(void *arg, esp_event_base_t event_base,
                                 int32_t event_id, void *event_data)
{
    (void)arg;
    (void)event_base;
    (void)event_data;

    if (event_id == OPENTHREAD_EVENT_ATTACHED) {
        ESP_LOGI(TAG, "Attached to the configured Thread network");
        xEventGroupSetBits(s_thread_events, THREAD_ATTACHED_BIT);
    } else if (event_id == OPENTHREAD_EVENT_DETACHED) {
        ESP_LOGW(TAG, "Detached from the Thread network");
        xEventGroupClearBits(s_thread_events, THREAD_ATTACHED_BIT);
    }
}

static esp_netif_t *init_openthread_netif(const esp_openthread_platform_config_t *config)
{
    esp_netif_config_t netif_config = ESP_NETIF_DEFAULT_OPENTHREAD();
    esp_netif_t *netif = esp_netif_new(&netif_config);
    assert(netif != NULL);
    ESP_ERROR_CHECK(esp_netif_attach(netif, esp_openthread_netif_glue_init(config)));
    return netif;
}

static void openthread_task(void *context)
{
    (void)context;
    esp_openthread_platform_config_t config = {
        .radio_config = {.radio_mode = RADIO_MODE_NATIVE},
        .host_config = {.host_connection_mode = HOST_CONNECTION_MODE_NONE},
        .port_config = {
            .storage_partition_name = "nvs",
            .netif_queue_size = 10,
            .task_queue_size = 10,
        },
    };
    ESP_ERROR_CHECK(esp_openthread_init(&config));

    esp_netif_t *netif = init_openthread_netif(&config);
    esp_netif_set_default_netif(netif);

    otOperationalDatasetTlvs dataset;
    ESP_ERROR_CHECK(parse_dataset_tlvs(CONFIG_EXTRITTIO_THREAD_ACTIVE_DATASET_TLVS,
                                       &dataset));

    esp_openthread_lock_acquire(portMAX_DELAY);
    otError error = otDatasetSetActiveTlvs(esp_openthread_get_instance(), &dataset);
    esp_openthread_lock_release();
    if (error != OT_ERROR_NONE) {
        ESP_LOGE(TAG, "Active Operational Dataset was rejected: %d", error);
        abort();
    }

    ESP_ERROR_CHECK(esp_openthread_auto_start(&dataset));
    ESP_ERROR_CHECK(esp_openthread_launch_mainloop());
    abort();
}

static void publish_loop(void)
{
    z_owned_config_t config;
    z_config_default(&config);
    zp_config_insert(z_loan_mut(config), Z_CONFIG_CONNECT_KEY,
                     CONFIG_EXTRITTIO_ZENOH_CONNECT);

    z_owned_session_t session;
    if (z_open(&session, z_move(config), NULL) != 0) {
        ESP_LOGE(TAG, "Zenoh connection failed: %s", CONFIG_EXTRITTIO_ZENOH_CONNECT);
        return;
    }
    if (zp_start_read_task(z_loan_mut(session), NULL) != 0 ||
        zp_start_lease_task(z_loan_mut(session), NULL) != 0) {
        ESP_LOGE(TAG, "Failed to start Zenoh background tasks");
        z_drop(z_move(session));
        return;
    }

    ESP_LOGI(TAG, "Zenoh connected: device=%s endpoint=%s",
             CONFIG_EXTRITTIO_DEVICE_ID, CONFIG_EXTRITTIO_ZENOH_CONNECT);
    int64_t boot_time_us = esp_timer_get_time();
    int64_t last_heartbeat_ms = 0;
    extrittio_sensor_state_t sensor;
    extrittio_sensor_init(&sensor);

    while (true) {
        EventBits_t bits = xEventGroupGetBits(s_thread_events);
        if ((bits & THREAD_ATTACHED_BIT) == 0) {
            ESP_LOGW(TAG, "Thread detached; waiting before the next publish");
            vTaskDelay(pdMS_TO_TICKS(1000));
            continue;
        }

        int64_t now = extrittio_now_millis();
        float temperature_offset = ((float)(rand() % 100) / 100.0f) - 0.5f;
        float humidity_offset = ((float)(rand() % 200) / 100.0f) - 1.0f;
        extrittio_sensor_step(&sensor, temperature_offset, humidity_offset, 0.05f);

        extrittio_telemetry_t telemetry = {
            .device_id = CONFIG_EXTRITTIO_DEVICE_ID,
            .timestamp = now,
            .temperature = sensor.temperature,
            .humidity = sensor.humidity,
            .battery_level = sensor.battery,
        };
        extrittio_telemetry_publish(z_loan_mut(session), &telemetry);

        if (now - last_heartbeat_ms >= CONFIG_EXTRITTIO_HEARTBEAT_INTERVAL_S * 1000LL) {
            extrittio_heartbeat_t heartbeat = {
                .device_id = CONFIG_EXTRITTIO_DEVICE_ID,
                .timestamp = now,
                .status = EXTRITTIO_STATUS_ONLINE,
                .firmware = CONFIG_EXTRITTIO_FIRMWARE_VERSION,
                .uptime_seconds = (esp_timer_get_time() - boot_time_us) / 1000000,
            };
            extrittio_heartbeat_publish(z_loan_mut(session), &heartbeat);
            last_heartbeat_ms = now;
        }
        vTaskDelay(pdMS_TO_TICKS(CONFIG_EXTRITTIO_TELEMETRY_INTERVAL_S * 1000));
    }
}

void app_main(void)
{
    ESP_ERROR_CHECK(nvs_flash_init());
    ESP_ERROR_CHECK(esp_netif_init());
    ESP_ERROR_CHECK(esp_event_loop_create_default());

    esp_vfs_eventfd_config_t eventfd_config = {.max_fds = 3};
    ESP_ERROR_CHECK(esp_vfs_eventfd_register(&eventfd_config));

    s_thread_events = xEventGroupCreate();
    configASSERT(s_thread_events != NULL);
    ESP_ERROR_CHECK(esp_event_handler_register(OPENTHREAD_EVENT, ESP_EVENT_ANY_ID,
                                                thread_event_handler, NULL));

    xTaskCreate(openthread_task, "openthread", 10240, NULL, 5, NULL);
    ESP_LOGI(TAG, "Waiting for Thread attachment");
    xEventGroupWaitBits(s_thread_events, THREAD_ATTACHED_BIT, pdFALSE, pdTRUE,
                        portMAX_DELAY);
    publish_loop();
}
