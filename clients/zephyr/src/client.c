#include "extrittio_zephyr/client.h"

#include <errno.h>
#include <stdio.h>
#include <string.h>

#include <mbedtls/base64.h>
#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>

#include <extrittio/heartbeat.h>
#include <extrittio/shadow.h>
#include <extrittio/time_utils.h>
#include <zenoh-pico.h>

#include "extrittio_zephyr/identity.h"
#include "extrittio_zephyr/network.h"

LOG_MODULE_REGISTER(extrittio_client, CONFIG_LOG_DEFAULT_LEVEL);

struct route_context {
    const char *key;
    extrittio_zephyr_route_cb callback;
    void *user_data;
};

static int base64_copy(const char *input, char **output) {
    size_t input_length = strlen(input);
    size_t capacity = ((input_length + 2U) / 3U) * 4U + 1U;
    char *encoded = k_malloc(capacity);
    if (encoded == NULL) {
        return -ENOMEM;
    }
    size_t written = 0U;
    int result = mbedtls_base64_encode((unsigned char *)encoded, capacity - 1U,
                                       &written,
                                       (const unsigned char *)input,
                                       input_length);
    if (result != 0) {
        k_free(encoded);
        return -EINVAL;
    }
    encoded[written] = '\0';
    *output = encoded;
    return 0;
}

static void route_handler(z_loaned_sample_t *sample, void *argument) {
    struct route_context *context = argument;
    z_owned_slice_t slice;
    if (z_bytes_to_slice(z_sample_payload(sample), &slice) != Z_OK) {
        return;
    }
    const z_loaned_slice_t *loaned = z_slice_loan(&slice);
    context->callback(context->key, z_slice_data(loaned), z_slice_len(loaned),
                      context->user_data);
    z_slice_drop(z_slice_move(&slice));
}

static int configure_tls(z_loaned_config_t *config,
                         const extrittio_bootstrap_t *bootstrap,
                         char **ca, char **certificate, char **private_key) {
    int result = base64_copy(bootstrap->credentials.ca_pem, ca);
    if (result == 0) {
        result = base64_copy(bootstrap->credentials.certificate_pem,
                             certificate);
    }
    if (result == 0) {
        result = base64_copy(bootstrap->credentials.private_key_pem,
                             private_key);
    }
    if (result != 0) {
        return result;
    }

    zp_config_insert(config, Z_CONFIG_TLS_ROOT_CA_CERTIFICATE_BASE64_KEY, *ca);
    zp_config_insert(config, Z_CONFIG_TLS_CONNECT_CERTIFICATE_BASE64_KEY,
                     *certificate);
    zp_config_insert(config, Z_CONFIG_TLS_CONNECT_PRIVATE_KEY_BASE64_KEY,
                     *private_key);
    zp_config_insert(config, Z_CONFIG_TLS_ENABLE_MTLS_KEY, "true");
    zp_config_insert(config, Z_CONFIG_TLS_VERIFY_NAME_ON_CONNECT_KEY, "true");
    return 0;
}

static void clear_secret(char **value) {
    if (*value != NULL) {
        memset(*value, 0, strlen(*value));
        k_free(*value);
        *value = NULL;
    }
}

static int run_session(
    const extrittio_bootstrap_t *bootstrap,
    const extrittio_zephyr_client_callbacks_t *callbacks) {
    z_owned_config_t config;
    z_config_default(&config);
    zp_config_insert(z_loan_mut(config), Z_CONFIG_CONNECT_KEY,
                     bootstrap->contract.zenoh_endpoint);

    char *ca = NULL;
    char *certificate = NULL;
    char *private_key = NULL;
    int result = configure_tls(z_loan_mut(config), bootstrap, &ca,
                               &certificate, &private_key);
    if (result != 0) {
        z_drop(z_move(config));
        goto cleanup_secrets;
    }

    z_owned_session_t session;
    result = z_open(&session, z_move(config), NULL);
    clear_secret(&private_key);
    clear_secret(&certificate);
    clear_secret(&ca);
    if (result != Z_OK) {
        return -ECONNREFUSED;
    }
    if (zp_start_read_task(z_loan_mut(session), NULL) != Z_OK ||
        zp_start_lease_task(z_loan_mut(session), NULL) != Z_OK) {
        z_drop(z_move(session));
        return -EIO;
    }

    z_owned_subscriber_t command_subscriber;
    z_owned_subscriber_t shadow_subscriber;
    bool has_command_subscriber = false;
    bool has_shadow_subscriber = false;
    if (callbacks != NULL && callbacks->command != NULL) {
        has_command_subscriber = extrittio_command_subscribe(
                                     z_loan_mut(session), bootstrap->device_id,
                                     callbacks->command, callbacks->user_data,
                                     &command_subscriber) == 0;
    }
    if (callbacks != NULL && callbacks->shadow_delta != NULL) {
        has_shadow_subscriber = extrittio_shadow_delta_subscribe(
                                    z_loan_mut(session), bootstrap->device_id,
                                    callbacks->shadow_delta,
                                    callbacks->user_data,
                                    &shadow_subscriber) == 0;
    }

    z_owned_subscriber_t route_subscribers[EXTRITTIO_CONTRACT_MAX_ROUTES];
    struct route_context route_contexts[EXTRITTIO_CONTRACT_MAX_ROUTES];
    size_t route_subscriber_count = 0U;
    if (callbacks != NULL && callbacks->route != NULL) {
        for (size_t index = 0; index < bootstrap->contract.route_count; index++) {
            const extrittio_contract_route_t *route =
                &bootstrap->contract.routes[index];
            if (route->direction != EXTRITTIO_ROUTE_CLOUD_TO_DEVICE) {
                continue;
            }
            route_contexts[route_subscriber_count] = (struct route_context){
                .key = route->key,
                .callback = callbacks->route,
                .user_data = callbacks->user_data,
            };
            z_view_keyexpr_t key_expression;
            z_view_keyexpr_from_str(&key_expression, route->address);
            z_owned_closure_sample_t closure;
            z_closure(&closure, route_handler, NULL,
                      &route_contexts[route_subscriber_count]);
            if (z_declare_subscriber(
                    z_loan_mut(session),
                    &route_subscribers[route_subscriber_count],
                    z_loan(key_expression), z_move(closure), NULL) == Z_OK) {
                route_subscriber_count++;
            }
        }
    }

    LOG_INF("Zenoh mTLS connected to %s",
            bootstrap->contract.zenoh_endpoint);
    (void)extrittio_shadow_get_publish(z_loan_mut(session),
                                       bootstrap->device_id);
    uint64_t interval = bootstrap->contract.heartbeat_interval_ms;
    if (interval == 0U) {
        interval = 30000U;
    }
    while (extrittio_network_is_ready()) {
        extrittio_heartbeat_t heartbeat = {
            .device_id = bootstrap->device_id,
            .timestamp = extrittio_now_millis(),
            .status = "online",
            .firmware = extrittio_firmware_version(),
            .uptime_seconds = k_uptime_get() / 1000,
        };
        if (extrittio_heartbeat_publish(z_loan_mut(session), &heartbeat) != 0) {
            result = -EIO;
            break;
        }
        k_sleep(K_MSEC(interval));
    }

    for (size_t index = 0; index < route_subscriber_count; index++) {
        z_drop(z_move(route_subscribers[index]));
    }
    if (has_shadow_subscriber) {
        z_drop(z_move(shadow_subscriber));
    }
    if (has_command_subscriber) {
        z_drop(z_move(command_subscriber));
    }
    z_drop(z_move(session));
    return result;

cleanup_secrets:
    clear_secret(&private_key);
    clear_secret(&certificate);
    clear_secret(&ca);
    return result;
}

int extrittio_zephyr_client_run(
    const extrittio_bootstrap_t *bootstrap,
    const extrittio_zephyr_client_callbacks_t *callbacks) {
    if (bootstrap == NULL ||
        strncmp(bootstrap->contract.zenoh_endpoint, "tls/", 4U) != 0) {
        LOG_ERR("A tls/ Zenoh endpoint is required");
        return -EINVAL;
    }
    while (true) {
        int result = run_session(bootstrap, callbacks);
        LOG_WRN("Zenoh session ended (%d); retrying", result);
        k_sleep(K_SECONDS(CONFIG_EXTRITTIO_ZENOH_RETRY_SECONDS));
    }
}
