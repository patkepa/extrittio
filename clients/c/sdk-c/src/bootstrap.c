#include "extrittio/bootstrap.h"

#include <cJSON.h>
#include <ctype.h>
#include <stdbool.h>
#include <string.h>

static bool copy_string(const cJSON *object, const char *key, char *destination,
                        size_t destination_length, bool required) {
    const cJSON *item = cJSON_GetObjectItemCaseSensitive(object, key);
    if (!cJSON_IsString(item) || item->valuestring == NULL) {
        return !required;
    }
    size_t length = strlen(item->valuestring);
    if (length == 0U || length >= destination_length) {
        return false;
    }
    memcpy(destination, item->valuestring, length + 1U);
    return true;
}

static bool valid_hash(const char *hash) {
    if (strlen(hash) != EXTRITTIO_CONTRACT_HASH_LENGTH) {
        return false;
    }
    for (size_t index = 0; index < EXTRITTIO_CONTRACT_HASH_LENGTH; index++) {
        if (!isxdigit((unsigned char)hash[index])) {
            return false;
        }
    }
    return true;
}

static bool parse_network(const cJSON *root, int version,
                          extrittio_bootstrap_network_t *network) {
    const cJSON *network_json = version == EXTRITTIO_BOOTSTRAP_VERSION_3
                                    ? cJSON_GetObjectItemCaseSensitive(root, "thread")
                                    : cJSON_GetObjectItemCaseSensitive(root, "network");
    if (!cJSON_IsObject(network_json)) {
        return false;
    }
    if (version == EXTRITTIO_BOOTSTRAP_VERSION_3) {
        network->type = EXTRITTIO_NETWORK_THREAD;
        return copy_string(network_json, "active_dataset_tlvs",
                           network->thread_dataset_hex,
                           sizeof(network->thread_dataset_hex), true);
    }

    char type[16] = {0};
    if (!copy_string(network_json, "type", type, sizeof(type), true)) {
        return false;
    }
    if (strcmp(type, "thread") == 0) {
        network->type = EXTRITTIO_NETWORK_THREAD;
        return copy_string(network_json, "active_dataset_tlvs",
                           network->thread_dataset_hex,
                           sizeof(network->thread_dataset_hex), true) &&
               copy_string(network_json, "zenoh_service", network->zenoh_service,
                           sizeof(network->zenoh_service), false);
    }
    if (strcmp(type, "wifi") == 0) {
        network->type = EXTRITTIO_NETWORK_WIFI;
        return copy_string(network_json, "ssid", network->wifi_ssid,
                           sizeof(network->wifi_ssid), true) &&
               copy_string(network_json, "password", network->wifi_password,
                           sizeof(network->wifi_password), true);
    }
    if (strcmp(type, "ethernet") == 0) {
        network->type = EXTRITTIO_NETWORK_ETHERNET;
        return true;
    }
    return false;
}

static bool parse_route(const cJSON *route_json, const char *key,
                        extrittio_contract_route_t *route) {
    if (strlen(key) > EXTRITTIO_CONTRACT_KEY_MAX) {
        return false;
    }
    strcpy(route->key, key);
    if (!copy_string(route_json, "address", route->address,
                     sizeof(route->address), true)) {
        return false;
    }
    const cJSON *direction =
        cJSON_GetObjectItemCaseSensitive(route_json, "direction");
    const cJSON *encoding = cJSON_GetObjectItemCaseSensitive(route_json, "encoding");
    if (!cJSON_IsString(direction) || !cJSON_IsString(encoding)) {
        return false;
    }
    if (strcmp(direction->valuestring, "device_to_cloud") == 0) {
        route->direction = EXTRITTIO_ROUTE_DEVICE_TO_CLOUD;
    } else if (strcmp(direction->valuestring, "cloud_to_device") == 0) {
        route->direction = EXTRITTIO_ROUTE_CLOUD_TO_DEVICE;
    } else {
        return false;
    }
    if (strcmp(encoding->valuestring, "json") == 0) {
        route->encoding = EXTRITTIO_ROUTE_ENCODING_JSON;
    } else if (strcmp(encoding->valuestring, "protobuf") == 0) {
        route->encoding = EXTRITTIO_ROUTE_ENCODING_PROTOBUF;
    } else {
        return false;
    }
    return true;
}

static bool parse_contract_document(const cJSON *document,
                                    extrittio_contract_t *contract) {
    const cJSON *contract_api =
        cJSON_GetObjectItemCaseSensitive(document, "contractApi");
    if (!cJSON_IsNumber(contract_api) || contract_api->valuedouble < 1.0 ||
        contract_api->valuedouble > 1.0) {
        return false;
    }
    contract->contract_api = (uint32_t)contract_api->valuedouble;
    if (!copy_string(document, "deviceId", contract->device_id,
                     sizeof(contract->device_id), true)) {
        return false;
    }

    const cJSON *runtime = cJSON_GetObjectItemCaseSensitive(document, "runtime");
    const cJSON *heartbeat = cJSON_GetObjectItemCaseSensitive(runtime, "heartbeatIntervalMs");
    const cJSON *offline = cJSON_GetObjectItemCaseSensitive(runtime, "offlineAfterMs");
    const cJSON *maximum = cJSON_GetObjectItemCaseSensitive(runtime, "maxMessageBytes");
    if (!cJSON_IsObject(runtime) || !cJSON_IsNumber(heartbeat) ||
        !cJSON_IsNumber(offline) || !cJSON_IsNumber(maximum) ||
        heartbeat->valuedouble <= 0.0 || maximum->valuedouble <= 0.0) {
        return false;
    }
    contract->heartbeat_interval_ms = (uint64_t)heartbeat->valuedouble;
    contract->offline_after_ms = (uint64_t)offline->valuedouble;
    contract->max_message_bytes = (uint64_t)maximum->valuedouble;

    const cJSON *transports = cJSON_GetObjectItemCaseSensitive(document, "transports");
    const cJSON *transport = NULL;
    cJSON_ArrayForEach(transport, transports) {
        const cJSON *protocol = cJSON_GetObjectItemCaseSensitive(transport, "protocol");
        if (cJSON_IsString(protocol) && strcmp(protocol->valuestring, "zenoh") == 0 &&
            copy_string(transport, "endpoint", contract->zenoh_endpoint,
                        sizeof(contract->zenoh_endpoint), true)) {
            break;
        }
    }
    if (contract->zenoh_endpoint[0] == '\0') {
        return false;
    }

    const cJSON *routes = cJSON_GetObjectItemCaseSensitive(document, "routes");
    const cJSON *route = NULL;
    cJSON_ArrayForEach(route, routes) {
        if (contract->route_count == EXTRITTIO_CONTRACT_MAX_ROUTES ||
            route->string == NULL ||
            !parse_route(route, route->string,
                         &contract->routes[contract->route_count])) {
            return false;
        }
        contract->route_count++;
    }

    const cJSON *commands = cJSON_GetObjectItemCaseSensitive(document, "commands");
    const cJSON *command = NULL;
    cJSON_ArrayForEach(command, commands) {
        if (contract->command_count == EXTRITTIO_CONTRACT_MAX_COMMANDS ||
            command->string == NULL ||
            strlen(command->string) > EXTRITTIO_CONTRACT_KEY_MAX) {
            return false;
        }
        extrittio_contract_command_t *destination =
            &contract->commands[contract->command_count];
        strcpy(destination->key, command->string);
        const cJSON *timeout = cJSON_GetObjectItemCaseSensitive(command, "timeoutMs");
        if (!copy_string(command, "requestRoute", destination->request_route,
                         sizeof(destination->request_route), true) ||
            !copy_string(command, "responseRoute", destination->response_route,
                         sizeof(destination->response_route), true) ||
            !cJSON_IsNumber(timeout) || timeout->valuedouble <= 0.0) {
            return false;
        }
        destination->timeout_ms = (uint64_t)timeout->valuedouble;
        contract->command_count++;
    }

    const cJSON *firmware = cJSON_GetObjectItemCaseSensitive(document, "firmware");
    if (cJSON_IsObject(firmware) &&
        !copy_string(firmware, "strategy", contract->firmware_strategy,
                     sizeof(contract->firmware_strategy), false)) {
        return false;
    }
    return contract->route_count > 0U;
}

static bool parse_contract(const cJSON *root, int version,
                           extrittio_contract_t *contract) {
    const cJSON *contract_json = cJSON_GetObjectItemCaseSensitive(root, "contract");
    if (!cJSON_IsObject(contract_json)) {
        return false;
    }
    const cJSON *document = contract_json;
    if (version == EXTRITTIO_BOOTSTRAP_VERSION_4) {
        if (!copy_string(contract_json, "contract_hash", contract->hash,
                         sizeof(contract->hash), true) ||
            !valid_hash(contract->hash)) {
            return false;
        }
        document = cJSON_GetObjectItemCaseSensitive(contract_json, "document");
        if (!cJSON_IsObject(document)) {
            return false;
        }
    }
    return parse_contract_document(document, contract);
}

extrittio_bootstrap_result_t extrittio_bootstrap_parse(
    const char *json,
    size_t json_length,
    extrittio_bootstrap_t *bootstrap) {
    if (json == NULL || json_length == 0U || bootstrap == NULL) {
        return EXTRITTIO_BOOTSTRAP_ERR_ARGUMENT;
    }
    memset(bootstrap, 0, sizeof(*bootstrap));
    cJSON *root = cJSON_ParseWithLength(json, json_length);
    if (!cJSON_IsObject(root)) {
        cJSON_Delete(root);
        return EXTRITTIO_BOOTSTRAP_ERR_JSON;
    }
    const cJSON *version = cJSON_GetObjectItemCaseSensitive(root, "version");
    if (!cJSON_IsNumber(version) ||
        (version->valueint != EXTRITTIO_BOOTSTRAP_VERSION_3 &&
         version->valueint != EXTRITTIO_BOOTSTRAP_VERSION_4)) {
        cJSON_Delete(root);
        return EXTRITTIO_BOOTSTRAP_ERR_VERSION;
    }
    bootstrap->version = version->valueint;

    const cJSON *backend = cJSON_GetObjectItemCaseSensitive(root, "backend");
    const cJSON *credentials = cJSON_GetObjectItemCaseSensitive(root, "credentials");
    bool valid = copy_string(root, "factory_device_id", bootstrap->factory_device_id,
                             sizeof(bootstrap->factory_device_id), true) &&
                 copy_string(root, "device_id", bootstrap->device_id,
                             sizeof(bootstrap->device_id), true) &&
                 copy_string(root, "device_name", bootstrap->device_name,
                             sizeof(bootstrap->device_name), true) &&
                 cJSON_IsObject(backend) &&
                 copy_string(backend, "address", bootstrap->backend_address,
                             sizeof(bootstrap->backend_address), true) &&
                 copy_string(backend, "api_base_url", bootstrap->api_base_url,
                             sizeof(bootstrap->api_base_url), true) &&
                 parse_network(root, bootstrap->version, &bootstrap->network) &&
                 parse_contract(root, bootstrap->version, &bootstrap->contract) &&
                 cJSON_IsObject(credentials) &&
                 copy_string(credentials, "certificate_pem",
                             bootstrap->credentials.certificate_pem,
                             sizeof(bootstrap->credentials.certificate_pem), true) &&
                 copy_string(credentials, "private_key_pem",
                             bootstrap->credentials.private_key_pem,
                             sizeof(bootstrap->credentials.private_key_pem), true) &&
                 copy_string(credentials, "ca_pem", bootstrap->credentials.ca_pem,
                             sizeof(bootstrap->credentials.ca_pem), true) &&
                 copy_string(credentials, "fingerprint",
                             bootstrap->credentials.fingerprint,
                             sizeof(bootstrap->credentials.fingerprint), true);
    cJSON_Delete(root);
    if (!valid) {
        extrittio_bootstrap_clear(bootstrap);
        return EXTRITTIO_BOOTSTRAP_ERR_REQUIRED;
    }
    if (strcmp(bootstrap->device_id, bootstrap->contract.device_id) != 0) {
        extrittio_bootstrap_clear(bootstrap);
        return EXTRITTIO_BOOTSTRAP_ERR_CONTRACT;
    }
    return EXTRITTIO_BOOTSTRAP_OK;
}

extrittio_bootstrap_result_t extrittio_bootstrap_validate_factory(
    const extrittio_bootstrap_t *bootstrap,
    const char *factory_device_id) {
    if (bootstrap == NULL || factory_device_id == NULL) {
        return EXTRITTIO_BOOTSTRAP_ERR_ARGUMENT;
    }
    return strcmp(bootstrap->factory_device_id, factory_device_id) == 0
               ? EXTRITTIO_BOOTSTRAP_OK
               : EXTRITTIO_BOOTSTRAP_ERR_IDENTITY;
}

void extrittio_bootstrap_clear(extrittio_bootstrap_t *bootstrap) {
    if (bootstrap == NULL) {
        return;
    }
    volatile unsigned char *bytes = (volatile unsigned char *)bootstrap;
    for (size_t index = 0; index < sizeof(*bootstrap); index++) {
        bytes[index] = 0U;
    }
}
