#ifndef EXTRITTIO_ZEPHYR_PROVISIONING_STORE_H
#define EXTRITTIO_ZEPHYR_PROVISIONING_STORE_H

#include <stdbool.h>

#include <extrittio/bootstrap.h>
#include <extrittio/provisioning.h>

#ifdef __cplusplus
extern "C" {
#endif

int extrittio_provisioning_store_init(void);
bool extrittio_provisioning_store_has_bootstrap(void);
int extrittio_provisioning_store_load(extrittio_bootstrap_t *bootstrap);
const extrittio_provisioning_storage_t *extrittio_provisioning_store_callbacks(void);

#ifdef __cplusplus
}
#endif

#endif
