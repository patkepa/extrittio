#include "programmable_led.h"

#include <stddef.h>
#include <string.h>

#include "driver/spi_master.h"
#include "esp_attr.h"

#define PROGRAMMABLE_LED_SPI_HOST SPI2_HOST
#define PROGRAMMABLE_LED_DATA_GPIO 9
#define PROGRAMMABLE_LED_SPI_CLOCK_HZ 2500000
#define PROGRAMMABLE_LED_COUNT 2
#define PROGRAMMABLE_LED_BITS_PER_PIXEL 24
#define PROGRAMMABLE_LED_SYMBOL_BITS 3
#define PROGRAMMABLE_LED_ONE_SYMBOL 0x6
#define PROGRAMMABLE_LED_ZERO_SYMBOL 0x4
#define PROGRAMMABLE_LED_BYTES_PER_PIXEL \
    ((PROGRAMMABLE_LED_BITS_PER_PIXEL * PROGRAMMABLE_LED_SYMBOL_BITS) / 8)
#define PROGRAMMABLE_LED_FRAME_BYTES \
    (PROGRAMMABLE_LED_COUNT * PROGRAMMABLE_LED_BYTES_PER_PIXEL)

static spi_device_handle_t s_led_spi;
static DMA_ATTR uint8_t s_led_frame[PROGRAMMABLE_LED_FRAME_BYTES];

static void append_symbol(uint8_t symbol, size_t *bit_index)
{
    for (int symbol_bit = PROGRAMMABLE_LED_SYMBOL_BITS - 1;
         symbol_bit >= 0; symbol_bit--) {
        size_t byte_index = *bit_index / 8;
        uint8_t bit_in_byte = 7 - (*bit_index % 8);
        if ((symbol & (1U << symbol_bit)) != 0) {
            s_led_frame[byte_index] |= 1U << bit_in_byte;
        }
        (*bit_index)++;
    }
}

static void append_color_byte(uint8_t value, size_t *bit_index)
{
    for (int color_bit = 7; color_bit >= 0; color_bit--) {
        uint8_t symbol = (value & (1U << color_bit)) != 0
                             ? PROGRAMMABLE_LED_ONE_SYMBOL
                             : PROGRAMMABLE_LED_ZERO_SYMBOL;
        append_symbol(symbol, bit_index);
    }
}

esp_err_t programmable_led_set_all_rgb(uint8_t red, uint8_t green, uint8_t blue)
{
    if (s_led_spi == NULL) {
        return ESP_ERR_INVALID_STATE;
    }

    memset(s_led_frame, 0, sizeof(s_led_frame));
    size_t bit_index = 0;
    for (uint8_t led = 0; led < PROGRAMMABLE_LED_COUNT; led++) {
        append_color_byte(green, &bit_index);
        append_color_byte(red, &bit_index);
        append_color_byte(blue, &bit_index);
    }

    spi_transaction_t transaction = {
        .length = sizeof(s_led_frame) * 8,
        .tx_buffer = s_led_frame,
    };
    return spi_device_transmit(s_led_spi, &transaction);
}

esp_err_t programmable_led_init(void)
{
    if (s_led_spi != NULL) {
        return ESP_ERR_INVALID_STATE;
    }

    spi_bus_config_t bus_configuration = {
        .mosi_io_num = PROGRAMMABLE_LED_DATA_GPIO,
        .miso_io_num = -1,
        .sclk_io_num = -1,
        .quadwp_io_num = -1,
        .quadhd_io_num = -1,
        .max_transfer_sz = sizeof(s_led_frame),
    };
    esp_err_t result = spi_bus_initialize(PROGRAMMABLE_LED_SPI_HOST,
                                           &bus_configuration,
                                           SPI_DMA_CH_AUTO);
    if (result != ESP_OK) {
        return result;
    }

    spi_device_interface_config_t device_configuration = {
        .clock_speed_hz = PROGRAMMABLE_LED_SPI_CLOCK_HZ,
        .mode = 0,
        .spics_io_num = -1,
        .queue_size = 1,
    };
    result = spi_bus_add_device(PROGRAMMABLE_LED_SPI_HOST,
                                &device_configuration, &s_led_spi);
    if (result != ESP_OK) {
        spi_bus_free(PROGRAMMABLE_LED_SPI_HOST);
        return result;
    }

    result = programmable_led_set_all_rgb(0, 0, 0);
    if (result != ESP_OK) {
        spi_bus_remove_device(s_led_spi);
        s_led_spi = NULL;
        spi_bus_free(PROGRAMMABLE_LED_SPI_HOST);
    }
    return result;
}
