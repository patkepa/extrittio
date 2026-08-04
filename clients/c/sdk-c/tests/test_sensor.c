#include <assert.h>
#include <math.h>
#include <stdio.h>
#include "extrittio/sensor.h"

static void test_init_values(void) {
    extrittio_sensor_state_t s;
    extrittio_sensor_init(&s);
    assert(s.temperature == 22.0f);
    assert(s.humidity == 45.0f);
    assert(s.battery == 100.0f);
}

static void test_step_clamps_temperature_high(void) {
    extrittio_sensor_state_t s = { .temperature = 30.0f, .humidity = 50.0f, .battery = 50.0f };
    extrittio_sensor_step(&s, 1.0f, 0.0f, 0.0f);
    assert(s.temperature == 30.0f);
}

static void test_step_clamps_temperature_low(void) {
    extrittio_sensor_state_t s = { .temperature = 15.0f, .humidity = 50.0f, .battery = 50.0f };
    extrittio_sensor_step(&s, -1.0f, 0.0f, 0.0f);
    assert(s.temperature == 15.0f);
}

static void test_step_drains_battery(void) {
    extrittio_sensor_state_t s;
    extrittio_sensor_init(&s);
    extrittio_sensor_step(&s, 0.0f, 0.0f, 0.1f);
    assert(fabsf(s.battery - 99.9f) < 0.001f);
}

static void test_battery_does_not_go_negative(void) {
    extrittio_sensor_state_t s = { .temperature = 22.0f, .humidity = 45.0f, .battery = 0.01f };
    extrittio_sensor_step(&s, 0.0f, 0.0f, 0.1f);
    assert(s.battery == 0.0f);
}

static void test_step_clamps_humidity(void) {
    extrittio_sensor_state_t s = { .temperature = 22.0f, .humidity = 80.0f, .battery = 50.0f };
    extrittio_sensor_step(&s, 0.0f, 5.0f, 0.0f);
    assert(s.humidity == 80.0f);

    s.humidity = 20.0f;
    extrittio_sensor_step(&s, 0.0f, -5.0f, 0.0f);
    assert(s.humidity == 20.0f);
}

int main(void) {
    test_init_values();
    test_step_clamps_temperature_high();
    test_step_clamps_temperature_low();
    test_step_drains_battery();
    test_battery_does_not_go_negative();
    test_step_clamps_humidity();
    printf("All sensor tests passed.\n");
    return 0;
}
