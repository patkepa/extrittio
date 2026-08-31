#include "extrittio_zephyr/ble_provisioning.h"

#include <errno.h>
#include <stdio.h>
#include <string.h>

#include <zephyr/bluetooth/bluetooth.h>
#include <zephyr/bluetooth/conn.h>
#include <zephyr/bluetooth/gatt.h>
#include <zephyr/bluetooth/uuid.h>
#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>
#include <zephyr/settings/settings.h>
#include <zephyr/sys/reboot.h>

#include <extrittio/provisioning.h>

#include "extrittio_zephyr/identity.h"
#include "extrittio_zephyr/provisioning_store.h"

LOG_MODULE_REGISTER(extrittio_ble, CONFIG_LOG_DEFAULT_LEVEL);

#define EX_UUID(value) BT_UUID_128_ENCODE(value, 0x1a7c, 0x4a21, 0x9f25, 0xe6b17eb89c11)

static struct bt_uuid_128 service_uuid =
    BT_UUID_INIT_128(EX_UUID(0x9d8d0001));
static struct bt_uuid_128 device_id_uuid =
    BT_UUID_INIT_128(EX_UUID(0x9d8d0002));
static struct bt_uuid_128 model_uuid =
    BT_UUID_INIT_128(EX_UUID(0x9d8d0003));
static struct bt_uuid_128 firmware_uuid =
    BT_UUID_INIT_128(EX_UUID(0x9d8d0004));
static struct bt_uuid_128 transport_uuid =
    BT_UUID_INIT_128(EX_UUID(0x9d8d0005));
static struct bt_uuid_128 message_uuid =
    BT_UUID_INIT_128(EX_UUID(0x9d8d0006));
static struct bt_uuid_128 capabilities_uuid =
    BT_UUID_INIT_128(EX_UUID(0x9d8d0007));

static extrittio_provisioning_transfer_t transfer;
static bool provisioning_allowed;
static char capabilities[192];
static struct k_work_delayable reboot_work;

static ssize_t read_text(struct bt_conn *connection,
                         const struct bt_gatt_attr *attribute, void *buffer,
                         uint16_t length, uint16_t offset,
                         const char *value) {
    return bt_gatt_attr_read(connection, attribute, buffer, length, offset,
                             value, strlen(value));
}

static ssize_t read_device_id(struct bt_conn *connection,
                              const struct bt_gatt_attr *attribute,
                              void *buffer, uint16_t length, uint16_t offset) {
    return read_text(connection, attribute, buffer, length, offset,
                     extrittio_factory_device_id());
}

static ssize_t read_model(struct bt_conn *connection,
                          const struct bt_gatt_attr *attribute, void *buffer,
                          uint16_t length, uint16_t offset) {
    return read_text(connection, attribute, buffer, length, offset,
                     extrittio_device_model());
}

static ssize_t read_firmware(struct bt_conn *connection,
                             const struct bt_gatt_attr *attribute,
                             void *buffer, uint16_t length, uint16_t offset) {
    return read_text(connection, attribute, buffer, length, offset,
                     extrittio_firmware_version());
}

static ssize_t read_transport(struct bt_conn *connection,
                              const struct bt_gatt_attr *attribute,
                              void *buffer, uint16_t length, uint16_t offset) {
    return read_text(connection, attribute, buffer, length, offset,
                     "thread+zenoh-mtls");
}

static ssize_t read_capabilities(struct bt_conn *connection,
                                 const struct bt_gatt_attr *attribute,
                                 void *buffer, uint16_t length,
                                 uint16_t offset) {
    return read_text(connection, attribute, buffer, length, offset,
                     capabilities);
}

static void reboot_handler(struct k_work *work) {
    ARG_UNUSED(work);
    sys_reboot(SYS_REBOOT_COLD);
}

static ssize_t write_message(struct bt_conn *connection,
                             const struct bt_gatt_attr *attribute,
                             const void *buffer, uint16_t length,
                             uint16_t offset, uint8_t flags) {
    ARG_UNUSED(connection);
    ARG_UNUSED(attribute);
    ARG_UNUSED(flags);
    if (!provisioning_allowed || offset != 0U) {
        return BT_GATT_ERR(BT_ATT_ERR_WRITE_NOT_PERMITTED);
    }

    extrittio_provisioning_result_t result =
        extrittio_provisioning_transfer_handle(
            &transfer, buffer, length,
            extrittio_provisioning_store_callbacks(), NULL);
    if (result != EXTRITTIO_PROVISIONING_OK) {
        LOG_WRN("Rejected provisioning frame: %d", result);
        return BT_GATT_ERR(BT_ATT_ERR_VALUE_NOT_ALLOWED);
    }
    if (length >= 4U && ((const uint8_t *)buffer)[3] ==
                            EXTRITTIO_PROVISIONING_COMMIT_OPCODE) {
        provisioning_allowed = false;
        (void)k_work_reschedule(&reboot_work, K_SECONDS(1));
    }
    return length;
}

BT_GATT_SERVICE_DEFINE(
    extrittio_service,
    BT_GATT_PRIMARY_SERVICE(&service_uuid),
    BT_GATT_CHARACTERISTIC(&device_id_uuid.uuid, BT_GATT_CHRC_READ,
                           BT_GATT_PERM_READ, read_device_id, NULL, NULL),
    BT_GATT_CHARACTERISTIC(&model_uuid.uuid, BT_GATT_CHRC_READ,
                           BT_GATT_PERM_READ, read_model, NULL, NULL),
    BT_GATT_CHARACTERISTIC(&firmware_uuid.uuid, BT_GATT_CHRC_READ,
                           BT_GATT_PERM_READ, read_firmware, NULL, NULL),
    BT_GATT_CHARACTERISTIC(&transport_uuid.uuid, BT_GATT_CHRC_READ,
                           BT_GATT_PERM_READ, read_transport, NULL, NULL),
    BT_GATT_CHARACTERISTIC(&message_uuid.uuid,
                           BT_GATT_CHRC_WRITE,
                           BT_GATT_PERM_WRITE_ENCRYPT,
                           NULL, write_message, NULL),
    BT_GATT_CHARACTERISTIC(&capabilities_uuid.uuid, BT_GATT_CHRC_READ,
                           BT_GATT_PERM_READ, read_capabilities, NULL, NULL));

static void connected(struct bt_conn *connection, uint8_t error) {
    if (error == 0U) {
        int result = bt_conn_set_security(connection, BT_SECURITY_L2);
        if (result != 0 && result != -EALREADY) {
            LOG_WRN("Could not request encrypted BLE link: %d", result);
        }
    }
}

static void disconnected(struct bt_conn *connection, uint8_t reason) {
    ARG_UNUSED(connection);
    LOG_DBG("BLE disconnected: 0x%02x", reason);
    extrittio_provisioning_transfer_abort(
        &transfer, extrittio_provisioning_store_callbacks(), NULL);
}

BT_CONN_CB_DEFINE(connection_callbacks) = {
    .connected = connected,
    .disconnected = disconnected,
};

static const struct bt_data advertising_data[] = {
    BT_DATA_BYTES(BT_DATA_FLAGS, BT_LE_AD_GENERAL | BT_LE_AD_NO_BREDR),
    BT_DATA_BYTES(BT_DATA_UUID128_ALL, EX_UUID(0x9d8d0001)),
};

static const struct bt_data scan_response[] = {
    BT_DATA(BT_DATA_NAME_COMPLETE, CONFIG_BT_DEVICE_NAME,
            sizeof(CONFIG_BT_DEVICE_NAME) - 1U),
};

int extrittio_ble_provisioning_init(void) {
    int result = bt_enable(NULL);
    if (result != 0) {
        return result;
    }
    (void)settings_load_subtree("bt");
    extrittio_provisioning_transfer_init(
        &transfer, CONFIG_EXTRITTIO_PROVISIONING_MAX_PAYLOAD);
    snprintf(capabilities, sizeof(capabilities),
             "{\"bootstrap\":[3,4],\"network\":[\"thread\"],"
             "\"transport\":[\"zenoh-mtls\"],\"max_payload\":%u}",
             CONFIG_EXTRITTIO_PROVISIONING_MAX_PAYLOAD);
    k_work_init_delayable(&reboot_work, reboot_handler);
    return 0;
}

int extrittio_ble_provisioning_start(bool allow_reprovisioning) {
    if (extrittio_provisioning_store_has_bootstrap() &&
        !allow_reprovisioning) {
        return -EPERM;
    }
    provisioning_allowed = true;
    int result = bt_le_adv_start(BT_LE_ADV_CONN_FAST_1, advertising_data,
                                 ARRAY_SIZE(advertising_data), scan_response,
                                 ARRAY_SIZE(scan_response));
    if (result == 0) {
        LOG_INF("BLE provisioning window opened");
    }
    return result;
}

int extrittio_ble_provisioning_stop(void) {
    provisioning_allowed = false;
    extrittio_provisioning_transfer_abort(
        &transfer, extrittio_provisioning_store_callbacks(), NULL);
    int result = bt_le_adv_stop();
    return result == -EALREADY ? 0 : result;
}
