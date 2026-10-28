#include "wrapper.h"
extern "C" bool drdj_stretch_clip(float *input, size_t frames, float *output, size_t out_frames, int channels, float rate) noexcept {
 signalsmith_stretch_t *handle = nullptr;
 try {
  handle = signalsmith_stretch_create_preset_default(channels, rate);
  bool ok = signalsmith_stretch_exact(handle, input, frames, output, out_frames);
  signalsmith_stretch_destroy(handle);
  return ok;
 } catch (...) { if (handle) signalsmith_stretch_destroy(handle); return false; }
}
