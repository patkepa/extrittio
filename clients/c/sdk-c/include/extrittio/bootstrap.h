#ifndef EXTRITTIO_BOOTSTRAP_H
#define EXTRITTIO_BOOTSTRAP_H

#include <stddef.h>

#include "extrittio/contract.h"

#ifdef __cplusplus
extern "C" {
#endif

#define EXTRITTIO_BOOTSTRAP_VERSION_3 3
#define EXTRITTIO_BOOTSTRAP_VERSION_4 4
#define EXTRITTIO_BOOTSTRAP_PEM_MAX 4095
#define EXTRITTIO_BOOTSTRAP_THREAD_DATASET_HEX_MAX 508

typedef enum {
    EXTRITTIO_NETWORK_THREAD,
    EXTRITTIO_NETWORK_WIFI,
    EXTRITTIO_NETWORK_ETHERNET,
} extrittio_network_type_t;

typedef struct {
    extrittio_network_type_t type;
    char thread_dataset_hex[EXTRITTIO_BOOTSTRAP_THREAD_DATASET_HEX_MAX + 1];
    char zenoh_service[128];
    char wifi_ssid[33];
    char wifi_password[65];
} extrittio_bootstrap_network_t;

typedef struct {
    char certificate_pem[EXTRITTIO_BOOTSTRAP_PEM_MAX + 1];
    char private_key_pem[EXTRITTIO_BOOTSTRAP_PEM_MAX + 1];
    char ca_pem[EXTRITTIO_BOOTSTRAP_PEM_MAX + 1];
    char fingerprint[129];
} extrittio_bootstrap_credentials_t;

typedef struct {
    int version;
    char factory_device_id[129];
    char device_id[129];
    char device_name[129];
    char backend_address[256];
    char api_base_url[512];
    extrittio_bootstrap_network_t network;
    extrittio_contract_t contract;
    extrittio_bootstrap_credentials_t credentials;
} extrittio_bootstrap_t;

typedef enum {
    EXTRITTIO_BOOTSTRAP_OK = 0,
    EXTRITTIO_BOOTSTRAP_ERR_ARGUMENT = -1,
    EXTRITTIO_BOOTSTRAP_ERR_JSON = -2,
    EXTRITTIO_BOOTSTRAP_ERR_VERSION = -3,
    EXTRITTIO_BOOTSTRAP_ERR_REQUIRED = -4,
    EXTRITTIO_BOOTSTRAP_ERR_LIMIT = -5,
    EXTRITTIO_BOOTSTRAP_ERR_IDENTITY = -6,
    EXTRITTIO_BOOTSTRAP_ERR_CONTRACT = -7,
} extrittio_bootstrap_result_t;

extrittio_bootstrap_result_t extrittio_bootstrap_parse(
    const char *json,
    size_t json_length,
    extrittio_bootstrap_t *bootstrap);

extrittio_bootstrap_result_t extrittio_bootstrap_validate_factory(
    const extrittio_bootstrap_t *bootstrap,
    const char *factory_device_id);

void extrittio_bootstrap_clear(extrittio_bootstrap_t *bootstrap);

#ifdef __cplusplus
}
#endif

#endif /* EXTRITTIO_BOOTSTRAP_H */
