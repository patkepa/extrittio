#ifndef EXTRITTIO_ZEPHYR_OTA_H
#define EXTRITTIO_ZEPHYR_OTA_H

#include <extrittio/bootstrap.h>
#include <extrittio/ota.h>

#ifdef __cplusplus
extern "C" {
#endif

int extrittio_zephyr_ota_install(
    const extrittio_bootstrap_t *bootstrap,
    const extrittio_ota_payload_t *update);

int extrittio_zephyr_ota_request(
    const extrittio_bootstrap_t *bootstrap,
    const extrittio_ota_payload_t *update);

#ifdef __cplusplus
}
#endif

#endif
