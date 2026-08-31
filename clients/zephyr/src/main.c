#include <errno.h>
#include <stdbool.h>

#include <zephyr/device.h>
#include <zephyr/devicetree.h>
#include <zephyr/drivers/gpio.h>
#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>

#include <extrittio/bootstrap.h>

#include "extrittio_zephyr/ble_provisioning.h"
#include "extrittio_zephyr/client.h"
#include "extrittio_zephyr/identity.h"
#include "extrittio_zephyr/network.h"
#include "extrittio_zephyr/provisioning_store.h"

LOG_MODULE_REGISTER(extrittio_main, CONFIG_LOG_DEFAULT_LEVEL);

#if DT_NODE_HAS_STATUS(DT_ALIAS(sw0), okay)
static const struct gpio_dt_spec provisioning_button =
    GPIO_DT_SPEC_GET(DT_ALIAS(sw0), gpios);
#endif

static bool provisioning_button_pressed(void) {
#if DT_NODE_HAS_STATUS(DT_ALIAS(sw0), okay)
    if (!gpio_is_ready_dt(&provisioning_button) ||
        gpio_pin_configure_dt(&provisioning_button, GPIO_INPUT) != 0) {
        return false;
    }
    return gpio_pin_get_dt(&provisioning_button) > 0;
#else
    return false;
#endif
}

static void default_command(const extrittio_command_t *command,
                            void *user_data) {
    ARG_UNUSED(user_data);
    LOG_WRN("No application handler for command %s", command->command);
}

static void default_shadow(const char *device_id, const char *delta_json,
                           int64_t version, void *user_data) {
    ARG_UNUSED(device_id);
    ARG_UNUSED(delta_json);
    ARG_UNUSED(user_data);
    LOG_INF("Received shadow delta version %lld", version);
}

int main(void) {
    if (extrittio_identity_init() != 0 ||
        extrittio_provisioning_store_init() != 0 ||
        extrittio_ble_provisioning_init() != 0) {
        LOG_ERR("Extrittio platform initialization failed");
        return -EIO;
    }

    bool provisioned = extrittio_provisioning_store_has_bootstrap();
    bool physical_reprovision = provisioned && provisioning_button_pressed();
    if (!provisioned || physical_reprovision) {
        if (extrittio_ble_provisioning_start(physical_reprovision) != 0) {
            LOG_ERR("Could not open BLE provisioning window");
            return -EIO;
        }
        k_sleep(K_SECONDS(CONFIG_EXTRITTIO_PROVISIONING_WINDOW_SECONDS));
        (void)extrittio_ble_provisioning_stop();
        if (!provisioned) {
            LOG_WRN("Provisioning window expired; reboot to try again");
            return -ETIMEDOUT;
        }
    }

    extrittio_bootstrap_t *bootstrap = k_malloc(sizeof(*bootstrap));
    if (bootstrap == NULL) {
        return -ENOMEM;
    }
    int result = extrittio_provisioning_store_load(bootstrap);
    if (result != 0) {
        LOG_ERR("Stored bootstrap is invalid: %d", result);
        k_free(bootstrap);
        return result;
    }
    result = extrittio_network_start(&bootstrap->network);
    if (result != 0) {
        LOG_ERR("Network start failed: %d", result);
        extrittio_bootstrap_clear(bootstrap);
        k_free(bootstrap);
        return result;
    }

    const extrittio_zephyr_client_callbacks_t callbacks = {
        .command = default_command,
        .shadow_delta = default_shadow,
    };
    result = extrittio_zephyr_client_run(bootstrap, &callbacks);
    extrittio_bootstrap_clear(bootstrap);
    k_free(bootstrap);
    return result;
}
