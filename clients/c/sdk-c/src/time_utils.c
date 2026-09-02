#include "extrittio/time_utils.h"
#include <stddef.h>
#include <sys/time.h>

int64_t extrittio_now_millis(void) {
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return (int64_t)tv.tv_sec * 1000 + (int64_t)tv.tv_usec / 1000;
}
