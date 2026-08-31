#ifndef EXTRITTIO_ZENOH_PICO_ZEPHYR_TYPES_H
#define EXTRITTIO_ZENOH_PICO_ZEPHYR_TYPES_H

#include <pthread.h>
#include <zephyr/kernel.h>
#include <zephyr/net/socket.h>

#include "zenoh-pico/config.h"

#ifdef __cplusplus
extern "C" {
#endif

#if Z_FEATURE_MULTI_THREAD == 1
typedef pthread_t _z_task_t;
typedef pthread_attr_t z_task_attr_t;
typedef pthread_mutex_t _z_mutex_t;
typedef pthread_mutex_t _z_mutex_rec_t;
typedef pthread_cond_t _z_condvar_t;
typedef pthread_t _z_task_id_t;
#endif

typedef struct timespec z_clock_t;
typedef struct timeval z_time_t;

typedef struct {
    int _fd;
    void *_tls_sock;
} _z_sys_net_socket_t;

typedef struct {
    struct zsock_addrinfo *_iptcp;
} _z_sys_net_endpoint_t;

#ifdef __cplusplus
}
#endif

#endif
