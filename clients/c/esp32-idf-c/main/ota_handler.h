#ifndef OTA_HANDLER_H
#define OTA_HANDLER_H

#include "extrittio/ota.h"
#include <zenoh-pico.h>
void ota_confirm_boot(z_loaned_session_t *session, const char *device_id);
void ota_start_boot_watchdog(void);
const char *ota_current_version(const char *fallback);

void ota_handle(z_loaned_session_t *session,
                const char *device_id,
                const char *current_fw,
                const extrittio_ota_payload_t *payload);

#endif /* OTA_HANDLER_H */
