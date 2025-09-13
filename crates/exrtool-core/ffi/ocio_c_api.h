#pragma once
#ifdef __cplusplus
extern "C" {
#endif

typedef void* OcioConfig;
typedef void* OcioProcessor;

OcioConfig ocio_config_from_file(const char *path);
void ocio_config_release(OcioConfig cfg);
OcioProcessor ocio_config_get_processor(OcioConfig cfg, const char *src, const char *dst);
void ocio_processor_release(OcioProcessor proc);
void ocio_processor_apply_rgb(OcioProcessor proc, float rgb[3]);

#ifdef __cplusplus
}
#endif
