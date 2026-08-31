#ifndef EXTRITTIO_ZEPHYR_IDENTITY_H
#define EXTRITTIO_ZEPHYR_IDENTITY_H

#ifdef __cplusplus
extern "C" {
#endif

int extrittio_identity_init(void);
const char *extrittio_factory_device_id(void);
const char *extrittio_device_model(void);
const char *extrittio_firmware_version(void);

#ifdef __cplusplus
}
#endif

#endif
