#include "extrittio/ota.h"
#include <cJSON.h>
#include <string.h>
#include <ctype.h>

bool extrittio_ota_parse_from_delta(const char *delta_json,
                                     extrittio_ota_payload_t *out) {
    cJSON *root = cJSON_Parse(delta_json);
    if (!root) return false;

    cJSON *ota = cJSON_GetObjectItemCaseSensitive(root, "ota");
    if (!ota || !cJSON_IsObject(ota)) {
        cJSON_Delete(root);
        return false;
    }

    cJSON *fw_ver = cJSON_GetObjectItemCaseSensitive(ota, "firmware_version");
    cJSON *fw_url = cJSON_GetObjectItemCaseSensitive(ota, "firmware_url");

    if (!cJSON_IsString(fw_ver) || !cJSON_IsString(fw_url)
        || !fw_ver->valuestring[0] || strlen(fw_ver->valuestring) >= sizeof(out->firmware_version)
        || strlen(fw_url->valuestring) >= sizeof(out->firmware_url)
        || (strncmp(fw_url->valuestring, "https://", 8) && strncmp(fw_url->valuestring, "http://", 7))) {
        cJSON_Delete(root);
        return false;
    }

    memset(out, 0, sizeof(*out));
    strncpy(out->firmware_version, fw_ver->valuestring,
            sizeof(out->firmware_version) - 1);
    strncpy(out->firmware_url, fw_url->valuestring,
            sizeof(out->firmware_url) - 1);

    cJSON *fw_id = cJSON_GetObjectItemCaseSensitive(ota, "firmware_update_id");
    if (cJSON_IsNumber(fw_id)) {
        out->firmware_update_id = (int64_t)fw_id->valuedouble;
    }

    cJSON *sha = cJSON_GetObjectItemCaseSensitive(ota, "sha256");
    cJSON *deployment = cJSON_GetObjectItemCaseSensitive(ota, "deployment_id");
    if (!cJSON_IsNumber(deployment) || deployment->valuedouble <= 0
        || deployment->valuedouble > INT32_MAX || (double)deployment->valueint != deployment->valuedouble
        || !cJSON_IsNumber(fw_id) || fw_id->valuedouble <= 0 || fw_id->valuedouble > INT32_MAX
        || (double)fw_id->valueint != fw_id->valuedouble
        || !cJSON_IsString(sha) || strlen(sha->valuestring) != 64) {
        cJSON_Delete(root);
        return false;
    }
    for (size_t i = 0; i < 64; ++i) {
        if (!isxdigit((unsigned char)sha->valuestring[i])) { cJSON_Delete(root); return false; }
    }
    out->deployment_id = deployment->valueint;
    memcpy(out->sha256, sha->valuestring, 65);

    cJSON_Delete(root);
    return true;
}

int extrittio_ota_build_status_json(char *buf, size_t len,
                                     const char *status,
                                     const char *fw_version,
                                     int64_t fw_update_id,
                                     int64_t deployment_id,
                                     const char *error) {
    cJSON *obj = cJSON_CreateObject();
    if (!obj) return -1;

    cJSON_AddStringToObject(obj, "status", status);
    cJSON_AddStringToObject(obj, "firmware_version", fw_version);
    cJSON_AddNumberToObject(obj, "deployment_id", (double)deployment_id);

    if (fw_update_id != 0) {
        cJSON_AddNumberToObject(obj, "firmware_update_id", (double)fw_update_id);
    }
    if (error) {
        cJSON_AddStringToObject(obj, "error", error);
    }

    char *json = cJSON_PrintUnformatted(obj);
    cJSON_Delete(obj);
    if (!json) return -1;

    size_t json_len = strlen(json);
    if (json_len >= len) {
        cJSON_free(json);
        return -1;
    }

    memcpy(buf, json, json_len + 1);
    cJSON_free(json);
    return (int)json_len;
}

static int strcasecmp_portable(const char *a, const char *b) {
    while (*a && *b) {
        int ca = tolower((unsigned char)*a);
        int cb = tolower((unsigned char)*b);
        if (ca != cb) return ca - cb;
        a++;
        b++;
    }
    return tolower((unsigned char)*a) - tolower((unsigned char)*b);
}

bool extrittio_ota_is_terminal(const char *status) {
    return strcasecmp_portable(status, EXTRITTIO_OTA_SUCCESS) == 0 ||
           strcasecmp_portable(status, EXTRITTIO_OTA_FAILED) == 0;
}
