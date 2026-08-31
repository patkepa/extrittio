#ifndef EXTRITTIO_ZEPHYR_BLE_PROVISIONING_H
#define EXTRITTIO_ZEPHYR_BLE_PROVISIONING_H

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

int extrittio_ble_provisioning_init(void);
int extrittio_ble_provisioning_start(bool allow_reprovisioning);
int extrittio_ble_provisioning_stop(void);

#ifdef __cplusplus
}
#endif

#endif
