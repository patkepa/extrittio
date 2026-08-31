#include "extrittio_zephyr/ota.h"

#include <errno.h>
#include <string.h>

#include <psa/crypto.h>
#include <zephyr/dfu/flash_img.h>
#include <zephyr/dfu/mcuboot.h>
#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>
#include <zephyr/net/http/client.h>
#include <zephyr/net/socket.h>
#include <zephyr/net/tls_credentials.h>
#include <zephyr/posix/netdb.h>
#include <zephyr/posix/sys/socket.h>
#include <zephyr/posix/unistd.h>
#include <zephyr/sys/reboot.h>

LOG_MODULE_REGISTER(extrittio_ota, CONFIG_LOG_DEFAULT_LEVEL);

#define OTA_TLS_TAG 43
#define OTA_WORK_STACK_SIZE 8192

struct ota_download_context {
    struct flash_img_context flash;
    psa_hash_operation_t sha;
    int error;
    bool body_received;
    uint8_t recv_buffer[CONFIG_EXTRITTIO_OTA_DOWNLOAD_BUFFER_SIZE];
};

static struct k_work_q ota_work_queue;
static struct k_work ota_work;
static atomic_t ota_pending;
static bool ota_queue_started;
static const extrittio_bootstrap_t *queued_bootstrap;
static extrittio_ota_payload_t queued_update;
K_THREAD_STACK_DEFINE(ota_work_stack, OTA_WORK_STACK_SIZE);

static int parse_https_url(const char *url, char *host, size_t host_capacity,
                           char *port, size_t port_capacity,
                           const char **path) {
    static const char prefix[] = "https://";
    if (url == NULL || strncmp(url, prefix, sizeof(prefix) - 1U) != 0) {
        return -EPROTONOSUPPORT;
    }

    const char *authority = url + sizeof(prefix) - 1U;
    const char *slash = strchr(authority, '/');
    size_t authority_length = slash == NULL
                                  ? strlen(authority)
                                  : (size_t)(slash - authority);
    const char *colon = memchr(authority, ':', authority_length);
    size_t host_length = colon == NULL
                             ? authority_length
                             : (size_t)(colon - authority);
    if (host_length == 0U || host_length >= host_capacity) {
        return -EINVAL;
    }
    memcpy(host, authority, host_length);
    host[host_length] = '\0';

    const char *port_value = "443";
    size_t port_length = 3U;
    if (colon != NULL) {
        port_value = colon + 1;
        port_length = authority_length - host_length - 1U;
        if (port_length == 0U || port_length >= port_capacity) {
            return -EINVAL;
        }
    }
    memcpy(port, port_value, port_length);
    port[port_length] = '\0';
    *path = slash == NULL ? "/" : slash;
    return 0;
}

static int connect_https(const char *host, const char *port) {
    struct addrinfo hints = {
        .ai_family = AF_UNSPEC,
        .ai_socktype = SOCK_STREAM,
    };
    struct addrinfo *addresses = NULL;
    int result = getaddrinfo(host, port, &hints, &addresses);
    if (result != 0) {
        return -EHOSTUNREACH;
    }

    int socket_fd = -1;
    const sec_tag_t tags[] = {OTA_TLS_TAG};
    for (struct addrinfo *address = addresses; address != NULL;
         address = address->ai_next) {
        socket_fd = socket(address->ai_family, SOCK_STREAM, IPPROTO_TLS_1_2);
        if (socket_fd < 0) {
            continue;
        }
        if (setsockopt(socket_fd, SOL_TLS, TLS_SEC_TAG_LIST, tags,
                       sizeof(tags)) == 0 &&
            setsockopt(socket_fd, SOL_TLS, TLS_HOSTNAME, host,
                       strlen(host) + 1U) == 0 &&
            connect(socket_fd, address->ai_addr, address->ai_addrlen) == 0) {
            break;
        }
        close(socket_fd);
        socket_fd = -1;
    }
    freeaddrinfo(addresses);
    return socket_fd < 0 ? -ECONNREFUSED : socket_fd;
}

static int response_callback(struct http_response *response,
                             enum http_final_call final_data,
                             void *user_data) {
    struct ota_download_context *context = user_data;
    if (response->http_status_code != 200U) {
        context->error = -EBADMSG;
        return context->error;
    }
    if (response->body_frag_len > 0U) {
        context->body_received = true;
        context->error = flash_img_buffered_write(
            &context->flash, response->body_frag_start,
            response->body_frag_len, false);
        if (context->error == 0) {
            psa_status_t status = psa_hash_update(
                &context->sha, response->body_frag_start,
                response->body_frag_len);
            context->error = status == PSA_SUCCESS ? 0 : -EIO;
        }
    }
    if (context->error == 0 && final_data == HTTP_DATA_FINAL) {
        context->error = flash_img_buffered_write(&context->flash, NULL, 0U,
                                                  true);
    }
    return context->error;
}

static int hex_nibble(char value) {
    if (value >= '0' && value <= '9') {
        return value - '0';
    }
    if (value >= 'a' && value <= 'f') {
        return value - 'a' + 10;
    }
    if (value >= 'A' && value <= 'F') {
        return value - 'A' + 10;
    }
    return -1;
}

static int decode_sha256(const char *hex, uint8_t output[32]) {
    if (hex == NULL || strlen(hex) != 64U) {
        return -EINVAL;
    }
    for (size_t index = 0; index < 32U; index++) {
        int upper = hex_nibble(hex[index * 2U]);
        int lower = hex_nibble(hex[index * 2U + 1U]);
        if (upper < 0 || lower < 0) {
            return -EINVAL;
        }
        output[index] = (uint8_t)((upper << 4) | lower);
    }
    return 0;
}

int extrittio_zephyr_ota_install(
    const extrittio_bootstrap_t *bootstrap,
    const extrittio_ota_payload_t *update) {
    if (bootstrap == NULL || update == NULL ||
        bootstrap->credentials.ca_pem[0] == '\0') {
        return -EINVAL;
    }

    char host[128];
    char port[6];
    const char *path;
    int result = parse_https_url(update->firmware_url, host, sizeof(host),
                                 port, sizeof(port), &path);
    uint8_t expected_sha[32];
    if (result == 0) {
        result = decode_sha256(update->sha256, expected_sha);
    }
    if (result != 0) {
        return result;
    }

    result = tls_credential_add(OTA_TLS_TAG, TLS_CREDENTIAL_CA_CERTIFICATE,
                                bootstrap->credentials.ca_pem,
                                strlen(bootstrap->credentials.ca_pem) + 1U);
    if (result != 0 && result != -EEXIST) {
        return result;
    }

    int socket_fd = connect_https(host, port);
    if (socket_fd < 0) {
        (void)tls_credential_delete(OTA_TLS_TAG,
                                    TLS_CREDENTIAL_CA_CERTIFICATE);
        return socket_fd;
    }

    struct ota_download_context context = {0};
    context.sha = (psa_hash_operation_t)PSA_HASH_OPERATION_INIT;
    result = psa_hash_setup(&context.sha, PSA_ALG_SHA_256) == PSA_SUCCESS
                 ? 0
                 : -EIO;
    if (result == 0) {
        result = flash_img_init(&context.flash);
    }
    if (result == 0) {
        struct http_request request = {
            .method = HTTP_GET,
            .url = path,
            .host = host,
            .protocol = "HTTP/1.1",
            .response = response_callback,
            .recv_buf = context.recv_buffer,
            .recv_buf_len = sizeof(context.recv_buffer),
        };
        result = http_client_req(socket_fd, &request, 120000, &context);
        if (result >= 0) {
            result = context.error;
        }
    }
    close(socket_fd);
    (void)tls_credential_delete(OTA_TLS_TAG, TLS_CREDENTIAL_CA_CERTIFICATE);

    uint8_t actual_sha[32];
    if (result == 0 && context.body_received) {
        size_t actual_sha_length = 0U;
        result = psa_hash_finish(&context.sha, actual_sha, sizeof(actual_sha),
                                 &actual_sha_length) == PSA_SUCCESS &&
                         actual_sha_length == sizeof(actual_sha)
                     ? 0
                     : -EIO;
    } else if (result == 0) {
        result = -ENODATA;
    }
    (void)psa_hash_abort(&context.sha);
    if (result == 0 && memcmp(expected_sha, actual_sha,
                              sizeof(expected_sha)) != 0) {
        result = -EBADMSG;
    }
    if (result == 0) {
        result = boot_request_upgrade(0);
    }
    return result;
}

static void ota_worker(struct k_work *work) {
    ARG_UNUSED(work);
    int result = extrittio_zephyr_ota_install(queued_bootstrap,
                                              &queued_update);
    atomic_clear(&ota_pending);
    if (result == 0) {
        sys_reboot(SYS_REBOOT_COLD);
    }
    LOG_ERR("Firmware update failed: %d", result);
}

int extrittio_zephyr_ota_request(
    const extrittio_bootstrap_t *bootstrap,
    const extrittio_ota_payload_t *update) {
    if (bootstrap == NULL || update == NULL) {
        return -EINVAL;
    }
    if (!atomic_cas(&ota_pending, 0, 1)) {
        return -EBUSY;
    }
    if (!ota_queue_started) {
        k_work_queue_start(&ota_work_queue, ota_work_stack,
                           K_THREAD_STACK_SIZEOF(ota_work_stack),
                           K_PRIO_PREEMPT(8), NULL);
        k_work_init(&ota_work, ota_worker);
        ota_queue_started = true;
    }
    queued_bootstrap = bootstrap;
    queued_update = *update;
    int result = k_work_submit_to_queue(&ota_work_queue, &ota_work);
    if (result < 0) {
        atomic_clear(&ota_pending);
        return result;
    }
    return 0;
}
