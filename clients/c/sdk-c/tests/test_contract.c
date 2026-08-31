#include "extrittio/contract.h"

#include <assert.h>
#include <string.h>

int main(void) {
    extrittio_contract_t contract = {0};
    strcpy(contract.hash,
           "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    contract.max_message_bytes = 512U;
    contract.route_count = 1U;
    strcpy(contract.routes[0].key, "environment");
    strcpy(contract.routes[0].address,
           "extrittio/devices/device-001/events/environment");
    contract.routes[0].direction = EXTRITTIO_ROUTE_DEVICE_TO_CLOUD;
    contract.routes[0].encoding = EXTRITTIO_ROUTE_ENCODING_JSON;

    char output[512];
    size_t written = 0U;
    assert(extrittio_contract_event_encode(
               &contract, "environment", "8fb19e64-a69d-49f2-9e0c-0bf07ed00e6a",
               "2026-08-30T12:00:00Z", "{\"temperature\":4.5}", output,
               sizeof(output), &written) == EXTRITTIO_CONTRACT_OK);
    assert(written == strlen(output));
    assert(strstr(output, "\"apiVersion\":1") != NULL);
    assert(strstr(output, "\"contractHash\":\"aaaaaaaa") != NULL);
    assert(strstr(output, "\"temperature\":4.5") != NULL);
    assert(extrittio_contract_event_encode(
               &contract, "missing", "id", "now", "{}", output,
               sizeof(output), &written) == EXTRITTIO_CONTRACT_ERR_ROUTE);
    return 0;
}
