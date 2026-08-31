#ifndef EXTRITTIO_CONTRACT_H
#define EXTRITTIO_CONTRACT_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define EXTRITTIO_CONTRACT_HASH_LENGTH 64
#define EXTRITTIO_CONTRACT_MAX_ROUTES 16
#define EXTRITTIO_CONTRACT_MAX_COMMANDS 8
#define EXTRITTIO_CONTRACT_KEY_MAX 64
#define EXTRITTIO_CONTRACT_ADDRESS_MAX 255
#define EXTRITTIO_CONTRACT_ENDPOINT_MAX 255

typedef enum {
    EXTRITTIO_ROUTE_DEVICE_TO_CLOUD,
    EXTRITTIO_ROUTE_CLOUD_TO_DEVICE,
} extrittio_route_direction_t;

typedef enum {
    EXTRITTIO_ROUTE_ENCODING_JSON,
    EXTRITTIO_ROUTE_ENCODING_PROTOBUF,
} extrittio_route_encoding_t;

typedef struct {
    char key[EXTRITTIO_CONTRACT_KEY_MAX + 1];
    char address[EXTRITTIO_CONTRACT_ADDRESS_MAX + 1];
    extrittio_route_direction_t direction;
    extrittio_route_encoding_t encoding;
} extrittio_contract_route_t;

typedef struct {
    char key[EXTRITTIO_CONTRACT_KEY_MAX + 1];
    char request_route[EXTRITTIO_CONTRACT_KEY_MAX + 1];
    char response_route[EXTRITTIO_CONTRACT_KEY_MAX + 1];
    uint64_t timeout_ms;
} extrittio_contract_command_t;

typedef struct {
    uint32_t contract_api;
    char hash[EXTRITTIO_CONTRACT_HASH_LENGTH + 1];
    char device_id[129];
    char zenoh_endpoint[EXTRITTIO_CONTRACT_ENDPOINT_MAX + 1];
    uint64_t heartbeat_interval_ms;
    uint64_t offline_after_ms;
    uint64_t max_message_bytes;
    size_t route_count;
    extrittio_contract_route_t routes[EXTRITTIO_CONTRACT_MAX_ROUTES];
    size_t command_count;
    extrittio_contract_command_t commands[EXTRITTIO_CONTRACT_MAX_COMMANDS];
    char firmware_strategy[32];
} extrittio_contract_t;

typedef enum {
    EXTRITTIO_CONTRACT_OK = 0,
    EXTRITTIO_CONTRACT_ERR_ARGUMENT = -1,
    EXTRITTIO_CONTRACT_ERR_JSON = -2,
    EXTRITTIO_CONTRACT_ERR_REQUIRED = -3,
    EXTRITTIO_CONTRACT_ERR_UNSUPPORTED = -4,
    EXTRITTIO_CONTRACT_ERR_LIMIT = -5,
    EXTRITTIO_CONTRACT_ERR_ROUTE = -6,
    EXTRITTIO_CONTRACT_ERR_SIZE = -7,
} extrittio_contract_result_t;

const extrittio_contract_route_t *extrittio_contract_route(
    const extrittio_contract_t *contract,
    const char *route_key);

const extrittio_contract_command_t *extrittio_contract_command(
    const extrittio_contract_t *contract,
    const char *command_key);

extrittio_contract_result_t extrittio_contract_event_encode(
    const extrittio_contract_t *contract,
    const char *route_key,
    const char *event_id,
    const char *occurred_at,
    const char *payload_json,
    char *buffer,
    size_t buffer_length,
    size_t *written);

#ifdef __cplusplus
}
#endif

#endif /* EXTRITTIO_CONTRACT_H */
