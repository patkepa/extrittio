#include "ble_contact.h"

#include <stdint.h>
#include <string.h>

#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/queue.h"
#include "freertos/task.h"
#include "host/ble_gap.h"
#include "host/ble_gatt.h"
#include "host/ble_hs.h"
#include "host/ble_uuid.h"
#include "host/util/util.h"
#include "nimble/nimble_port.h"
#include "programmable_led.h"
#include "services/gap/ble_svc_gap.h"
#include "services/gatt/ble_svc_gatt.h"
#include "sdkconfig.h"

#define BLE_CONTACT_DEVICE_NAME "Extrittio ESP32-C6"
#define BLE_CONTACT_ADV_INTERVAL_MS 100
#define BLE_CONTACT_MESSAGE_MAX_LENGTH 120
#define BLE_CONTACT_LED_FLASH_COUNT 3
#define BLE_CONTACT_LED_GREEN_LEVEL 94
#define BLE_CONTACT_LED_PHASE_MS 150
#define BLE_CONTACT_LED_QUEUE_DEPTH 4

static const char *TAG = "extrittio_ble";
static uint8_t s_own_addr_type;
static QueueHandle_t s_led_flash_queue;

/*
 * Public BLE contract (canonical UUID form):
 *   service:   9D8D0001-1A7C-4A21-9F25-E6B17EB89C11
 *   device ID: 9D8D0002-1A7C-4A21-9F25-E6B17EB89C11
 *   model:     9D8D0003-1A7C-4A21-9F25-E6B17EB89C11
 *   firmware:  9D8D0004-1A7C-4A21-9F25-E6B17EB89C11
 *   transport: 9D8D0005-1A7C-4A21-9F25-E6B17EB89C11
 *   message:   9D8D0006-1A7C-4A21-9F25-E6B17EB89C11
 *
 * BLE_UUID128_INIT takes bytes in little-endian order.
 */
static const ble_uuid128_t s_contact_service_uuid =
    BLE_UUID128_INIT(0x11, 0x9c, 0xb8, 0x7e, 0xb1, 0xe6, 0x25, 0x9f,
                     0x21, 0x4a, 0x7c, 0x1a, 0x01, 0x00, 0x8d, 0x9d);
static const ble_uuid128_t s_device_id_uuid =
    BLE_UUID128_INIT(0x11, 0x9c, 0xb8, 0x7e, 0xb1, 0xe6, 0x25, 0x9f,
                     0x21, 0x4a, 0x7c, 0x1a, 0x02, 0x00, 0x8d, 0x9d);
static const ble_uuid128_t s_model_uuid =
    BLE_UUID128_INIT(0x11, 0x9c, 0xb8, 0x7e, 0xb1, 0xe6, 0x25, 0x9f,
                     0x21, 0x4a, 0x7c, 0x1a, 0x03, 0x00, 0x8d, 0x9d);
static const ble_uuid128_t s_firmware_uuid =
    BLE_UUID128_INIT(0x11, 0x9c, 0xb8, 0x7e, 0xb1, 0xe6, 0x25, 0x9f,
                     0x21, 0x4a, 0x7c, 0x1a, 0x04, 0x00, 0x8d, 0x9d);
static const ble_uuid128_t s_transport_uuid =
    BLE_UUID128_INIT(0x11, 0x9c, 0xb8, 0x7e, 0xb1, 0xe6, 0x25, 0x9f,
                     0x21, 0x4a, 0x7c, 0x1a, 0x05, 0x00, 0x8d, 0x9d);
static const ble_uuid128_t s_message_uuid =
    BLE_UUID128_INIT(0x11, 0x9c, 0xb8, 0x7e, 0xb1, 0xe6, 0x25, 0x9f,
                     0x21, 0x4a, 0x7c, 0x1a, 0x06, 0x00, 0x8d, 0x9d);

static const char s_model[] = "ESP32-C6";
static const char s_transport[] = "OpenThread + Zenoh";

static void led_flash_task(void *context)
{
    (void)context;

    uint8_t flash_count;
    while (true) {
        if (xQueueReceive(s_led_flash_queue, &flash_count, portMAX_DELAY) != pdTRUE) {
            continue;
        }

        uint8_t completed_flashes = 0;
        for (uint8_t index = 0; index < flash_count; index++) {
            esp_err_t result = programmable_led_set_all_rgb(
                0, BLE_CONTACT_LED_GREEN_LEVEL, 0);
            if (result != ESP_OK) {
                ESP_LOGE(TAG, "Could not turn on contact LEDs: %s",
                         esp_err_to_name(result));
                break;
            }
            vTaskDelay(pdMS_TO_TICKS(BLE_CONTACT_LED_PHASE_MS));
            result = programmable_led_set_all_rgb(0, 0, 0);
            if (result != ESP_OK) {
                ESP_LOGE(TAG, "Could not turn off contact LEDs: %s",
                         esp_err_to_name(result));
                break;
            }
            vTaskDelay(pdMS_TO_TICKS(BLE_CONTACT_LED_PHASE_MS));
            completed_flashes++;
        }
        ESP_LOGI(TAG, "Contact write feedback complete (%u/%u flashes)",
                 completed_flashes, flash_count);
    }
}

static esp_err_t start_led_feedback(void)
{
    esp_err_t result = programmable_led_init();
    if (result != ESP_OK) {
        return result;
    }

    s_led_flash_queue = xQueueCreate(BLE_CONTACT_LED_QUEUE_DEPTH, sizeof(uint8_t));
    if (s_led_flash_queue == NULL) {
        return ESP_ERR_NO_MEM;
    }

    BaseType_t task_created = xTaskCreate(led_flash_task, "contact_led", 2048,
                                          NULL, 4, NULL);
    if (task_created != pdPASS) {
        vQueueDelete(s_led_flash_queue);
        s_led_flash_queue = NULL;
        return ESP_ERR_NO_MEM;
    }

    return ESP_OK;
}

static int identity_access(uint16_t conn_handle, uint16_t attr_handle,
                           struct ble_gatt_access_ctxt *ctxt, void *arg)
{
    (void)conn_handle;
    (void)attr_handle;

    if (ctxt->op != BLE_GATT_ACCESS_OP_READ_CHR || arg == NULL) {
        return BLE_ATT_ERR_UNLIKELY;
    }

    const char *value = arg;
    int result = os_mbuf_append(ctxt->om, value, strlen(value));
    return result == 0 ? 0 : BLE_ATT_ERR_INSUFFICIENT_RES;
}

static int message_access(uint16_t conn_handle, uint16_t attr_handle,
                          struct ble_gatt_access_ctxt *ctxt, void *arg)
{
    (void)conn_handle;
    (void)attr_handle;
    (void)arg;

    if (ctxt->op != BLE_GATT_ACCESS_OP_WRITE_CHR) {
        return BLE_ATT_ERR_UNLIKELY;
    }

    uint16_t message_length = OS_MBUF_PKTLEN(ctxt->om);
    if (message_length == 0 || message_length > BLE_CONTACT_MESSAGE_MAX_LENGTH) {
        return BLE_ATT_ERR_INVALID_ATTR_VALUE_LEN;
    }

    uint8_t flash_count = BLE_CONTACT_LED_FLASH_COUNT;
    if (s_led_flash_queue == NULL ||
        xQueueSend(s_led_flash_queue, &flash_count, 0) != pdTRUE) {
        ESP_LOGW(TAG, "Could not queue contact write feedback");
        return BLE_ATT_ERR_INSUFFICIENT_RES;
    }

    ESP_LOGI(TAG, "Accepted contact message (%u bytes); flashing both LEDs green",
             message_length);
    return 0;
}

static const struct ble_gatt_svc_def s_contact_services[] = {
    {
        .type = BLE_GATT_SVC_TYPE_PRIMARY,
        .uuid = &s_contact_service_uuid.u,
        .characteristics = (struct ble_gatt_chr_def[]){
            {
                .uuid = &s_device_id_uuid.u,
                .access_cb = identity_access,
                .arg = (void *)CONFIG_EXTRITTIO_DEVICE_ID,
                .flags = BLE_GATT_CHR_F_READ,
            },
            {
                .uuid = &s_model_uuid.u,
                .access_cb = identity_access,
                .arg = (void *)s_model,
                .flags = BLE_GATT_CHR_F_READ,
            },
            {
                .uuid = &s_firmware_uuid.u,
                .access_cb = identity_access,
                .arg = (void *)CONFIG_EXTRITTIO_FIRMWARE_VERSION,
                .flags = BLE_GATT_CHR_F_READ,
            },
            {
                .uuid = &s_transport_uuid.u,
                .access_cb = identity_access,
                .arg = (void *)s_transport,
                .flags = BLE_GATT_CHR_F_READ,
            },
            {
                .uuid = &s_message_uuid.u,
                .access_cb = message_access,
                .flags = BLE_GATT_CHR_F_WRITE,
            },
            {0},
        },
    },
    {0},
};

static void start_advertising(void);

static int gap_event_handler(struct ble_gap_event *event, void *arg)
{
    (void)arg;

    switch (event->type) {
    case BLE_GAP_EVENT_CONNECT:
        if (event->connect.status == 0) {
            ESP_LOGI(TAG, "Contact reader connected");
        } else {
            ESP_LOGW(TAG, "BLE connection failed (status=%d); advertising again",
                     event->connect.status);
            start_advertising();
        }
        return 0;
    case BLE_GAP_EVENT_DISCONNECT:
        ESP_LOGI(TAG, "Contact reader disconnected (reason=%d)",
                 event->disconnect.reason);
        start_advertising();
        return 0;
    case BLE_GAP_EVENT_ADV_COMPLETE:
        start_advertising();
        return 0;
    case BLE_GAP_EVENT_MTU:
        ESP_LOGD(TAG, "Contact connection MTU updated to %d", event->mtu.value);
        return 0;
    default:
        return 0;
    }
}

static void start_advertising(void)
{
    struct ble_hs_adv_fields advertisement = {0};
    advertisement.flags = BLE_HS_ADV_F_DISC_GEN | BLE_HS_ADV_F_BREDR_UNSUP;
    advertisement.tx_pwr_lvl = BLE_HS_ADV_TX_PWR_LVL_AUTO;
    advertisement.tx_pwr_lvl_is_present = 1;
    advertisement.uuids128 = (ble_uuid128_t *)&s_contact_service_uuid;
    advertisement.num_uuids128 = 1;
    advertisement.uuids128_is_complete = 1;

    int result = ble_gap_adv_set_fields(&advertisement);
    if (result != 0) {
        ESP_LOGE(TAG, "Could not set BLE contact advertisement (rc=%d)", result);
        return;
    }

    const char *name = ble_svc_gap_device_name();
    struct ble_hs_adv_fields scan_response = {0};
    scan_response.name = (uint8_t *)name;
    scan_response.name_len = strlen(name);
    scan_response.name_is_complete = 1;
    result = ble_gap_adv_rsp_set_fields(&scan_response);
    if (result != 0) {
        ESP_LOGE(TAG, "Could not set BLE contact scan response (rc=%d)", result);
        return;
    }

    struct ble_gap_adv_params parameters = {0};
    parameters.conn_mode = BLE_GAP_CONN_MODE_UND;
    parameters.disc_mode = BLE_GAP_DISC_MODE_GEN;
    parameters.itvl_min = BLE_GAP_ADV_ITVL_MS(BLE_CONTACT_ADV_INTERVAL_MS);
    parameters.itvl_max = BLE_GAP_ADV_ITVL_MS(BLE_CONTACT_ADV_INTERVAL_MS + 20);
    result = ble_gap_adv_start(s_own_addr_type, NULL, BLE_HS_FOREVER,
                               &parameters, gap_event_handler, NULL);
    if (result != 0) {
        ESP_LOGE(TAG, "Could not start BLE contact advertising (rc=%d)", result);
        return;
    }

    ESP_LOGI(TAG, "BLE contact pairing ready: device=%s", CONFIG_EXTRITTIO_DEVICE_ID);
}

static void on_stack_reset(int reason)
{
    ESP_LOGW(TAG, "NimBLE stack reset (reason=%d)", reason);
}

static void on_stack_sync(void)
{
    int result = ble_hs_util_ensure_addr(0);
    if (result != 0) {
        ESP_LOGE(TAG, "No usable BLE identity address (rc=%d)", result);
        return;
    }

    result = ble_hs_id_infer_auto(0, &s_own_addr_type);
    if (result != 0) {
        ESP_LOGE(TAG, "Could not infer BLE address type (rc=%d)", result);
        return;
    }

    start_advertising();
}

static void nimble_host_task(void *context)
{
    (void)context;
    nimble_port_run();
    vTaskDelete(NULL);
}

esp_err_t ble_contact_start(void)
{
    esp_err_t result = start_led_feedback();
    if (result != ESP_OK) {
        ESP_LOGE(TAG, "Could not initialize contact LED feedback: %s",
                 esp_err_to_name(result));
        return result;
    }

    result = nimble_port_init();
    if (result != ESP_OK) {
        ESP_LOGE(TAG, "Could not initialize NimBLE: %s", esp_err_to_name(result));
        return result;
    }

    ble_hs_cfg.reset_cb = on_stack_reset;
    ble_hs_cfg.sync_cb = on_stack_sync;

    ble_svc_gap_init();
    ble_svc_gatt_init();

    int rc = ble_svc_gap_device_name_set(BLE_CONTACT_DEVICE_NAME);
    if (rc == 0) {
        rc = ble_gatts_count_cfg(s_contact_services);
    }
    if (rc == 0) {
        rc = ble_gatts_add_svcs(s_contact_services);
    }
    if (rc != 0) {
        ESP_LOGE(TAG, "Could not configure BLE contact service (rc=%d)", rc);
        return ESP_FAIL;
    }

    BaseType_t task_created = xTaskCreate(nimble_host_task, "nimble_host", 4096,
                                          NULL, 5, NULL);
    if (task_created != pdPASS) {
        ESP_LOGE(TAG, "Could not create NimBLE host task");
        return ESP_ERR_NO_MEM;
    }

    return ESP_OK;
}
