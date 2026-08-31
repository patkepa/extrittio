#include "extrittio/bootstrap.h"

#include <assert.h>
#include <string.h>

static const char bootstrap_json[] =
    "{"
    "\"version\":4,"
    "\"factory_device_id\":\"factory-001\","
    "\"device_id\":\"device-001\","
    "\"device_name\":\"Cold room\","
    "\"backend\":{\"address\":\"fd00::1\",\"api_base_url\":\"https://[fd00::1]:8080/api/v1\"},"
    "\"network\":{\"type\":\"thread\",\"active_dataset_tlvs\":\"0e080000000000010000\",\"zenoh_service\":\"_extrittio-zenoh._tcp.default.service.arpa.\"},"
    "\"contract\":{"
      "\"contract_hash\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\","
      "\"document\":{"
        "\"contractApi\":1,\"deviceId\":\"device-001\","
        "\"runtime\":{\"heartbeatIntervalMs\":30000,\"offlineAfterMs\":95000,\"maxMessageBytes\":8192},"
        "\"transports\":{\"primary\":{\"protocol\":\"zenoh\",\"endpoint\":\"tls/[fd00::1]:7447\"}},"
        "\"routes\":{"
          "\"environment\":{\"address\":\"extrittio/devices/device-001/events/environment\",\"direction\":\"device_to_cloud\",\"encoding\":\"json\"},"
          "\"command_requests\":{\"address\":\"extrittio/devices/device-001/commands/request\",\"direction\":\"cloud_to_device\",\"encoding\":\"protobuf\"},"
          "\"command_results\":{\"address\":\"extrittio/devices/device-001/commands/result\",\"direction\":\"device_to_cloud\",\"encoding\":\"protobuf\"}"
        "},"
        "\"commands\":{\"identify\":{\"requestRoute\":\"command_requests\",\"responseRoute\":\"command_results\",\"timeoutMs\":10000}},"
        "\"firmware\":{\"strategy\":\"partition_swap\"}"
      "}"
    "},"
    "\"credentials\":{\"certificate_pem\":\"CERT\",\"private_key_pem\":\"KEY\",\"ca_pem\":\"CA\",\"fingerprint\":\"fingerprint\"}"
    "}";

int main(void) {
    extrittio_bootstrap_t bootstrap;
    assert(extrittio_bootstrap_parse(bootstrap_json, strlen(bootstrap_json),
                                     &bootstrap) == EXTRITTIO_BOOTSTRAP_OK);
    assert(bootstrap.version == 4);
    assert(strcmp(bootstrap.device_id, "device-001") == 0);
    assert(bootstrap.network.type == EXTRITTIO_NETWORK_THREAD);
    assert(strcmp(bootstrap.contract.zenoh_endpoint, "tls/[fd00::1]:7447") == 0);
    assert(bootstrap.contract.route_count == 3U);
    assert(bootstrap.contract.command_count == 1U);
    assert(strcmp(bootstrap.contract.firmware_strategy, "partition_swap") == 0);
    assert(extrittio_bootstrap_validate_factory(&bootstrap, "factory-001") ==
           EXTRITTIO_BOOTSTRAP_OK);
    assert(extrittio_bootstrap_validate_factory(&bootstrap, "other") ==
           EXTRITTIO_BOOTSTRAP_ERR_IDENTITY);
    extrittio_bootstrap_clear(&bootstrap);
    assert(bootstrap.credentials.private_key_pem[0] == '\0');

    char mismatched[sizeof(bootstrap_json)];
    strcpy(mismatched, bootstrap_json);
    char *device = strstr(mismatched, "\"deviceId\":\"device-001\"");
    assert(device != NULL);
    memcpy(device + strlen("\"deviceId\":\""), "device-002", 10U);
    assert(extrittio_bootstrap_parse(mismatched, strlen(mismatched), &bootstrap) ==
           EXTRITTIO_BOOTSTRAP_ERR_CONTRACT);
    return 0;
}
