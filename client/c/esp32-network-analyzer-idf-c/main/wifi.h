#ifndef WIFI_H
#define WIFI_H

#include "esp_err.h"
#include "esp_netif.h"

esp_err_t wifi_init_sta(const char *ssid, const char *password);
esp_netif_t *wifi_sta_netif(void);

#endif /* WIFI_H */
