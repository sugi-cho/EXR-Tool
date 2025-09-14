#pragma once
#ifdef __cplusplus
extern "C" {
#endif

typedef void* OcioConfig;
typedef void* OcioProcessor;

OcioConfig ocio_config_from_file(const char *path);
void ocio_config_release(OcioConfig cfg);
OcioProcessor ocio_config_get_processor(OcioConfig cfg, const char *src, const char *dst);
int ocio_config_num_displays(OcioConfig cfg);
const char* ocio_config_get_display_name(OcioConfig cfg, int index);
int ocio_config_num_views(OcioConfig cfg, const char *display);
const char* ocio_config_get_view_name(OcioConfig cfg, const char *display, int index);
OcioProcessor ocio_config_get_processor_display_view(OcioConfig cfg, const char *display, const char *view);
void ocio_processor_release(OcioProcessor proc);
void ocio_processor_apply_rgb(OcioProcessor proc, float rgb[3]);

#ifdef __cplusplus
}
#endif
