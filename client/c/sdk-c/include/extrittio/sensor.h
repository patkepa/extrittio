#ifndef EXTRITTIO_SENSOR_H
#define EXTRITTIO_SENSOR_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    float temperature;
    float humidity;
    float battery;
} extrittio_sensor_state_t;

void extrittio_sensor_init(extrittio_sensor_state_t *s);

void extrittio_sensor_step(extrittio_sensor_state_t *s,
                            float temp_offset,
                            float humidity_offset,
                            float battery_drain);

#ifdef __cplusplus
}
#endif

#endif /* EXTRITTIO_SENSOR_H */
