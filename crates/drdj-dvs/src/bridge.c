/* DRDJPro xwax adapter, GPL-3.0-only. See vendor/xwax/COPYING. */
#include <stdlib.h>
#include "timecoder.h"

void *drdj_xwax_new(const char *name, unsigned int rate, int phono)
{
    struct timecode_def *def = timecoder_find_definition(name);
    if (!def) return NULL;
    struct timecoder *tc = calloc(1, sizeof(*tc));
    if (!tc) return NULL;
    timecoder_init(tc, def, 1.0, rate, phono != 0);
    return tc;
}
void drdj_xwax_delete(void *ptr)
{
    struct timecoder *tc = ptr;
    timecoder_clear(tc);
    free(tc);
}
void drdj_xwax_submit(void *ptr, const short *pcm, size_t frames)
{
    /* Upstream never mutates PCM; the API predates const-correctness. */
    timecoder_submit(ptr, (short *)pcm, frames);
}
int drdj_xwax_position(void *ptr, double *age)
{
    return timecoder_get_position(ptr, age);
}
double drdj_xwax_pitch(void *ptr) { return timecoder_get_pitch(ptr); }
double drdj_xwax_resolution(void *ptr) { return timecoder_get_resolution(ptr); }
unsigned int drdj_xwax_safe(void *ptr) { return timecoder_get_safe(ptr); }
void drdj_xwax_reset(void *ptr, unsigned int rate, int phono)
{
    struct timecoder *tc = ptr;
    struct timecode_def *def = timecoder_get_definition(tc);
    timecoder_clear(tc);
    /* Initialize counters omitted by upstream init_channel as well. */
    *tc = (struct timecoder){0};
    timecoder_init(tc, def, 1.0, rate, phono != 0);
}
