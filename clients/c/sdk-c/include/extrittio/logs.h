#ifndef EXTRITTIO_LOGS_H
#define EXTRITTIO_LOGS_H

#include <stdint.h>
#include <zenoh-pico.h>

#ifdef __cplusplus
extern "C" {
#endif

int extrittio_log_publish(z_loaned_session_t *session,
                           const char *device_id,
                           int64_t timestamp,
                           const char *level,
                           const char *message);

#ifdef __cplusplus
}
#endif

#endif /* EXTRITTIO_LOGS_H */
