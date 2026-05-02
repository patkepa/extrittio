#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>

#include "esp_log.h"
#include "esp_mac.h"
#include "esp_netif.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "esp_wifi.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "lwip/inet.h"
#include "lwip/ip_addr.h"
#include "lwip/sockets.h"
#include "nvs_flash.h"
#include "sdkconfig.h"

#if __has_include("esp_netif_net_stack.h")
#include "esp_netif_net_stack.h"
#include "lwip/etharp.h"
#include "lwip/prot/ethernet.h"
#define EXTRITTIO_HAVE_ARP_LOOKUP 1
#else
#define EXTRITTIO_HAVE_ARP_LOOKUP 0
#endif

#include "wifi.h"

#include "extrittio/extrittio.h"
#include <zenoh-pico.h>

static const char *TAG = "network_analyzer";

typedef struct {
    uint32_t addr;
    char ip[16];
    char mac[18];
    char hostname[32];
    const char *vendor;
    const char *device_type;
    const char *classification;
    uint32_t rtt_ms;
    const char *source;
} host_record_t;

typedef struct {
    uint32_t scan_id;
    uint32_t targets_scanned;
    uint32_t host_count;
    bool target_limit_reached;
    bool host_limit_reached;
    host_record_t hosts[CONFIG_EXTRITTIO_ANALYZER_MAX_HOSTS];
} scan_result_t;

typedef struct {
    char *buf;
    size_t cap;
    size_t len;
    bool failed;
} json_writer_t;

static bool jw_append(json_writer_t *w, const char *fmt, ...) {
    if (w->failed || w->len >= w->cap) {
        return false;
    }

    va_list args;
    va_start(args, fmt);
    int written = vsnprintf(w->buf + w->len, w->cap - w->len, fmt, args);
    va_end(args);

    if (written < 0 || (size_t)written >= w->cap - w->len) {
        w->failed = true;
        if (w->cap > 0) {
            w->buf[w->cap - 1] = '\0';
        }
        return false;
    }

    w->len += (size_t)written;
    return true;
}

static bool jw_append_escaped(json_writer_t *w, const char *value) {
    if (!jw_append(w, "\"")) {
        return false;
    }

    if (value == NULL) {
        value = "";
    }

    for (const char *p = value; *p != '\0'; p++) {
        switch (*p) {
        case '"':
            if (!jw_append(w, "\\\"")) return false;
            break;
        case '\\':
            if (!jw_append(w, "\\\\")) return false;
            break;
        case '\n':
            if (!jw_append(w, "\\n")) return false;
            break;
        case '\r':
            if (!jw_append(w, "\\r")) return false;
            break;
        case '\t':
            if (!jw_append(w, "\\t")) return false;
            break;
        default:
            if ((unsigned char)*p < 0x20) {
                if (!jw_append(w, "\\u%04x", (unsigned char)*p)) return false;
            } else {
                if (!jw_append(w, "%c", *p)) return false;
            }
            break;
        }
    }

    return jw_append(w, "\"");
}

static void ipv4_to_string(uint32_t addr, char *out, size_t len) {
    snprintf(out, len, "%u.%u.%u.%u",
             (unsigned int)((addr >> 24) & 0xff),
             (unsigned int)((addr >> 16) & 0xff),
             (unsigned int)((addr >> 8) & 0xff),
             (unsigned int)(addr & 0xff));
}

static void mac_to_string(const uint8_t mac[6], char *out, size_t len) {
    snprintf(out, len, "%02x:%02x:%02x:%02x:%02x:%02x",
             mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
}

static const char *authmode_to_string(wifi_auth_mode_t mode) {
    switch (mode) {
    case WIFI_AUTH_OPEN:
        return "open";
    case WIFI_AUTH_WEP:
        return "wep";
    case WIFI_AUTH_WPA_PSK:
        return "wpa_psk";
    case WIFI_AUTH_WPA2_PSK:
        return "wpa2_psk";
    case WIFI_AUTH_WPA_WPA2_PSK:
        return "wpa_wpa2_psk";
    case WIFI_AUTH_WPA2_ENTERPRISE:
        return "wpa2_enterprise";
    case WIFI_AUTH_WPA3_PSK:
        return "wpa3_psk";
    case WIFI_AUTH_WPA2_WPA3_PSK:
        return "wpa2_wpa3_psk";
    default:
        return "unknown";
    }
}

static uint8_t hex_nibble(char c) {
    if (c >= '0' && c <= '9') return (uint8_t)(c - '0');
    if (c >= 'a' && c <= 'f') return (uint8_t)(c - 'a' + 10);
    if (c >= 'A' && c <= 'F') return (uint8_t)(c - 'A' + 10);
    return 0;
}

static bool parse_mac(const char *mac, uint8_t out[6]) {
    if (mac == NULL || strlen(mac) < 17) {
        return false;
    }
    for (size_t i = 0; i < 6; i++) {
        size_t pos = i * 3;
        out[i] = (uint8_t)((hex_nibble(mac[pos]) << 4) | hex_nibble(mac[pos + 1]));
    }
    return true;
}

static bool oui_is(const uint8_t mac[6], uint8_t a, uint8_t b, uint8_t c) {
    return mac[0] == a && mac[1] == b && mac[2] == c;
}

static const char *vendor_from_oui(const uint8_t mac[6]) {
    if ((mac[0] & 0x02) != 0) {
        return "Private/Randomized";
    }

    if (oui_is(mac, 0x00, 0x03, 0x93) || oui_is(mac, 0x00, 0x05, 0x02) ||
        oui_is(mac, 0x00, 0x0a, 0x27) || oui_is(mac, 0x00, 0x0a, 0x95) ||
        oui_is(mac, 0x00, 0x0d, 0x93) || oui_is(mac, 0x00, 0x11, 0x24) ||
        oui_is(mac, 0x00, 0x14, 0x51) || oui_is(mac, 0x00, 0x16, 0xcb) ||
        oui_is(mac, 0x00, 0x17, 0xf2) || oui_is(mac, 0x00, 0x19, 0xe3) ||
        oui_is(mac, 0x00, 0x1b, 0x63) || oui_is(mac, 0x00, 0x1c, 0xb3) ||
        oui_is(mac, 0x00, 0x1d, 0x4f) || oui_is(mac, 0x00, 0x1e, 0x52) ||
        oui_is(mac, 0x00, 0x1e, 0xc2) || oui_is(mac, 0x00, 0x1f, 0x5b) ||
        oui_is(mac, 0x00, 0x21, 0xe9) || oui_is(mac, 0x00, 0x22, 0x41) ||
        oui_is(mac, 0x00, 0x23, 0x12) || oui_is(mac, 0x00, 0x23, 0x32) ||
        oui_is(mac, 0x00, 0x23, 0x6c) || oui_is(mac, 0x00, 0x23, 0xdf) ||
        oui_is(mac, 0x00, 0x24, 0x36) || oui_is(mac, 0x00, 0x25, 0x00) ||
        oui_is(mac, 0x00, 0x25, 0x4b) || oui_is(mac, 0x00, 0x25, 0xbc) ||
        oui_is(mac, 0x00, 0x26, 0x08) || oui_is(mac, 0x00, 0x26, 0x4a) ||
        oui_is(mac, 0x00, 0x26, 0xb0) || oui_is(mac, 0x00, 0x26, 0xbb) ||
        oui_is(mac, 0x04, 0x0c, 0xce) || oui_is(mac, 0x04, 0x1e, 0x64) ||
        oui_is(mac, 0x04, 0x26, 0x65) || oui_is(mac, 0x04, 0x4b, 0xed) ||
        oui_is(mac, 0x04, 0x52, 0xf3) || oui_is(mac, 0x04, 0x54, 0x53) ||
        oui_is(mac, 0x04, 0x69, 0xf8) || oui_is(mac, 0x04, 0xdb, 0x56) ||
        oui_is(mac, 0x08, 0x00, 0x07) || oui_is(mac, 0x08, 0x66, 0x98) ||
        oui_is(mac, 0x0c, 0x30, 0x21) || oui_is(mac, 0x0c, 0x3e, 0x9f) ||
        oui_is(mac, 0x10, 0x40, 0xf3) || oui_is(mac, 0x10, 0x93, 0xe9) ||
        oui_is(mac, 0x14, 0x10, 0x9f) || oui_is(mac, 0x14, 0x20, 0x5e) ||
        oui_is(mac, 0x14, 0x5a, 0x05) || oui_is(mac, 0x14, 0x7d, 0xda) ||
        oui_is(mac, 0x18, 0x34, 0x51) || oui_is(mac, 0x18, 0x65, 0x90) ||
        oui_is(mac, 0x18, 0xaf, 0x61) || oui_is(mac, 0x1c, 0xab, 0xa7) ||
        oui_is(mac, 0x20, 0x3c, 0xae) || oui_is(mac, 0x24, 0xa0, 0x74) ||
        oui_is(mac, 0x28, 0xcf, 0xda) || oui_is(mac, 0x28, 0xe0, 0x2c) ||
        oui_is(mac, 0x2c, 0xbe, 0x08) || oui_is(mac, 0x30, 0x10, 0xe4) ||
        oui_is(mac, 0x34, 0x15, 0x9e) || oui_is(mac, 0x34, 0x36, 0x3b) ||
        oui_is(mac, 0x38, 0x48, 0x4c) || oui_is(mac, 0x3c, 0x07, 0x54) ||
        oui_is(mac, 0x3c, 0x15, 0xc2) || oui_is(mac, 0x40, 0x30, 0x04) ||
        oui_is(mac, 0x40, 0x6c, 0x8f) || oui_is(mac, 0x40, 0xa6, 0xd9) ||
        oui_is(mac, 0x40, 0xb3, 0x95) || oui_is(mac, 0x40, 0xcb, 0xc0) ||
        oui_is(mac, 0x40, 0xd3, 0x2d) || oui_is(mac, 0x40, 0xed, 0xcf) ||
        oui_is(mac, 0x44, 0x4c, 0x0c) || oui_is(mac, 0x48, 0x43, 0x7c) ||
        oui_is(mac, 0x48, 0x60, 0xbc) || oui_is(mac, 0x48, 0xa9, 0x1c) ||
        oui_is(mac, 0x4c, 0x57, 0xca) || oui_is(mac, 0x4c, 0x74, 0xbf) ||
        oui_is(mac, 0x50, 0xed, 0x3c) || oui_is(mac, 0x54, 0x26, 0x96) ||
        oui_is(mac, 0x58, 0x1f, 0xaa) || oui_is(mac, 0x58, 0x55, 0xca) ||
        oui_is(mac, 0x5c, 0x59, 0x48) || oui_is(mac, 0x5c, 0x8d, 0x4e) ||
        oui_is(mac, 0x60, 0x33, 0x4b) || oui_is(mac, 0x60, 0xf8, 0x1d) ||
        oui_is(mac, 0x64, 0xa3, 0xcb) || oui_is(mac, 0x68, 0x96, 0x7b) ||
        oui_is(mac, 0x68, 0xab, 0x1e) || oui_is(mac, 0x6c, 0x40, 0x08) ||
        oui_is(mac, 0x70, 0x3e, 0xac) || oui_is(mac, 0x70, 0xcd, 0x60) ||
        oui_is(mac, 0x74, 0x81, 0x14) || oui_is(mac, 0x78, 0x31, 0xc1) ||
        oui_is(mac, 0x78, 0x4f, 0x43) || oui_is(mac, 0x7c, 0x6d, 0x62) ||
        oui_is(mac, 0x7c, 0xc3, 0xa1) || oui_is(mac, 0x80, 0xbe, 0x05) ||
        oui_is(mac, 0x84, 0x29, 0x99) || oui_is(mac, 0x88, 0x1f, 0xa1) ||
        oui_is(mac, 0x8c, 0x29, 0x37) || oui_is(mac, 0x8c, 0x85, 0x90) ||
        oui_is(mac, 0x90, 0x27, 0xe4) || oui_is(mac, 0x90, 0x72, 0x40) ||
        oui_is(mac, 0x94, 0xe9, 0x79) || oui_is(mac, 0x98, 0x01, 0xa7) ||
        oui_is(mac, 0x98, 0x5a, 0xeb) || oui_is(mac, 0x98, 0xe0, 0xd9) ||
        oui_is(mac, 0x9c, 0x04, 0xeb) || oui_is(mac, 0x9c, 0x20, 0x7b) ||
        oui_is(mac, 0xa0, 0x99, 0x9b) || oui_is(mac, 0xa4, 0x83, 0xe7) ||
        oui_is(mac, 0xa8, 0x20, 0x66) || oui_is(mac, 0xac, 0xbc, 0x32) ||
        oui_is(mac, 0xb0, 0x34, 0x95) || oui_is(mac, 0xb8, 0x09, 0x8a) ||
        oui_is(mac, 0xbc, 0x52, 0xb7) || oui_is(mac, 0xc0, 0x84, 0x7a) ||
        oui_is(mac, 0xc0, 0xf2, 0xfb) || oui_is(mac, 0xc8, 0x69, 0xcd) ||
        oui_is(mac, 0xcc, 0x08, 0xe0) || oui_is(mac, 0xd0, 0x23, 0xdb) ||
        oui_is(mac, 0xd4, 0x61, 0x9d) || oui_is(mac, 0xd8, 0x30, 0x62) ||
        oui_is(mac, 0xdc, 0x2b, 0x2a) || oui_is(mac, 0xe0, 0xac, 0xcb) ||
        oui_is(mac, 0xe4, 0x25, 0xe7) || oui_is(mac, 0xe8, 0x06, 0x88) ||
        oui_is(mac, 0xec, 0x35, 0x86) || oui_is(mac, 0xf0, 0x18, 0x98) ||
        oui_is(mac, 0xf4, 0x0f, 0x24) || oui_is(mac, 0xf8, 0x27, 0x93) ||
        oui_is(mac, 0xfc, 0xfc, 0x48)) {
        return "Apple";
    }

    if (oui_is(mac, 0x00, 0x1a, 0x11) || oui_is(mac, 0x04, 0x18, 0xd6) ||
        oui_is(mac, 0x08, 0xd4, 0x0c) || oui_is(mac, 0x10, 0x0d, 0x7f) ||
        oui_is(mac, 0x20, 0x3d, 0xbd) || oui_is(mac, 0x28, 0x6c, 0x07) ||
        oui_is(mac, 0x38, 0x2d, 0xd1) || oui_is(mac, 0x50, 0xf5, 0xda) ||
        oui_is(mac, 0x5c, 0xf6, 0xdc) || oui_is(mac, 0x64, 0xbc, 0x0c) ||
        oui_is(mac, 0x78, 0x1f, 0xdb) || oui_is(mac, 0x84, 0x11, 0x9e) ||
        oui_is(mac, 0x90, 0xb6, 0x86) || oui_is(mac, 0xa0, 0x21, 0xb7) ||
        oui_is(mac, 0xbc, 0x14, 0x85) || oui_is(mac, 0xcc, 0x6e, 0xa4) ||
        oui_is(mac, 0xdc, 0x71, 0x44) || oui_is(mac, 0xec, 0x1f, 0x72)) {
        return "Samsung";
    }

    if (oui_is(mac, 0x3c, 0x5a, 0xb4) || oui_is(mac, 0x54, 0x60, 0x09) ||
        oui_is(mac, 0x64, 0x16, 0x66) || oui_is(mac, 0x6c, 0xad, 0xf8) ||
        oui_is(mac, 0x70, 0x3a, 0xcb) || oui_is(mac, 0x74, 0xda, 0x38) ||
        oui_is(mac, 0xa4, 0x77, 0x33) || oui_is(mac, 0xac, 0x37, 0x43) ||
        oui_is(mac, 0xf4, 0xf5, 0xd8)) {
        return "Google";
    }

    if (oui_is(mac, 0x00, 0x1a, 0xa0) || oui_is(mac, 0x04, 0x18, 0xb6) ||
        oui_is(mac, 0x24, 0x5a, 0x4c) || oui_is(mac, 0x44, 0xd9, 0xe7) ||
        oui_is(mac, 0x68, 0x7f, 0xf0) || oui_is(mac, 0x74, 0xac, 0xb9) ||
        oui_is(mac, 0x78, 0x8a, 0x20) || oui_is(mac, 0x80, 0x2a, 0xa8) ||
        oui_is(mac, 0xb0, 0x95, 0x75) || oui_is(mac, 0xd4, 0x6e, 0x0e)) {
        return "Router/Network Gear";
    }

    if (oui_is(mac, 0x44, 0x17, 0x93) || oui_is(mac, 0x48, 0x55, 0x19) ||
        oui_is(mac, 0x50, 0xc7, 0xbf) || oui_is(mac, 0x84, 0xf3, 0xeb) ||
        oui_is(mac, 0xb8, 0x27, 0xeb) || oui_is(mac, 0xdc, 0xa6, 0x32) ||
        oui_is(mac, 0xe4, 0x5f, 0x01)) {
        return "Raspberry Pi";
    }

    if (oui_is(mac, 0x18, 0xfe, 0x34) || oui_is(mac, 0x24, 0x0a, 0xc4) ||
        oui_is(mac, 0x30, 0xae, 0xa4) || oui_is(mac, 0x7c, 0x9e, 0xbd) ||
        oui_is(mac, 0x84, 0x0d, 0x8e) || oui_is(mac, 0x98, 0xf4, 0xab) ||
        oui_is(mac, 0xa0, 0xb7, 0x65) || oui_is(mac, 0xc4, 0xdd, 0x57)) {
        return "Espressif";
    }

    if (oui_is(mac, 0x00, 0x09, 0xbf) || oui_is(mac, 0x00, 0x17, 0xab) ||
        oui_is(mac, 0x00, 0x19, 0x1d) || oui_is(mac, 0x00, 0x1a, 0xe9) ||
        oui_is(mac, 0x00, 0x1b, 0xea) || oui_is(mac, 0x00, 0x1c, 0xbe) ||
        oui_is(mac, 0x00, 0x1e, 0x35) || oui_is(mac, 0x00, 0x1f, 0x32) ||
        oui_is(mac, 0x00, 0x21, 0x47) || oui_is(mac, 0x00, 0x22, 0xaa) ||
        oui_is(mac, 0x00, 0x23, 0xcc) || oui_is(mac, 0x00, 0x24, 0x44) ||
        oui_is(mac, 0x00, 0x24, 0xf3) || oui_is(mac, 0x00, 0x25, 0xa0) ||
        oui_is(mac, 0x00, 0x2b, 0xd1) || oui_is(mac, 0x04, 0x03, 0xd6) ||
        oui_is(mac, 0x10, 0x27, 0xf5) || oui_is(mac, 0x40, 0xf4, 0x07) ||
        oui_is(mac, 0x58, 0xbd, 0xa3) || oui_is(mac, 0x78, 0xa2, 0xa0) ||
        oui_is(mac, 0x7c, 0xbb, 0x8a) || oui_is(mac, 0x98, 0xb6, 0xe9) ||
        oui_is(mac, 0xb8, 0xae, 0x6e) || oui_is(mac, 0xd8, 0x6b, 0xf7) ||
        oui_is(mac, 0xe0, 0xe7, 0x51)) {
        return "Nintendo";
    }

    if (oui_is(mac, 0x18, 0x74, 0x2e) || oui_is(mac, 0x1c, 0x12, 0xb0) ||
        oui_is(mac, 0x34, 0xd2, 0x70) || oui_is(mac, 0x44, 0x65, 0x0d) ||
        oui_is(mac, 0x68, 0x37, 0xe9) || oui_is(mac, 0x74, 0x75, 0x48) ||
        oui_is(mac, 0x84, 0xd6, 0xd0) || oui_is(mac, 0xa4, 0x08, 0xf5) ||
        oui_is(mac, 0xbc, 0xff, 0x4d) || oui_is(mac, 0xcc, 0x9e, 0xa2) ||
        oui_is(mac, 0xf0, 0x27, 0x2d) || oui_is(mac, 0xf0, 0x81, 0x73) ||
        oui_is(mac, 0xfc, 0xa1, 0x83)) {
        return "Amazon";
    }

    if (oui_is(mac, 0x00, 0x04, 0x1f) || oui_is(mac, 0x00, 0x15, 0xc1) ||
        oui_is(mac, 0x00, 0x19, 0xc5) || oui_is(mac, 0x00, 0x1a, 0x8a) ||
        oui_is(mac, 0x00, 0x1d, 0x0d) || oui_is(mac, 0x00, 0x24, 0x8d) ||
        oui_is(mac, 0x00, 0xd9, 0xd1) || oui_is(mac, 0x10, 0xb3, 0xd5) ||
        oui_is(mac, 0x28, 0x37, 0x37) || oui_is(mac, 0x70, 0x9e, 0x29) ||
        oui_is(mac, 0xa8, 0xe3, 0xee) || oui_is(mac, 0xbc, 0x60, 0xa7) ||
        oui_is(mac, 0xf0, 0xb0, 0x52) || oui_is(mac, 0xf8, 0x46, 0x1c)) {
        return "Sony";
    }

    if (oui_is(mac, 0x00, 0x17, 0xfa) || oui_is(mac, 0x00, 0x50, 0xf2) ||
        oui_is(mac, 0x28, 0x18, 0x78) || oui_is(mac, 0x30, 0x59, 0xb7) ||
        oui_is(mac, 0x60, 0x45, 0xcb) || oui_is(mac, 0x7c, 0xed, 0x8d) ||
        oui_is(mac, 0x98, 0x5f, 0xd3) || oui_is(mac, 0xd8, 0xb4, 0x2a)) {
        return "Microsoft";
    }

    if (oui_is(mac, 0x00, 0x0d, 0x4b) || oui_is(mac, 0x08, 0x05, 0x81) ||
        oui_is(mac, 0xb0, 0xa7, 0x37) || oui_is(mac, 0xcc, 0x6d, 0xa0) ||
        oui_is(mac, 0xd4, 0xe2, 0x2e)) {
        return "Roku";
    }

    if (oui_is(mac, 0x00, 0x19, 0xa1) || oui_is(mac, 0x00, 0x1e, 0x75) ||
        oui_is(mac, 0x18, 0x3d, 0xa2) || oui_is(mac, 0x1c, 0x08, 0xc7) ||
        oui_is(mac, 0x28, 0xb2, 0xbd) || oui_is(mac, 0x30, 0xcd, 0xa7) ||
        oui_is(mac, 0x3c, 0xbd, 0x3e) || oui_is(mac, 0x88, 0xc9, 0xd0) ||
        oui_is(mac, 0x9c, 0x3a, 0xaf) || oui_is(mac, 0xa8, 0x23, 0xfe)) {
        return "LG";
    }

    if (oui_is(mac, 0x00, 0x9e, 0xc8) || oui_is(mac, 0x10, 0x2a, 0xb3) ||
        oui_is(mac, 0x18, 0x59, 0x36) || oui_is(mac, 0x34, 0xce, 0x00) ||
        oui_is(mac, 0x38, 0xa4, 0xed) || oui_is(mac, 0x50, 0x64, 0x2b) ||
        oui_is(mac, 0x58, 0x44, 0x98) || oui_is(mac, 0x64, 0x09, 0x80) ||
        oui_is(mac, 0x74, 0x23, 0x44) || oui_is(mac, 0x8c, 0xbe, 0xbe) ||
        oui_is(mac, 0xac, 0xc1, 0xee) || oui_is(mac, 0xf4, 0x8e, 0x38) ||
        oui_is(mac, 0xfc, 0x64, 0xba)) {
        return "Xiaomi";
    }

    if (oui_is(mac, 0x00, 0xe0, 0xfc) || oui_is(mac, 0x04, 0xbd, 0x70) ||
        oui_is(mac, 0x1c, 0x8e, 0x5c) || oui_is(mac, 0x24, 0xdf, 0x6a) ||
        oui_is(mac, 0x30, 0x74, 0x96) || oui_is(mac, 0x38, 0x37, 0x8b) ||
        oui_is(mac, 0x48, 0x00, 0x31) || oui_is(mac, 0x54, 0x51, 0x1b) ||
        oui_is(mac, 0x5c, 0xc3, 0x07) || oui_is(mac, 0x78, 0x1d, 0xba)) {
        return "Huawei";
    }

    if (oui_is(mac, 0x10, 0xfe, 0xed) || oui_is(mac, 0x14, 0xcc, 0x20) ||
        oui_is(mac, 0x18, 0xa6, 0xf7) || oui_is(mac, 0x30, 0xb5, 0xc2) ||
        oui_is(mac, 0x50, 0xc7, 0xbf) || oui_is(mac, 0x54, 0xaf, 0x97) ||
        oui_is(mac, 0x64, 0x70, 0x02) || oui_is(mac, 0x9c, 0xa6, 0x15) ||
        oui_is(mac, 0xc4, 0x6e, 0x1f) || oui_is(mac, 0xec, 0x08, 0x6b)) {
        return "TP-Link";
    }

    if (oui_is(mac, 0x00, 0x14, 0x6c) || oui_is(mac, 0x20, 0x4e, 0x7f) ||
        oui_is(mac, 0x28, 0xc6, 0x8e) || oui_is(mac, 0x2c, 0xb0, 0x5d) ||
        oui_is(mac, 0x44, 0x94, 0xfc) || oui_is(mac, 0x84, 0x1b, 0x5e) ||
        oui_is(mac, 0xc0, 0x3f, 0x0e)) {
        return "Netgear";
    }

    return "Unknown";
}

static void classify_host(host_record_t *host, uint32_t host_addr, uint32_t gateway_addr) {
    uint8_t mac[6] = {0};
    host->vendor = "Unknown";
    host->device_type = "unknown";
    host->classification = "unknown";

    if (!parse_mac(host->mac, mac)) {
        return;
    }

    host->vendor = vendor_from_oui(mac);
    bool local_admin = (mac[0] & 0x02) != 0;

    if (host_addr == gateway_addr) {
        host->device_type = "router";
        host->classification = "network_gateway";
    } else if (local_admin) {
        host->device_type = "private_wifi_device";
        host->classification = "randomized_mac";
    } else if (strcmp(host->vendor, "Apple") == 0) {
        host->device_type = "apple_device";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "Samsung") == 0) {
        host->device_type = "samsung_device";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "Google") == 0) {
        host->device_type = "google_device";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "Raspberry Pi") == 0) {
        host->device_type = "raspberry_pi";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "Espressif") == 0) {
        host->device_type = "esp32";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "Router/Network Gear") == 0) {
        host->device_type = "network_device";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "Nintendo") == 0) {
        host->device_type = "nintendo_switch";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "Amazon") == 0) {
        host->device_type = "amazon_device";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "Sony") == 0) {
        host->device_type = "sony_playstation";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "Microsoft") == 0) {
        host->device_type = "xbox_or_surface";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "Roku") == 0) {
        host->device_type = "roku";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "LG") == 0) {
        host->device_type = "lg_device";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "Xiaomi") == 0) {
        host->device_type = "xiaomi_device";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "Huawei") == 0) {
        host->device_type = "huawei_device";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "TP-Link") == 0) {
        host->device_type = "tp_link_device";
        host->classification = "vendor_oui";
    } else if (strcmp(host->vendor, "Netgear") == 0) {
        host->device_type = "netgear_device";
        host->classification = "vendor_oui";
    }
}

static int classification_priority(const char *classification) {
    if (classification == NULL) return 0;
    if (strcmp(classification, "network_gateway") == 0) return 100;
    if (strcmp(classification, "mdns_model") == 0) return 90;
    if (strcmp(classification, "mdns_service") == 0) return 80;
    if (strcmp(classification, "tcp_service") == 0) return 70;
    if (strcmp(classification, "netbios_name") == 0) return 60;
    if (strcmp(classification, "ssdp_service") == 0) return 50;
    if (strcmp(classification, "vendor_oui") == 0) return 30;
    if (strcmp(classification, "randomized_mac") == 0) return 20;
    return 0;
}

static void refine_host_identity(host_record_t *host, const char *vendor,
                                 const char *device_type,
                                 const char *classification) {
    int current = classification_priority(host->classification);
    int incoming = classification_priority(classification);

    if (current == 100 && incoming < 100) {
        return;
    }
    if (incoming < current) {
        return;
    }

    if (vendor != NULL &&
        (strcmp(host->vendor, "Unknown") == 0 ||
         strcmp(host->vendor, "Private/Randomized") == 0 ||
         incoming > current)) {
        host->vendor = vendor;
    }
    if (device_type != NULL) {
        host->device_type = device_type;
    }
    if (classification != NULL) {
        host->classification = classification;
    }
}

static char ascii_lower_char(char c) {
    if (c >= 'A' && c <= 'Z') {
        return (char)(c + ('a' - 'A'));
    }
    return c;
}

static bool bytes_contains_ci(const uint8_t *data, size_t data_len, const char *needle) {
    size_t needle_len = strlen(needle);
    if (needle_len == 0 || data_len < needle_len) {
        return false;
    }

    for (size_t i = 0; i <= data_len - needle_len; i++) {
        size_t j = 0;
        while (j < needle_len &&
               ascii_lower_char((char)data[i + j]) == ascii_lower_char(needle[j])) {
            j++;
        }
        if (j == needle_len) {
            return true;
        }
    }
    return false;
}

static host_record_t *find_host_by_addr(scan_result_t *scan, uint32_t addr) {
    for (uint32_t i = 0; i < scan->host_count; i++) {
        if (scan->hosts[i].addr == addr) {
            return &scan->hosts[i];
        }
    }
    return NULL;
}

static void encode_netbios_name(const char *name, uint8_t out[34]) {
    char padded[16];
    memset(padded, ' ', sizeof(padded));
    size_t name_len = strlen(name);
    if (name_len > 15) {
        name_len = 15;
    }
    memcpy(padded, name, name_len);
    padded[15] = '\0';

    out[0] = 32;
    for (size_t i = 0; i < sizeof(padded); i++) {
        out[1 + i * 2] = (uint8_t)('A' + ((padded[i] >> 4) & 0x0f));
        out[2 + i * 2] = (uint8_t)('A' + (padded[i] & 0x0f));
    }
    out[33] = 0;
}

static bool valid_hostname_char(char c) {
    return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') ||
           (c >= '0' && c <= '9') || c == '-' || c == '_';
}

static bool nbns_probe_hostname(uint32_t host_addr, char *hostname, size_t hostname_len) {
    if (hostname_len == 0) {
        return false;
    }
    hostname[0] = '\0';

    int sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_IP);
    if (sock < 0) {
        return false;
    }

    struct timeval timeout = {
        .tv_sec = 0,
        .tv_usec = 180000,
    };
    setsockopt(sock, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout));

    struct sockaddr_in dest = {
        .sin_family = AF_INET,
        .sin_port = htons(137),
        .sin_addr.s_addr = htonl(host_addr),
    };

    uint8_t packet[50] = {0};
    packet[0] = 0x45;
    packet[1] = 0x58;
    packet[4] = 0x00;
    packet[5] = 0x01;
    encode_netbios_name("*", &packet[12]);
    packet[46] = 0x00;
    packet[47] = 0x21;
    packet[48] = 0x00;
    packet[49] = 0x01;

    if (sendto(sock, packet, sizeof(packet), 0, (struct sockaddr *)&dest, sizeof(dest)) < 0) {
        close(sock);
        return false;
    }

    uint8_t response[384];
    ssize_t received = recvfrom(sock, response, sizeof(response), 0, NULL, NULL);
    close(sock);
    if (received < 70) {
        return false;
    }

    size_t pos = 12;
    while (pos < (size_t)received && response[pos] != 0) {
        pos += (size_t)response[pos] + 1;
    }
    if (pos + 11 >= (size_t)received) {
        return false;
    }

    pos += 11;
    if (pos >= (size_t)received) {
        return false;
    }

    uint8_t name_count = response[pos++];
    for (uint8_t i = 0; i < name_count && pos + 18 <= (size_t)received; i++, pos += 18) {
        const uint8_t *name = &response[pos];
        uint8_t suffix = name[15];
        uint16_t flags = ((uint16_t)response[pos + 16] << 8) | response[pos + 17];
        bool group_name = (flags & 0x8000) != 0;
        if (suffix != 0x00 || group_name) {
            continue;
        }

        size_t out_len = 0;
        for (size_t j = 0; j < 15 && out_len + 1 < hostname_len; j++) {
            char c = (char)name[j];
            if (c == ' ') {
                break;
            }
            if (!valid_hostname_char(c)) {
                out_len = 0;
                break;
            }
            hostname[out_len++] = c;
        }
        hostname[out_len] = '\0';
        return out_len > 0;
    }

    return false;
}

static bool tcp_port_open(uint32_t host_addr, uint16_t port, uint32_t timeout_ms) {
    int sock = socket(AF_INET, SOCK_STREAM, IPPROTO_IP);
    if (sock < 0) {
        return false;
    }

    int flags = fcntl(sock, F_GETFL, 0);
    if (flags >= 0) {
        fcntl(sock, F_SETFL, flags | O_NONBLOCK);
    }

    struct sockaddr_in dest = {
        .sin_family = AF_INET,
        .sin_port = htons(port),
        .sin_addr.s_addr = htonl(host_addr),
    };

    int rc = connect(sock, (struct sockaddr *)&dest, sizeof(dest));
    if (rc == 0) {
        close(sock);
        return true;
    }
    if (errno != EINPROGRESS && errno != EWOULDBLOCK && errno != EAGAIN) {
        close(sock);
        return false;
    }

    fd_set writefds;
    FD_ZERO(&writefds);
    FD_SET(sock, &writefds);
    struct timeval timeout = {
        .tv_sec = timeout_ms / 1000,
        .tv_usec = (timeout_ms % 1000) * 1000,
    };

    rc = select(sock + 1, NULL, &writefds, NULL, &timeout);
    if (rc <= 0 || !FD_ISSET(sock, &writefds)) {
        close(sock);
        return false;
    }

    int err = 0;
    socklen_t err_len = sizeof(err);
    if (getsockopt(sock, SOL_SOCKET, SO_ERROR, &err, &err_len) != 0) {
        close(sock);
        return false;
    }

    close(sock);
    return err == 0;
}

static void apply_hostname_heuristics(host_record_t *host) {
    if (host->hostname[0] == '\0') {
        return;
    }

    char lower[32];
    size_t len = strnlen(host->hostname, sizeof(host->hostname) - 1);
    for (size_t i = 0; i < len; i++) {
        lower[i] = ascii_lower_char(host->hostname[i]);
    }
    lower[len] = '\0';

    if (strstr(lower, "xbox")) {
        refine_host_identity(host, "Microsoft", "xbox", "netbios_name");
    } else if (strstr(lower, "macbook")) {
        refine_host_identity(host, "Apple", "macbook", "netbios_name");
    } else if (strstr(lower, "imac")) {
        refine_host_identity(host, "Apple", "mac", "netbios_name");
    } else if (strstr(lower, "iphone")) {
        refine_host_identity(host, "Apple", "iphone", "netbios_name");
    } else if (strstr(lower, "ipad")) {
        refine_host_identity(host, "Apple", "ipad", "netbios_name");
    } else if (strstr(lower, "galaxy") ||
               (lower[0] == 's' && lower[1] == 'm' && lower[2] == '-')) {
        refine_host_identity(host, "Samsung", "samsung_phone", "netbios_name");
    } else if (strstr(lower, "pixel")) {
        refine_host_identity(host, "Google", "google_pixel", "netbios_name");
    } else if (strstr(lower, "echo") || strstr(lower, "alexa")) {
        refine_host_identity(host, "Amazon", "amazon_echo", "netbios_name");
    } else if (strstr(lower, "firetv") || strstr(lower, "fire-tv") ||
               strstr(lower, "fire_tv")) {
        refine_host_identity(host, "Amazon", "amazon_fire_tv", "netbios_name");
    } else if (strstr(lower, "roku")) {
        refine_host_identity(host, "Roku", "roku", "netbios_name");
    } else if (strstr(lower, "sonos")) {
        refine_host_identity(host, "Sonos", "sonos_speaker", "netbios_name");
    } else if (strstr(lower, "ring")) {
        refine_host_identity(host, "Ring", "ring_doorbell", "netbios_name");
    } else if (strstr(lower, "nest")) {
        refine_host_identity(host, "Google", "nest_device", "netbios_name");
    } else if (strstr(lower, "shield")) {
        refine_host_identity(host, "NVIDIA", "nvidia_shield", "netbios_name");
    } else if (strstr(lower, "playstation") || strstr(lower, "ps4") ||
               strstr(lower, "ps5")) {
        refine_host_identity(host, "Sony", "playstation", "netbios_name");
    } else if (strstr(lower, "nintendo") ||
               (strcmp(host->vendor, "Nintendo") == 0 && strstr(lower, "switch"))) {
        refine_host_identity(host, "Nintendo", "nintendo_switch", "netbios_name");
    }
}

static void apply_tcp_fingerprint(host_record_t *host) {
    const uint32_t timeout_ms = CONFIG_EXTRITTIO_ANALYZER_PING_TIMEOUT_MS / 2;

    if (tcp_port_open(host->addr, 62078, timeout_ms)) {
        refine_host_identity(host, "Apple", "iphone_or_ipad", "tcp_service");
        return;
    }
    if (tcp_port_open(host->addr, 8009, timeout_ms) ||
        tcp_port_open(host->addr, 8008, timeout_ms)) {
        refine_host_identity(host, "Google", "chromecast_or_google_home", "tcp_service");
        return;
    }
    if (tcp_port_open(host->addr, 7000, timeout_ms)) {
        refine_host_identity(host, "Apple", "airplay_device", "tcp_service");
        return;
    }
    if (tcp_port_open(host->addr, 9100, timeout_ms)) {
        refine_host_identity(host, NULL, "printer", "tcp_service");
        return;
    }
    if (tcp_port_open(host->addr, 548, timeout_ms)) {
        refine_host_identity(host,
                             strcmp(host->vendor, "Apple") == 0 ? "Apple" : NULL,
                             strcmp(host->vendor, "Apple") == 0 ? "mac" : "file_server_or_nas",
                             "tcp_service");
        return;
    }
    if (tcp_port_open(host->addr, 445, timeout_ms) ||
        tcp_port_open(host->addr, 139, timeout_ms)) {
        if (strcmp(host->vendor, "Apple") == 0) {
            refine_host_identity(host, "Apple", "mac", "tcp_service");
        } else {
            refine_host_identity(host, NULL, "windows_or_smb_device", "tcp_service");
        }
        return;
    }
    if (tcp_port_open(host->addr, 8001, timeout_ms)) {
        if (strcmp(host->vendor, "Samsung") == 0 ||
            strcmp(host->vendor, "Unknown") == 0 ||
            strcmp(host->vendor, "Private/Randomized") == 0) {
            refine_host_identity(host, "Samsung", "samsung_smart_tv", "tcp_service");
        }
        return;
    }
    if (tcp_port_open(host->addr, 1400, timeout_ms)) {
        refine_host_identity(host, "Sonos", "sonos_speaker", "tcp_service");
        return;
    }
    if (tcp_port_open(host->addr, 1925, timeout_ms)) {
        refine_host_identity(host, "Philips", "philips_tv", "tcp_service");
        return;
    }
    if (tcp_port_open(host->addr, 3689, timeout_ms)) {
        refine_host_identity(host, "Apple", "apple_device", "tcp_service");
        return;
    }
    if (tcp_port_open(host->addr, 9295, timeout_ms)) {
        refine_host_identity(host, "Sony", "playstation", "tcp_service");
        return;
    }
    if (tcp_port_open(host->addr, 8008, timeout_ms) ||
        tcp_port_open(host->addr, 8443, timeout_ms)) {
        if (strcmp(host->vendor, "Google") == 0 ||
            strcmp(host->vendor, "Unknown") == 0 ||
            strcmp(host->vendor, "Private/Randomized") == 0) {
            refine_host_identity(host, "Google", "chromecast_or_google_home", "tcp_service");
        }
    }
}

static void apply_service_fingerprint(host_record_t *host, const uint8_t *data,
                                      size_t len, const char *classification) {
    if (bytes_contains_ci(data, len, "model=MacBook")) {
        refine_host_identity(host, "Apple", "macbook", "mdns_model");
    } else if (bytes_contains_ci(data, len, "model=iMac") ||
               bytes_contains_ci(data, len, "model=Mac")) {
        refine_host_identity(host, "Apple", "mac", "mdns_model");
    } else if (bytes_contains_ci(data, len, "model=iPhone")) {
        refine_host_identity(host, "Apple", "iphone", "mdns_model");
    } else if (bytes_contains_ci(data, len, "model=iPad")) {
        refine_host_identity(host, "Apple", "ipad", "mdns_model");
    } else if (bytes_contains_ci(data, len, "model=AppleTV")) {
        refine_host_identity(host, "Apple", "apple_tv", "mdns_model");
    } else if (bytes_contains_ci(data, len, "model=HomePod")) {
        refine_host_identity(host, "Apple", "homepod", "mdns_model");
    } else if (bytes_contains_ci(data, len, "model=AppleWatch") ||
               bytes_contains_ci(data, len, "model=Watch")) {
        refine_host_identity(host, "Apple", "apple_watch", "mdns_model");
    } else if (bytes_contains_ci(data, len, "model=iPod")) {
        refine_host_identity(host, "Apple", "ipod_touch", "mdns_model");
    } else if (bytes_contains_ci(data, len, "_googlecast") ||
               bytes_contains_ci(data, len, "chromecast")) {
        if (bytes_contains_ci(data, len, "Google Home") ||
            bytes_contains_ci(data, len, "Google-Home") ||
            bytes_contains_ci(data, len, "googlehome") ||
            bytes_contains_ci(data, len, "Nest")) {
            refine_host_identity(host, "Google", "google_home_or_nest", classification);
        } else {
            refine_host_identity(host, "Google", "chromecast", classification);
        }
    } else if (bytes_contains_ci(data, len, "_airplay") ||
               bytes_contains_ci(data, len, "_raop") ||
               bytes_contains_ci(data, len, "_companion-link") ||
               bytes_contains_ci(data, len, "_apple-mobdev2")) {
        refine_host_identity(host, "Apple", "apple_device", classification);
    } else if (bytes_contains_ci(data, len, "_pscontrol") ||
               bytes_contains_ci(data, len, "playstation") ||
               bytes_contains_ci(data, len, "PS4") ||
               bytes_contains_ci(data, len, "PS5")) {
        refine_host_identity(host, "Sony", "playstation", classification);
    } else if (bytes_contains_ci(data, len, "_xbox-smartglass") ||
               bytes_contains_ci(data, len, "xbox")) {
        refine_host_identity(host, "Microsoft", "xbox", classification);
    } else if (bytes_contains_ci(data, len, "nintendo") ||
               bytes_contains_ci(data, len, "_nintendo")) {
        refine_host_identity(host, "Nintendo", "nintendo_switch", classification);
    } else if (bytes_contains_ci(data, len, "_nvstream") ||
               (bytes_contains_ci(data, len, "nvidia") &&
                bytes_contains_ci(data, len, "shield"))) {
        refine_host_identity(host, "NVIDIA", "nvidia_shield", classification);
    } else if (bytes_contains_ci(data, len, "_androidtvremote") ||
               bytes_contains_ci(data, len, "androidtv") ||
               bytes_contains_ci(data, len, "android_tv")) {
        refine_host_identity(host, NULL, "android_tv", classification);
    } else if (bytes_contains_ci(data, len, "_amzn-wplay") ||
               bytes_contains_ci(data, len, "amazon") ||
               bytes_contains_ci(data, len, "alexa")) {
        if (bytes_contains_ci(data, len, "fire tv") ||
            bytes_contains_ci(data, len, "firetv")) {
            refine_host_identity(host, "Amazon", "amazon_fire_tv", classification);
        } else {
            refine_host_identity(host, "Amazon", "amazon_device", classification);
        }
    } else if (bytes_contains_ci(data, len, "_ipp") ||
               bytes_contains_ci(data, len, "_printer") ||
               bytes_contains_ci(data, len, "printer")) {
        refine_host_identity(host, NULL, "printer", classification);
    } else if (bytes_contains_ci(data, len, "internetgatewaydevice") ||
               bytes_contains_ci(data, len, "wanipconnection")) {
        refine_host_identity(host, NULL, "router", classification);
    } else if (bytes_contains_ci(data, len, "synology") ||
               bytes_contains_ci(data, len, "qnap") ||
               bytes_contains_ci(data, len, "diskstation") ||
               bytes_contains_ci(data, len, "nas")) {
        refine_host_identity(host, NULL, "nas", classification);
    } else if (bytes_contains_ci(data, len, "samsung") &&
               (bytes_contains_ci(data, len, "tv") ||
                bytes_contains_ci(data, len, "SmartTV"))) {
        refine_host_identity(host, "Samsung", "samsung_smart_tv", classification);
    } else if (bytes_contains_ci(data, len, "webos") ||
               (bytes_contains_ci(data, len, "lg") &&
                bytes_contains_ci(data, len, "tv"))) {
        refine_host_identity(host, "LG", "lg_smart_tv", classification);
    } else if (bytes_contains_ci(data, len, "philips") &&
               bytes_contains_ci(data, len, "tv")) {
        refine_host_identity(host, "Philips", "philips_tv", classification);
    } else if (bytes_contains_ci(data, len, "philips-hue") ||
               bytes_contains_ci(data, len, "philipshue")) {
        refine_host_identity(host, "Philips", "philips_hue_bridge", classification);
    } else if (bytes_contains_ci(data, len, "roku")) {
        refine_host_identity(host, "Roku", "roku", classification);
    } else if (bytes_contains_ci(data, len, "sonos") ||
               bytes_contains_ci(data, len, "_sonos")) {
        refine_host_identity(host, "Sonos", "sonos_speaker", classification);
    } else if (bytes_contains_ci(data, len, "_spotify-connect")) {
        refine_host_identity(host, NULL, "spotify_connect_speaker", classification);
    } else if (bytes_contains_ci(data, len, "_wemo")) {
        refine_host_identity(host, "Belkin", "wemo_device", classification);
    } else if (bytes_contains_ci(data, len, "ring")) {
        refine_host_identity(host, "Ring", "ring_doorbell", classification);
    } else if (bytes_contains_ci(data, len, "_smb")) {
        refine_host_identity(host, NULL, "windows_or_smb_device", classification);
    } else if (bytes_contains_ci(data, len, "_hap") ||
               bytes_contains_ci(data, len, "_matter")) {
        refine_host_identity(host, NULL, "smart_home_device", classification);
    }
}

static void apply_ssdp_discovery(scan_result_t *scan) {
    int sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_IP);
    if (sock < 0) {
        return;
    }

    struct timeval timeout = {
        .tv_sec = 0,
        .tv_usec = 120000,
    };
    setsockopt(sock, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout));

    struct sockaddr_in dest = {
        .sin_family = AF_INET,
        .sin_port = htons(1900),
        .sin_addr.s_addr = inet_addr("239.255.255.250"),
    };

    const char request[] =
        "M-SEARCH * HTTP/1.1\r\n"
        "HOST: 239.255.255.250:1900\r\n"
        "MAN: \"ssdp:discover\"\r\n"
        "MX: 1\r\n"
        "ST: ssdp:all\r\n\r\n";
    sendto(sock, request, strlen(request), 0, (struct sockaddr *)&dest, sizeof(dest));

    int64_t deadline_us = esp_timer_get_time() + 900000;
    uint8_t response[768];
    while (esp_timer_get_time() < deadline_us) {
        struct sockaddr_in from;
        socklen_t from_len = sizeof(from);
        ssize_t received = recvfrom(sock, response, sizeof(response), 0,
                                    (struct sockaddr *)&from, &from_len);
        if (received <= 0) {
            continue;
        }

        uint32_t addr = ntohl(from.sin_addr.s_addr);
        host_record_t *host = find_host_by_addr(scan, addr);
        if (host != NULL) {
            apply_service_fingerprint(host, response, (size_t)received, "ssdp_service");
        }
    }

    close(sock);
}

static size_t mdns_build_ptr_query(const char *name, uint8_t *packet, size_t packet_len) {
    if (packet_len < 18) {
        return 0;
    }

    memset(packet, 0, packet_len);
    packet[5] = 0x01;
    size_t pos = 12;
    const char *label = name;
    while (*label != '\0') {
        const char *dot = strchr(label, '.');
        size_t label_len = dot == NULL ? strlen(label) : (size_t)(dot - label);
        if (label_len == 0 || label_len > 63 || pos + label_len + 1 >= packet_len) {
            return 0;
        }
        packet[pos++] = (uint8_t)label_len;
        memcpy(&packet[pos], label, label_len);
        pos += label_len;
        if (dot == NULL) {
            break;
        }
        label = dot + 1;
    }

    if (pos + 5 > packet_len) {
        return 0;
    }
    packet[pos++] = 0x00;
    packet[pos++] = 0x00;
    packet[pos++] = 0x0c;
    packet[pos++] = 0x00;
    packet[pos++] = 0x01;
    return pos;
}

static void apply_mdns_discovery(scan_result_t *scan) {
    int sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_IP);
    if (sock < 0) {
        return;
    }

    int yes = 1;
    setsockopt(sock, SOL_SOCKET, SO_REUSEADDR, &yes, sizeof(yes));

    struct sockaddr_in bind_addr = {
        .sin_family = AF_INET,
        .sin_port = htons(5353),
        .sin_addr.s_addr = htonl(INADDR_ANY),
    };
    if (bind(sock, (struct sockaddr *)&bind_addr, sizeof(bind_addr)) != 0) {
        close(sock);
        return;
    }

    struct ip_mreq mreq = {
        .imr_multiaddr.s_addr = inet_addr("224.0.0.251"),
        .imr_interface.s_addr = htonl(INADDR_ANY),
    };
    setsockopt(sock, IPPROTO_IP, IP_ADD_MEMBERSHIP, &mreq, sizeof(mreq));

    struct timeval timeout = {
        .tv_sec = 0,
        .tv_usec = 120000,
    };
    setsockopt(sock, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout));

    struct sockaddr_in dest = {
        .sin_family = AF_INET,
        .sin_port = htons(5353),
        .sin_addr.s_addr = inet_addr("224.0.0.251"),
    };

    const char *queries[] = {
        "_device-info._tcp.local",
        "_airplay._tcp.local",
        "_raop._tcp.local",
        "_companion-link._tcp.local",
        "_apple-mobdev2._tcp.local",
        "_googlecast._tcp.local",
        "_ipp._tcp.local",
        "_printer._tcp.local",
        "_smb._tcp.local",
        "_workstation._tcp.local",
        "_hap._tcp.local",
        "_matter._tcp.local",
        "_pscontrol._tcp.local",
        "_xbox-smartglass-devicerelay._tcp.local",
        "_nintendo._tcp.local",
        "_nvstream._tcp.local",
        "_androidtvremote._tcp.local",
        "_androidtvremote2._tcp.local",
        "_amzn-wplay._tcp.local",
        "_sonos._tcp.local",
        "_spotify-connect._tcp.local",
        "_dacp._tcp.local",
        "_wemo._tcp.local",
    };

    uint8_t packet[128];
    for (size_t i = 0; i < sizeof(queries) / sizeof(queries[0]); i++) {
        size_t packet_len = mdns_build_ptr_query(queries[i], packet, sizeof(packet));
        if (packet_len > 0) {
            sendto(sock, packet, packet_len, 0, (struct sockaddr *)&dest, sizeof(dest));
        }
    }

    int64_t deadline_us = esp_timer_get_time() + 900000;
    uint8_t response[900];
    while (esp_timer_get_time() < deadline_us) {
        struct sockaddr_in from;
        socklen_t from_len = sizeof(from);
        ssize_t received = recvfrom(sock, response, sizeof(response), 0,
                                    (struct sockaddr *)&from, &from_len);
        if (received <= 0) {
            continue;
        }

        uint32_t addr = ntohl(from.sin_addr.s_addr);
        host_record_t *host = find_host_by_addr(scan, addr);
        if (host != NULL) {
            apply_service_fingerprint(host, response, (size_t)received, "mdns_service");
        }
    }

    setsockopt(sock, IPPROTO_IP, IP_DROP_MEMBERSHIP, &mreq, sizeof(mreq));
    close(sock);
}

static bool lookup_arp_mac(esp_netif_t *esp_netif, uint32_t host_addr, char *out, size_t len) {
#if EXTRITTIO_HAVE_ARP_LOOKUP
    struct netif *lwip_netif = (struct netif *)esp_netif_get_netif_impl(esp_netif);
    if (lwip_netif == NULL) {
        return false;
    }

    ip4_addr_t ipaddr;
    ipaddr.addr = htonl(host_addr);
    struct eth_addr *eth = NULL;
    const ip4_addr_t *eth_ip = NULL;
    if (etharp_find_addr(lwip_netif, &ipaddr, &eth, &eth_ip) < 0 || eth == NULL) {
        return false;
    }

    snprintf(out, len, "%02x:%02x:%02x:%02x:%02x:%02x",
             eth->addr[0], eth->addr[1], eth->addr[2],
             eth->addr[3], eth->addr[4], eth->addr[5]);
    return true;
#else
    (void)esp_netif;
    (void)host_addr;
    (void)out;
    (void)len;
    return false;
#endif
}

static bool arp_probe_host(esp_netif_t *esp_netif, uint32_t host_addr,
                           char *mac, size_t mac_len, uint32_t *elapsed_ms) {
#if EXTRITTIO_HAVE_ARP_LOOKUP
    struct netif *lwip_netif = (struct netif *)esp_netif_get_netif_impl(esp_netif);
    if (lwip_netif == NULL) {
        return false;
    }

    int64_t started_us = esp_timer_get_time();
    if (lookup_arp_mac(esp_netif, host_addr, mac, mac_len)) {
        if (elapsed_ms != NULL) {
            *elapsed_ms = 0;
        }
        return true;
    }

    ip4_addr_t ipaddr;
    ipaddr.addr = htonl(host_addr);
    if (etharp_request(lwip_netif, &ipaddr) != ERR_OK) {
        return false;
    }

    int64_t deadline_us =
        started_us + (int64_t)CONFIG_EXTRITTIO_ANALYZER_PING_TIMEOUT_MS * 1000;
    do {
        vTaskDelay(pdMS_TO_TICKS(10));
        if (lookup_arp_mac(esp_netif, host_addr, mac, mac_len)) {
            if (elapsed_ms != NULL) {
                *elapsed_ms = (uint32_t)((esp_timer_get_time() - started_us) / 1000);
            }
            return true;
        }
    } while (esp_timer_get_time() < deadline_us);

    return false;
#else
    (void)esp_netif;
    (void)host_addr;
    (void)mac;
    (void)mac_len;
    (void)elapsed_ms;
    return false;
#endif
}

static void run_scan(esp_netif_t *netif, scan_result_t *scan) {
    memset(scan, 0, sizeof(*scan));

    esp_netif_ip_info_t ip_info;
    if (esp_netif_get_ip_info(netif, &ip_info) != ESP_OK) {
        ESP_LOGE(TAG, "Failed to read station IP info");
        return;
    }

    uint32_t local_ip = ntohl(ip_info.ip.addr);
    uint32_t gateway = ntohl(ip_info.gw.addr);
    uint32_t network = ntohl(ip_info.ip.addr & ip_info.netmask.addr);
    uint32_t broadcast = network | ~ntohl(ip_info.netmask.addr);
    uint32_t max_targets = CONFIG_EXTRITTIO_ANALYZER_MAX_SCAN_TARGETS;

    for (uint32_t addr = network + 1; addr < broadcast; addr++) {
        if (addr == local_ip) {
            continue;
        }
        if (scan->targets_scanned >= max_targets) {
            scan->target_limit_reached = true;
            break;
        }

        scan->targets_scanned++;
        char mac[18] = {0};
        uint32_t elapsed_ms = 0;
        if (!arp_probe_host(netif, addr, mac, sizeof(mac), &elapsed_ms)) {
            continue;
        }

        if (scan->host_count >= CONFIG_EXTRITTIO_ANALYZER_MAX_HOSTS) {
            scan->host_limit_reached = true;
            continue;
        }

        host_record_t *host = &scan->hosts[scan->host_count++];
        host->addr = addr;
        ipv4_to_string(addr, host->ip, sizeof(host->ip));
        strncpy(host->mac, mac, sizeof(host->mac) - 1);
        host->rtt_ms = elapsed_ms;
        host->source = "arp";
        classify_host(host, addr, gateway);
        if (nbns_probe_hostname(addr, host->hostname, sizeof(host->hostname))) {
            refine_host_identity(host, NULL, "windows_or_smb_device", "netbios_name");
            apply_hostname_heuristics(host);
        }
    }

    apply_mdns_discovery(scan);
    apply_ssdp_discovery(scan);
    for (uint32_t i = 0; i < scan->host_count; i++) {
        apply_tcp_fingerprint(&scan->hosts[i]);
    }
}

static void build_snapshot_json(const scan_result_t *scan, char *buf, size_t len) {
    json_writer_t w = {
        .buf = buf,
        .cap = len,
        .len = 0,
        .failed = false,
    };
    if (len > 0) {
        buf[0] = '\0';
    }

    esp_netif_ip_info_t ip_info;
    memset(&ip_info, 0, sizeof(ip_info));
    esp_netif_get_ip_info(wifi_sta_netif(), &ip_info);

    wifi_ap_record_t ap;
    memset(&ap, 0, sizeof(ap));
    esp_wifi_sta_get_ap_info(&ap);

    char local_ip[16];
    char netmask[16];
    char gateway[16];
    char bssid[18];
    char station_mac[18];
    uint8_t sta_mac[6] = {0};
    ipv4_to_string(ntohl(ip_info.ip.addr), local_ip, sizeof(local_ip));
    ipv4_to_string(ntohl(ip_info.netmask.addr), netmask, sizeof(netmask));
    ipv4_to_string(ntohl(ip_info.gw.addr), gateway, sizeof(gateway));
    mac_to_string(ap.bssid, bssid, sizeof(bssid));
    esp_wifi_get_mac(WIFI_IF_STA, sta_mac);
    mac_to_string(sta_mac, station_mac, sizeof(station_mac));

    char ssid[33] = {0};
    memcpy(ssid, ap.ssid, sizeof(ap.ssid));

    jw_append(&w, "{\"schema\":\"extrittio.network_analyzer.v1\",");
    jw_append(&w, "\"scan_id\":%lu,", (unsigned long)scan->scan_id);
    jw_append(&w, "\"network\":{\"ssid\":");
    jw_append_escaped(&w, ssid);
    jw_append(&w, ",\"bssid\":\"%s\",\"channel\":%u,\"rssi\":%d,"
                  "\"authmode\":\"%s\",\"station_mac\":\"%s\","
                  "\"ip\":\"%s\",\"netmask\":\"%s\",\"gateway\":\"%s\"},",
              bssid, (unsigned int)ap.primary, (int)ap.rssi,
              authmode_to_string(ap.authmode), station_mac, local_ip, netmask, gateway);
    jw_append(&w, "\"hosts\":[");

    bool emitted_any = false;
    bool snapshot_truncated = false;
    for (uint32_t i = 0; i < scan->host_count; i++) {
        if (w.len + 280 >= w.cap) {
            snapshot_truncated = true;
            break;
        }
        const host_record_t *host = &scan->hosts[i];
        jw_append(&w, "%s{\"ip\":\"%s\",\"mac\":\"%s\",\"hostname\":",
                  emitted_any ? "," : "", host->ip, host->mac);
        if (host->hostname[0] == '\0') {
            jw_append(&w, "null");
        } else {
            jw_append_escaped(&w, host->hostname);
        }
        jw_append(&w, ",\"vendor\":");
        jw_append_escaped(&w, host->vendor);
        jw_append(&w, ",\"device_type\":");
        jw_append_escaped(&w, host->device_type);
        jw_append(&w, ",\"classification\":");
        jw_append_escaped(&w, host->classification);
        jw_append(&w, ",\"reachable\":true,\"rtt_ms\":%lu,\"source\":\"%s\"}",
                  (unsigned long)host->rtt_ms, host->source);
        emitted_any = true;
    }

    jw_append(&w, "],\"host_count\":%lu,\"targets_scanned\":%lu,"
                  "\"target_limit_reached\":%s,"
                  "\"host_limit_reached\":%s,\"snapshot_truncated\":%s}",
              (unsigned long)scan->host_count,
              (unsigned long)scan->targets_scanned,
              scan->target_limit_reached ? "true" : "false",
              scan->host_limit_reached ? "true" : "false",
              snapshot_truncated || w.failed ? "true" : "false");
}

static void publish_scan(z_loaned_session_t *session, const scan_result_t *scan,
                         const char *snapshot_json) {
    char host_count[16];
    char targets_scanned[16];
    snprintf(host_count, sizeof(host_count), "%lu", (unsigned long)scan->host_count);
    snprintf(targets_scanned, sizeof(targets_scanned), "%lu",
             (unsigned long)scan->targets_scanned);

    extrittio_metadata_entry_t metadata[] = {
        {.key = "kind", .value = "network_analyzer_scan"},
        {.key = "schema", .value = "extrittio.network_analyzer.v1"},
        {.key = "host_count", .value = host_count},
        {.key = "targets_scanned", .value = targets_scanned},
        {.key = "snapshot_json", .value = snapshot_json},
    };

    extrittio_telemetry_t telemetry = {
        .device_id = CONFIG_EXTRITTIO_DEVICE_ID,
        .timestamp = extrittio_now_millis(),
        .temperature = 0.0f,
        .humidity = 0.0f,
        .battery_level = 100.0f,
        .metadata_count = sizeof(metadata) / sizeof(metadata[0]),
        .metadata = metadata,
    };

    int rc = extrittio_telemetry_publish(session, &telemetry);
    if (rc == 0) {
        ESP_LOGI(TAG, "Published scan: hosts=%lu targets=%lu",
                 (unsigned long)scan->host_count,
                 (unsigned long)scan->targets_scanned);
    } else {
        ESP_LOGE(TAG, "Failed to publish scan telemetry: rc=%d", rc);
    }
}

void app_main(void) {
    esp_err_t ret = nvs_flash_init();
    if (ret == ESP_ERR_NVS_NO_FREE_PAGES ||
        ret == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_ERROR_CHECK(nvs_flash_erase());
        ret = nvs_flash_init();
    }
    ESP_ERROR_CHECK(ret);

    ESP_ERROR_CHECK(wifi_init_sta());

    z_owned_config_t config;
    z_config_default(&config);
    zp_config_insert(z_loan_mut(config), Z_CONFIG_CONNECT_KEY,
                     CONFIG_EXTRITTIO_ZENOH_CONNECT);

    z_owned_session_t session;
    if (z_open(&session, z_move(config), NULL) != 0) {
        ESP_LOGE(TAG, "Failed to open Zenoh session");
        return;
    }

    if (zp_start_read_task(z_loan_mut(session), NULL) != 0 ||
        zp_start_lease_task(z_loan_mut(session), NULL) != 0) {
        ESP_LOGE(TAG, "Failed to start Zenoh tasks");
        z_drop(z_move(session));
        return;
    }

    int64_t boot_time = esp_timer_get_time();
    int64_t last_heartbeat = 0;
    uint32_t scan_id = 0;
    char *snapshot = malloc(CONFIG_EXTRITTIO_ANALYZER_SNAPSHOT_BYTES);
    if (snapshot == NULL) {
        ESP_LOGE(TAG, "Failed to allocate snapshot buffer");
        z_drop(z_move(session));
        return;
    }

    ESP_LOGI(TAG, "Network analyzer started, device=%s, interval=%ds",
             CONFIG_EXTRITTIO_DEVICE_ID, CONFIG_EXTRITTIO_SCAN_INTERVAL_S);

    while (1) {
        int64_t loop_started_ms = extrittio_now_millis();

        scan_result_t scan;
        scan.scan_id = ++scan_id;
        run_scan(wifi_sta_netif(), &scan);
        scan.scan_id = scan_id;
        build_snapshot_json(&scan, snapshot, CONFIG_EXTRITTIO_ANALYZER_SNAPSHOT_BYTES);
        publish_scan(z_loan(session), &scan, snapshot);

        int64_t now = extrittio_now_millis();
        int64_t hb_interval_ms = CONFIG_EXTRITTIO_HEARTBEAT_INTERVAL_S * 1000LL;
        if (now - last_heartbeat >= hb_interval_ms) {
            int64_t uptime_us = esp_timer_get_time() - boot_time;
            extrittio_heartbeat_t hb = {
                .device_id = CONFIG_EXTRITTIO_DEVICE_ID,
                .timestamp = now,
                .status = EXTRITTIO_STATUS_ONLINE,
                .firmware = CONFIG_EXTRITTIO_FIRMWARE_VERSION,
                .uptime_seconds = uptime_us / 1000000,
            };
            extrittio_heartbeat_publish(z_loan(session), &hb);
            last_heartbeat = now;
        }

        int64_t elapsed_ms = extrittio_now_millis() - loop_started_ms;
        int64_t interval_ms = CONFIG_EXTRITTIO_SCAN_INTERVAL_S * 1000LL;
        int64_t delay_ms = interval_ms > elapsed_ms ? interval_ms - elapsed_ms : 1000;
        vTaskDelay(pdMS_TO_TICKS(delay_ms));
    }
}
