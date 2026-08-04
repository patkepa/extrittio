#ifndef EXTRITTIO_HEARTBEAT_H
#define EXTRITTIO_HEARTBEAT_H

#include <stddef.h>
#include <stdint.h>
#include <zenoh-pico.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    const char *device_id;
    int64_t timestamp;
    const char *status;
    const char *firmware;
    int64_t uptime_seconds;
} extrittio_heartbeat_t;

int extrittio_heartbeat_encode(const extrittio_heartbeat_t *h,
                                uint8_t *buf, size_t len, size_t *written);

int extrittio_heartbeat_publish(z_loaned_session_t *session,
                                 const extrittio_heartbeat_t *h);

#ifdef __cplusplus
}
#endif

#endif /* EXTRITTIO_HEARTBEAT_H */
