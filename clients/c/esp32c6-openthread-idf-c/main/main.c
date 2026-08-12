#include <ctype.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
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
#include "openthread/dns_client.h"
#include "openthread/error.h"
#include "openthread/ip6.h"
#include "sdkconfig.h"

#include "extrittio/extrittio.h"
#include <zenoh-pico.h>

#define THREAD_ATTACHED_BIT BIT0
#define BACKEND_DISCOVERY_QUEUE_DEPTH 1
#define BACKEND_HOSTNAME_MAX_LENGTH 128
#define BACKEND_TXT_MAX_LENGTH 128
#define BACKEND_LOCATOR_MAX_LENGTH 128
#define BACKEND_DISCOVERY_TIMEOUT_MS 20000

static const char *TAG = "extrittio_thread";
static EventGroupHandle_t s_thread_events;
static QueueHandle_t s_backend_discovery_queue;

typedef struct {
    char locator[BACKEND_LOCATOR_MAX_LENGTH];
    bool tls_enabled;
} backend_endpoint_t;

static void initialize_nvs(void)
{
    esp_err_t err = nvs_flash_init();
    if (err == ESP_ERR_NVS_NO_FREE_PAGES || err == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_LOGW(TAG, "NVS needs recovery (%s); erasing its partition", esp_err_to_name(err));
        ESP_ERROR_CHECK(nvs_flash_erase());
        err = nvs_flash_init();
    }
    ESP_ERROR_CHECK(err);
}

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

static bool txt_property_is_true(const uint8_t *txt, uint16_t txt_length, const char *name)
{
    size_t name_length = strlen(name);
    uint16_t offset = 0;

    while (offset < txt_length) {
        uint8_t length = txt[offset++];
        if (length == 0 || length > txt_length - offset) {
            return false;
        }
        const uint8_t *property = &txt[offset];
        if (length == name_length + 2 && memcmp(property, name, name_length) == 0 &&
            property[name_length] == '=' && property[name_length + 1] == '1') {
            return true;
        }
        offset += length;
    }
    return false;
}

static void backend_service_callback(otError error, const otDnsServiceResponse *response,
                                     void *context)
{
    (void)context;
    if (error != OT_ERROR_NONE) {
        ESP_LOGW(TAG, "Thread DNS-SD service resolution failed: %d", error);
        return;
    }

    char hostname[BACKEND_HOSTNAME_MAX_LENGTH] = {0};
    uint8_t txt[BACKEND_TXT_MAX_LENGTH] = {0};
    otDnsServiceInfo service = {
        .mHostNameBuffer = hostname,
        .mHostNameBufferSize = sizeof(hostname),
        .mTxtData = txt,
        .mTxtDataSize = sizeof(txt),
    };
    otError result = otDnsServiceResponseGetServiceInfo(response, &service);
    if (result != OT_ERROR_NONE || otIp6IsAddressUnspecified(&service.mHostAddress) ||
        service.mPort == 0) {
        ESP_LOGW(TAG, "Thread DNS-SD service has no usable IPv6 endpoint: %d", result);
        return;
    }

    backend_endpoint_t endpoint = {0};
    char address[OT_IP6_ADDRESS_STRING_SIZE];
    otIp6AddressToString(&service.mHostAddress, address, sizeof(address));
    endpoint.tls_enabled = txt_property_is_true(txt, service.mTxtDataSize, "tls");
    int written = snprintf(endpoint.locator, sizeof(endpoint.locator), "%s/[%s]:%u",
                           endpoint.tls_enabled ? "tls" : "tcp", address, service.mPort);
    if (written < 0 || written >= sizeof(endpoint.locator)) {
        ESP_LOGW(TAG, "Discovered Zenoh endpoint is too long");
        return;
    }
    xQueueOverwrite(s_backend_discovery_queue, &endpoint);
    ESP_LOGI(TAG, "Discovered Extrittio Zenoh endpoint: %s", endpoint.locator);
}

static bool discover_backend(backend_endpoint_t *endpoint)
{
    xQueueReset(s_backend_discovery_queue);
    const otDnsQueryConfig query_config = {
        // OTBR's Discovery Proxy receives separate SRV and TXT questions on
        // macOS reliably, whereas a combined question can wait for the DNS
        // client's fallback timeout before the proxy responds.
        .mResponseTimeout = BACKEND_DISCOVERY_TIMEOUT_MS,
        .mMaxTxAttempts = 1,
        .mServiceMode = OT_DNS_SERVICE_MODE_SRV_TXT_SEPARATE,
    };
    esp_openthread_lock_acquire(portMAX_DELAY);
    otError error = otDnsClientResolveServiceAndHostAddress(
        esp_openthread_get_instance(), CONFIG_EXTRITTIO_THREAD_ZENOH_INSTANCE_NAME,
        CONFIG_EXTRITTIO_THREAD_ZENOH_SERVICE_NAME, backend_service_callback, NULL, &query_config);
    esp_openthread_lock_release();
    if (error != OT_ERROR_NONE) {
        ESP_LOGW(TAG, "Could not start Thread DNS-SD service resolution: %d", error);
        return false;
    }
    return xQueueReceive(s_backend_discovery_queue, endpoint,
                         pdMS_TO_TICKS(BACKEND_DISCOVERY_TIMEOUT_MS)) == pdTRUE;
}

static bool configured_backend(backend_endpoint_t *endpoint)
{
    const char *locator = CONFIG_EXTRITTIO_THREAD_ZENOH_LOCATOR;

    if (locator[0] == '\0') {
        return false;
    }
    if (strlen(locator) >= sizeof(endpoint->locator)) {
        ESP_LOGE(TAG, "Configured Thread Zenoh locator is too long");
        return false;
    }
    memset(endpoint, 0, sizeof(*endpoint));
    strcpy(endpoint->locator, locator);
    endpoint->tls_enabled = strncmp(locator, "tls/", 4) == 0;
    ESP_LOGW(TAG, "Using configured Thread Zenoh locator after DNS-SD timeout: %s",
             endpoint->locator);
    return true;
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

        backend_endpoint_t endpoint;
        if (!discover_backend(&endpoint) && !configured_backend(&endpoint)) {
            ESP_LOGW(TAG, "Extrittio DNS-SD service not found and no locator fallback is configured; retrying");
            vTaskDelay(pdMS_TO_TICKS(5000));
            continue;
        }
        if (endpoint.tls_enabled) {
            ESP_LOGE(TAG, "Discovered TLS Zenoh service, but this example has no mTLS credentials");
            vTaskDelay(pdMS_TO_TICKS(10000));
            continue;
        }

        z_owned_config_t config;
        z_config_default(&config);
        zp_config_insert(z_loan_mut(config), Z_CONFIG_CONNECT_KEY, endpoint.locator);
        z_owned_session_t session;
        if (z_open(&session, z_move(config), NULL) != 0) {
            ESP_LOGW(TAG, "Zenoh connection failed: %s", endpoint.locator);
            vTaskDelay(pdMS_TO_TICKS(5000));
            continue;
        }
        if (zp_start_read_task(z_loan_mut(session), NULL) != 0 ||
            zp_start_lease_task(z_loan_mut(session), NULL) != 0) {
            ESP_LOGE(TAG, "Failed to start Zenoh background tasks");
            z_drop(z_move(session));
            vTaskDelay(pdMS_TO_TICKS(5000));
            continue;
        }
        ESP_LOGI(TAG, "Zenoh connected: device=%s endpoint=%s",
                 CONFIG_EXTRITTIO_DEVICE_ID, endpoint.locator);

        while (xEventGroupGetBits(s_thread_events) & THREAD_ATTACHED_BIT) {
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
        z_drop(z_move(session));
    }
}

void app_main(void)
{
    initialize_nvs();
    ESP_ERROR_CHECK(esp_netif_init());
    ESP_ERROR_CHECK(esp_event_loop_create_default());

    esp_vfs_eventfd_config_t eventfd_config = {.max_fds = 3};
    ESP_ERROR_CHECK(esp_vfs_eventfd_register(&eventfd_config));

    s_thread_events = xEventGroupCreate();
    configASSERT(s_thread_events != NULL);
    s_backend_discovery_queue = xQueueCreate(BACKEND_DISCOVERY_QUEUE_DEPTH,
                                              sizeof(backend_endpoint_t));
    configASSERT(s_backend_discovery_queue != NULL);
    ESP_ERROR_CHECK(esp_event_handler_register(OPENTHREAD_EVENT, ESP_EVENT_ANY_ID,
                                                thread_event_handler, NULL));

    xTaskCreate(openthread_task, "openthread", 10240, NULL, 5, NULL);
    ESP_LOGI(TAG, "Waiting for Thread attachment");
    xEventGroupWaitBits(s_thread_events, THREAD_ATTACHED_BIT, pdFALSE, pdTRUE,
                        portMAX_DELAY);
    publish_loop();
}
