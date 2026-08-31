#ifndef EXTRITTIO_ZEPHYR_CLIENT_H
#define EXTRITTIO_ZEPHYR_CLIENT_H

#include <stddef.h>

#include <extrittio/bootstrap.h>
#include <extrittio/commands.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void (*extrittio_zephyr_route_cb)(const char *route_key,
                                         const uint8_t *payload,
                                         size_t payload_length,
                                         void *user_data);

typedef struct {
    extrittio_command_cb command;
    extrittio_shadow_delta_cb shadow_delta;
    extrittio_zephyr_route_cb route;
    void *user_data;
} extrittio_zephyr_client_callbacks_t;

/* Runs the reconnecting Zenoh lifecycle until a fatal configuration error. */
int extrittio_zephyr_client_run(
    const extrittio_bootstrap_t *bootstrap,
    const extrittio_zephyr_client_callbacks_t *callbacks);

#ifdef __cplusplus
}
#endif

#endif
