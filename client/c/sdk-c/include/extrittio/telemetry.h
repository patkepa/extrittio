#ifndef EXTRITTIO_TELEMETRY_H
#define EXTRITTIO_TELEMETRY_H

#include <stddef.h>
#include <stdint.h>
#include <zenoh-pico.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    const char *device_id;
    int64_t timestamp;
    float temperature;
    float humidity;
    float battery_level;
    double latitude;
    double longitude;
    float speed;
    float altitude;
    float heading;
} extrittio_telemetry_t;

int extrittio_telemetry_encode(const extrittio_telemetry_t *t,
                                uint8_t *buf, size_t len, size_t *written);

int extrittio_telemetry_publish(z_loaned_session_t *session,
                                 const extrittio_telemetry_t *t);

#ifdef __cplusplus
}
#endif

#endif /* EXTRITTIO_TELEMETRY_H */
