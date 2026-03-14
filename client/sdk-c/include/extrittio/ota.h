#ifndef EXTRITTIO_OTA_H
#define EXTRITTIO_OTA_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define EXTRITTIO_OTA_DOWNLOADING "downloading"
#define EXTRITTIO_OTA_VERIFYING   "verifying"
#define EXTRITTIO_OTA_INSTALLING  "installing"
#define EXTRITTIO_OTA_SUCCESS     "success"
#define EXTRITTIO_OTA_FAILED      "failed"

typedef struct {
    char firmware_version[64];
    char firmware_url[256];
    int64_t firmware_update_id;
    char sha256[65];
} extrittio_ota_payload_t;

bool extrittio_ota_parse_from_delta(const char *delta_json,
                                     extrittio_ota_payload_t *out);

int extrittio_ota_build_status_json(char *buf, size_t len,
                                     const char *status,
                                     const char *fw_version,
                                     int64_t fw_update_id,
                                     const char *error);

bool extrittio_ota_is_terminal(const char *status);

#ifdef __cplusplus
}
#endif

#endif /* EXTRITTIO_OTA_H */
