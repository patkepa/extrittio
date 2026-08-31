#include "zenoh-pico/system/link/tls.h"

#if Z_FEATURE_LINK_TLS == 1

#include <errno.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#include <mbedtls/base64.h>
#include <zephyr/net/socket.h>
#include <zephyr/net/tls_credentials.h>
#include <zephyr/posix/netdb.h>

#include "zenoh-pico/link/config/tls.h"
#include "zenoh-pico/utils/pointers.h"

#define EXTRITTIO_TLS_TAG 42

static z_result_t decode_value(const _z_str_intmap_t *config, uint8_t key,
                               unsigned char **output, size_t *length) {
    const char *encoded = _z_str_intmap_get(config, key);
    if (encoded == NULL) {
        return _Z_ERR_GENERIC;
    }

    size_t required = 0U;
    int result = mbedtls_base64_decode(NULL, 0U, &required,
                                       (const unsigned char *)encoded,
                                       strlen(encoded));
    if (result != MBEDTLS_ERR_BASE64_BUFFER_TOO_SMALL && result != 0) {
        return _Z_ERR_GENERIC;
    }
    unsigned char *decoded = z_malloc(required + 1U);
    if (decoded == NULL) {
        return _Z_ERR_SYSTEM_OUT_OF_MEMORY;
    }
    result = mbedtls_base64_decode(decoded, required, &required,
                                   (const unsigned char *)encoded,
                                   strlen(encoded));
    if (result != 0) {
        z_free(decoded);
        return _Z_ERR_GENERIC;
    }
    decoded[required] = '\0';
    *output = decoded;
    *length = required + 1U;
    return _Z_RES_OK;
}

_z_tls_context_t *_z_tls_context_new(void) {
    _z_tls_context_t *context = z_malloc(sizeof(*context));
    if (context != NULL) {
        memset(context, 0, sizeof(*context));
        context->credential_tag = EXTRITTIO_TLS_TAG;
    }
    return context;
}

void _z_tls_context_free(_z_tls_context_t **context) {
    if (context == NULL || *context == NULL) {
        return;
    }
    (void)tls_credential_delete((*context)->credential_tag,
                                TLS_CREDENTIAL_CA_CERTIFICATE);
    (void)tls_credential_delete((*context)->credential_tag,
                                TLS_CREDENTIAL_SERVER_CERTIFICATE);
    (void)tls_credential_delete((*context)->credential_tag,
                                TLS_CREDENTIAL_PRIVATE_KEY);
    z_free((*context)->private_key);
    z_free((*context)->certificate);
    z_free((*context)->ca);
    z_free(*context);
    *context = NULL;
}

static z_result_t load_credentials(_z_tls_context_t *context,
                                   const _z_str_intmap_t *config) {
    size_t ca_length = 0U;
    size_t certificate_length = 0U;
    size_t private_key_length = 0U;
    if (decode_value(config, TLS_CONFIG_ROOT_CA_CERTIFICATE_BASE64_KEY,
                     &context->ca, &ca_length) != _Z_RES_OK ||
        decode_value(config, TLS_CONFIG_CONNECT_CERTIFICATE_BASE64_KEY,
                     &context->certificate, &certificate_length) != _Z_RES_OK ||
        decode_value(config, TLS_CONFIG_CONNECT_PRIVATE_KEY_BASE64_KEY,
                     &context->private_key, &private_key_length) != _Z_RES_OK) {
        return _Z_ERR_GENERIC;
    }

    if (tls_credential_add(context->credential_tag,
                           TLS_CREDENTIAL_CA_CERTIFICATE,
                           context->ca, ca_length) != 0 ||
        tls_credential_add(context->credential_tag,
                           TLS_CREDENTIAL_SERVER_CERTIFICATE,
                           context->certificate, certificate_length) != 0 ||
        tls_credential_add(context->credential_tag,
                           TLS_CREDENTIAL_PRIVATE_KEY,
                           context->private_key, private_key_length) != 0) {
        return _Z_ERR_GENERIC;
    }
    return _Z_RES_OK;
}

z_result_t _z_open_tls(_z_tls_socket_t *sock,
                       const _z_sys_net_endpoint_t *rep,
                       const char *hostname,
                       const _z_str_intmap_t *config,
                       bool peer_socket) {
    if (sock == NULL || rep == NULL || rep->_iptcp == NULL || hostname == NULL) {
        return _Z_ERR_GENERIC;
    }
    memset(sock, 0, sizeof(*sock));
    sock->_sock._fd = -1;
    sock->_is_peer_socket = peer_socket;
    sock->_tls_ctx = _z_tls_context_new();
    if (sock->_tls_ctx == NULL ||
        load_credentials(sock->_tls_ctx, config) != _Z_RES_OK) {
        _z_tls_context_free(&sock->_tls_ctx);
        return _Z_ERR_GENERIC;
    }

    int fd = socket(rep->_iptcp->ai_family, SOCK_STREAM, IPPROTO_TLS_1_2);
    if (fd < 0) {
        _z_tls_context_free(&sock->_tls_ctx);
        return _Z_ERR_GENERIC;
    }
    sec_tag_t tags[] = {(sec_tag_t)sock->_tls_ctx->credential_tag};
    int verify = TLS_PEER_VERIFY_REQUIRED;
    if (setsockopt(fd, SOL_TLS, TLS_SEC_TAG_LIST, tags, sizeof(tags)) < 0 ||
        setsockopt(fd, SOL_TLS, TLS_PEER_VERIFY, &verify, sizeof(verify)) < 0 ||
        setsockopt(fd, SOL_TLS, TLS_HOSTNAME, hostname,
                   strlen(hostname) + 1U) < 0 ||
        connect(fd, rep->_iptcp->ai_addr, rep->_iptcp->ai_addrlen) < 0) {
        close(fd);
        _z_tls_context_free(&sock->_tls_ctx);
        return _Z_ERR_GENERIC;
    }
    sock->_sock._fd = fd;
    sock->_sock._tls_sock = sock;
    return _Z_RES_OK;
}

z_result_t _z_listen_tls(_z_tls_socket_t *sock, const char *host,
                         const char *port, const _z_str_intmap_t *config) {
    (void)sock;
    (void)host;
    (void)port;
    (void)config;
    return _Z_ERR_GENERIC;
}

z_result_t _z_tls_accept(_z_sys_net_socket_t *socket,
                         const _z_sys_net_socket_t *listen_sock) {
    (void)socket;
    (void)listen_sock;
    return _Z_ERR_GENERIC;
}

void _z_close_tls(_z_tls_socket_t *sock) {
    if (sock == NULL) {
        return;
    }
    if (sock->_sock._fd >= 0) {
        close(sock->_sock._fd);
        sock->_sock._fd = -1;
    }
    _z_tls_context_free(&sock->_tls_ctx);
}

size_t _z_read_tls(const _z_tls_socket_t *sock, uint8_t *ptr, size_t len) {
    ssize_t result = recv(sock->_sock._fd, ptr, len, 0);
    return result < 0 ? SIZE_MAX : (size_t)result;
}

size_t _z_write_tls(const _z_tls_socket_t *sock, const uint8_t *ptr,
                    size_t len) {
    ssize_t result = send(sock->_sock._fd, ptr, len, 0);
    return result < 0 ? SIZE_MAX : (size_t)result;
}

size_t _z_write_all_tls(const _z_tls_socket_t *sock, const uint8_t *ptr,
                        size_t len) {
    size_t written = 0U;
    while (written < len) {
        size_t result = _z_write_tls(sock, ptr + written, len - written);
        if (result == SIZE_MAX || result == 0U) {
            return SIZE_MAX;
        }
        written += result;
    }
    return written;
}

#endif
