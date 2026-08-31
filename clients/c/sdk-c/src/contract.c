#include "extrittio/contract.h"

#include <cJSON.h>
#include <stdbool.h>
#include <string.h>

const extrittio_contract_route_t *extrittio_contract_route(
    const extrittio_contract_t *contract,
    const char *route_key) {
    if (contract == NULL || route_key == NULL) {
        return NULL;
    }
    for (size_t index = 0; index < contract->route_count; index++) {
        if (strcmp(contract->routes[index].key, route_key) == 0) {
            return &contract->routes[index];
        }
    }
    return NULL;
}

const extrittio_contract_command_t *extrittio_contract_command(
    const extrittio_contract_t *contract,
    const char *command_key) {
    if (contract == NULL || command_key == NULL) {
        return NULL;
    }
    for (size_t index = 0; index < contract->command_count; index++) {
        if (strcmp(contract->commands[index].key, command_key) == 0) {
            return &contract->commands[index];
        }
    }
    return NULL;
}

extrittio_contract_result_t extrittio_contract_event_encode(
    const extrittio_contract_t *contract,
    const char *route_key,
    const char *event_id,
    const char *occurred_at,
    const char *payload_json,
    char *buffer,
    size_t buffer_length,
    size_t *written) {
    if (contract == NULL || route_key == NULL || event_id == NULL ||
        occurred_at == NULL || payload_json == NULL || buffer == NULL ||
        buffer_length == 0U || written == NULL) {
        return EXTRITTIO_CONTRACT_ERR_ARGUMENT;
    }
    const extrittio_contract_route_t *route =
        extrittio_contract_route(contract, route_key);
    if (route == NULL || route->direction != EXTRITTIO_ROUTE_DEVICE_TO_CLOUD ||
        route->encoding != EXTRITTIO_ROUTE_ENCODING_JSON) {
        return EXTRITTIO_CONTRACT_ERR_ROUTE;
    }
    if (contract->hash[0] == '\0') {
        return EXTRITTIO_CONTRACT_ERR_REQUIRED;
    }

    cJSON *payload = cJSON_Parse(payload_json);
    if (payload == NULL) {
        return EXTRITTIO_CONTRACT_ERR_JSON;
    }
    cJSON *envelope = cJSON_CreateObject();
    if (envelope == NULL ||
        !cJSON_AddNumberToObject(envelope, "apiVersion", 1) ||
        !cJSON_AddStringToObject(envelope, "eventId", event_id) ||
        !cJSON_AddStringToObject(envelope, "contractHash", contract->hash) ||
        !cJSON_AddStringToObject(envelope, "occurredAt", occurred_at)) {
        cJSON_Delete(payload);
        cJSON_Delete(envelope);
        return EXTRITTIO_CONTRACT_ERR_JSON;
    }
    cJSON_AddItemToObject(envelope, "payload", payload);
    bool printed = cJSON_PrintPreallocated(envelope, buffer, (int)buffer_length, false);
    cJSON_Delete(envelope);
    if (!printed) {
        return EXTRITTIO_CONTRACT_ERR_SIZE;
    }
    *written = strlen(buffer);
    if ((contract->max_message_bytes != 0U &&
         *written > contract->max_message_bytes) ||
        *written >= buffer_length) {
        buffer[0] = '\0';
        *written = 0U;
        return EXTRITTIO_CONTRACT_ERR_SIZE;
    }
    return EXTRITTIO_CONTRACT_OK;
}
