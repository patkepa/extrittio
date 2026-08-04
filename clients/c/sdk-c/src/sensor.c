#include "extrittio/sensor.h"

static float clampf(float v, float lo, float hi) {
    if (v < lo) return lo;
    if (v > hi) return hi;
    return v;
}

void extrittio_sensor_init(extrittio_sensor_state_t *s) {
    s->temperature = 22.0f;
    s->humidity = 45.0f;
    s->battery = 100.0f;
}

void extrittio_sensor_step(extrittio_sensor_state_t *s,
                            float temp_offset,
                            float humidity_offset,
                            float battery_drain) {
    s->temperature = clampf(s->temperature + temp_offset, 15.0f, 30.0f);
    s->humidity = clampf(s->humidity + humidity_offset, 20.0f, 80.0f);
    float new_batt = s->battery - battery_drain;
    s->battery = new_batt < 0.0f ? 0.0f : new_batt;
}
