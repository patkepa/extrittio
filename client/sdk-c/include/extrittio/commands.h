#ifndef EXTRITTIO_COMMANDS_H
#define EXTRITTIO_COMMANDS_H

#include <stdint.h>
#include <zenoh-pico.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    const char *command;
    const char *params_json;
    const char *correlation_id;
} extrittio_command_t;

typedef void (*extrittio_command_cb)(const extrittio_command_t *cmd,
                                     void *user_data);

int extrittio_command_subscribe(z_loaned_session_t *session,
                                const char *device_id,
                                extrittio_command_cb cb,
                                void *user_data,
                                z_owned_subscriber_t *sub);

int extrittio_command_response_publish(z_loaned_session_t *session,
                                       const char *device_id,
                                       const char *correlation_id,
                                       const char *status,
                                       const char *payload,
                                       int64_t timestamp);

#ifdef __cplusplus
}
#endif

#endif /* EXTRITTIO_COMMANDS_H */
