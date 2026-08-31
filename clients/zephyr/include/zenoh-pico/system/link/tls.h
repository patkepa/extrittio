#ifndef EXTRITTIO_ZENOH_PICO_SYSTEM_LINK_TLS_H
#define EXTRITTIO_ZENOH_PICO_SYSTEM_LINK_TLS_H

#include <stdbool.h>
#include <stdint.h>

#include "zenoh-pico/collections/string.h"
#include "zenoh-pico/system/platform.h"

#ifdef __cplusplus
extern "C" {
#endif

#if Z_FEATURE_LINK_TLS == 1
typedef struct {
    int credential_tag;
    unsigned char *ca;
    unsigned char *certificate;
    unsigned char *private_key;
} _z_tls_context_t;

typedef struct {
    _z_sys_net_socket_t _sock;
    _z_tls_context_t *_tls_ctx;
    bool _is_peer_socket;
} _z_tls_socket_t;

z_result_t _z_open_tls(_z_tls_socket_t *sock,
                       const _z_sys_net_endpoint_t *rep,
                       const char *hostname,
                       const _z_str_intmap_t *config,
                       bool peer_socket);
z_result_t _z_listen_tls(_z_tls_socket_t *sock, const char *host,
                         const char *port, const _z_str_intmap_t *config);
z_result_t _z_tls_accept(_z_sys_net_socket_t *socket,
                         const _z_sys_net_socket_t *listen_sock);
void _z_close_tls(_z_tls_socket_t *sock);
size_t _z_read_tls(const _z_tls_socket_t *sock, uint8_t *ptr, size_t len);
size_t _z_write_tls(const _z_tls_socket_t *sock, const uint8_t *ptr,
                    size_t len);
size_t _z_write_all_tls(const _z_tls_socket_t *sock, const uint8_t *ptr,
                        size_t len);

_z_tls_context_t *_z_tls_context_new(void);
void _z_tls_context_free(_z_tls_context_t **ctx);
#endif

#ifdef __cplusplus
}
#endif

#endif
