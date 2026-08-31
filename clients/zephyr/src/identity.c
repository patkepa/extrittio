#include "extrittio_zephyr/identity.h"

#include <stdio.h>

#include <zephyr/drivers/hwinfo.h>
#include <zephyr/logging/log.h>

LOG_MODULE_REGISTER(extrittio_identity, CONFIG_LOG_DEFAULT_LEVEL);

static char factory_id[129];

int extrittio_identity_init(void) {
    if (CONFIG_EXTRITTIO_FACTORY_DEVICE_ID[0] != '\0') {
        snprintf(factory_id, sizeof(factory_id), "%s",
                 CONFIG_EXTRITTIO_FACTORY_DEVICE_ID);
        return 0;
    }

    uint8_t hardware_id[16];
    ssize_t length = hwinfo_get_device_id(hardware_id, sizeof(hardware_id));
    if (length <= 0) {
        LOG_ERR("No factory identity is configured and hardware ID is unavailable");
        return -1;
    }

    size_t position = 0;
    position += (size_t)snprintf(factory_id, sizeof(factory_id), "zephyr-");
    for (ssize_t index = 0; index < length && position + 2 < sizeof(factory_id);
         index++) {
        position += (size_t)snprintf(factory_id + position,
                                     sizeof(factory_id) - position,
                                     "%02x", hardware_id[index]);
    }
    LOG_INF("Using hardware-derived factory identity %s", factory_id);
    return 0;
}

const char *extrittio_factory_device_id(void) { return factory_id; }

const char *extrittio_device_model(void) { return CONFIG_EXTRITTIO_MODEL; }

const char *extrittio_firmware_version(void) {
    return CONFIG_EXTRITTIO_FIRMWARE_VERSION;
}
