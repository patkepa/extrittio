#ifndef EXTRITTIO_TOPICS_H
#define EXTRITTIO_TOPICS_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

int extrittio_topic_telemetry(char *buf, size_t len, const char *device_id);
int extrittio_topic_heartbeat(char *buf, size_t len, const char *device_id);
int extrittio_topic_shadow_report(char *buf, size_t len, const char *device_id);
int extrittio_topic_shadow_delta(char *buf, size_t len, const char *device_id);
int extrittio_topic_shadow_get(char *buf, size_t len, const char *device_id);
int extrittio_topic_commands(char *buf, size_t len, const char *device_id);
int extrittio_topic_commands_response(char *buf, size_t len, const char *device_id);
int extrittio_topic_logs(char *buf, size_t len, const char *device_id);

#ifdef __cplusplus
}
#endif

#endif /* EXTRITTIO_TOPICS_H */
