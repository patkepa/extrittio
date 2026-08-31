#ifndef EXTRITTIO_ZEPHYR_NETWORK_H
#define EXTRITTIO_ZEPHYR_NETWORK_H

#include <stdbool.h>

#include <extrittio/bootstrap.h>

#ifdef __cplusplus
extern "C" {
#endif

int extrittio_network_start(const extrittio_bootstrap_network_t *network);
bool extrittio_network_is_ready(void);

#ifdef __cplusplus
}
#endif

#endif
