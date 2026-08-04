#ifndef EXTRITTIO_SHADOW_H
#define EXTRITTIO_SHADOW_H

#include <stddef.h>
#include <stdint.h>
#include <zenoh-pico.h>

#ifdef __cplusplus
extern "C" {
#endif

int extrittio_shadow_report_publish(z_loaned_session_t *session,
                                     const char *device_id,
                                     int64_t timestamp,
                                     const char *state_json,
                                     int64_t version);

int extrittio_shadow_get_publish(z_loaned_session_t *session,
                                  const char *device_id);

typedef void (*extrittio_shadow_delta_cb)(const char *device_id,
                                          const char *delta_json,
                                          int64_t version,
                                          void *user_data);

int extrittio_shadow_delta_subscribe(z_loaned_session_t *session,
                                      const char *device_id,
                                      extrittio_shadow_delta_cb cb,
                                      void *user_data,
                                      z_owned_subscriber_t *sub);

#ifdef __cplusplus
}
#endif

#endif /* EXTRITTIO_SHADOW_H */
