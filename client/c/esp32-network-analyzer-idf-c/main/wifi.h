#ifndef WIFI_H
#define WIFI_H

#include "esp_err.h"
#include "esp_netif.h"

esp_err_t wifi_init_sta(void);
esp_netif_t *wifi_sta_netif(void);

#endif /* WIFI_H */
