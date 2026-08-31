#include "extrittio_zephyr/network.h"

#include <errno.h>
#include <string.h>

#include <openthread/dataset.h>
#include <openthread/ip6.h>
#include <openthread/thread.h>
#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>
#include <zephyr/net/net_if.h>
#include <zephyr/net/openthread.h>

LOG_MODULE_REGISTER(extrittio_network, CONFIG_LOG_DEFAULT_LEVEL);

static bool ready;

static int hex_nibble(char value) {
    if (value >= '0' && value <= '9') {
        return value - '0';
    }
    if (value >= 'a' && value <= 'f') {
        return value - 'a' + 10;
    }
    if (value >= 'A' && value <= 'F') {
        return value - 'A' + 10;
    }
    return -1;
}

static int decode_dataset(const char *hex, otOperationalDatasetTlvs *dataset) {
    size_t length = strlen(hex);
    if (length == 0U || (length & 1U) != 0U ||
        length / 2U > sizeof(dataset->mTlvs)) {
        return -EINVAL;
    }
    dataset->mLength = (uint8_t)(length / 2U);
    for (size_t index = 0; index < dataset->mLength; index++) {
        int upper = hex_nibble(hex[index * 2U]);
        int lower = hex_nibble(hex[index * 2U + 1U]);
        if (upper < 0 || lower < 0) {
            return -EINVAL;
        }
        dataset->mTlvs[index] = (uint8_t)((upper << 4) | lower);
    }
    return 0;
}

static int start_thread(const extrittio_bootstrap_network_t *network) {
    otOperationalDatasetTlvs dataset = {0};
    int result = decode_dataset(network->thread_dataset_hex, &dataset);
    if (result != 0) {
        return result;
    }

    struct openthread_context *context = openthread_get_default_context();
    if (context == NULL) {
        return -ENODEV;
    }
    otInstance *instance = openthread_get_default_instance();
    openthread_api_mutex_lock(context);
    otError error = otDatasetSetActiveTlvs(instance, &dataset);
    if (error == OT_ERROR_NONE) {
        error = otIp6SetEnabled(instance, true);
    }
    if (error == OT_ERROR_NONE) {
        error = otThreadSetEnabled(instance, true);
    }
    openthread_api_mutex_unlock(context);
    if (error != OT_ERROR_NONE) {
        LOG_ERR("OpenThread start failed: %d", error);
        return -EIO;
    }

    int64_t deadline = k_uptime_get() + 60000;
    while (k_uptime_get() < deadline) {
        openthread_api_mutex_lock(context);
        otDeviceRole role = otThreadGetDeviceRole(instance);
        openthread_api_mutex_unlock(context);
        if (role == OT_DEVICE_ROLE_CHILD || role == OT_DEVICE_ROLE_ROUTER ||
            role == OT_DEVICE_ROLE_LEADER) {
            ready = true;
            LOG_INF("Attached to Thread network as role %d", role);
            return 0;
        }
        k_sleep(K_MSEC(250));
    }
    return -ETIMEDOUT;
}

static int wait_for_default_interface(void) {
    struct net_if *interface = net_if_get_default();
    if (interface == NULL) {
        return -ENODEV;
    }
    int64_t deadline = k_uptime_get() + 60000;
    while (k_uptime_get() < deadline) {
        if (net_if_is_up(interface) && net_if_is_carrier_ok(interface)) {
            ready = true;
            return 0;
        }
        k_sleep(K_MSEC(250));
    }
    return -ETIMEDOUT;
}

int extrittio_network_start(const extrittio_bootstrap_network_t *network) {
    if (network == NULL) {
        return -EINVAL;
    }
    ready = false;
    switch (network->type) {
    case EXTRITTIO_NETWORK_THREAD:
        return start_thread(network);
    case EXTRITTIO_NETWORK_ETHERNET:
        return wait_for_default_interface();
    case EXTRITTIO_NETWORK_WIFI:
        LOG_ERR("This board build has no Wi-Fi credentials adapter");
        return -ENOTSUP;
    default:
        return -EINVAL;
    }
}

bool extrittio_network_is_ready(void) { return ready; }
