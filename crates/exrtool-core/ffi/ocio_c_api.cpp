#include "ocio_c_api.h"
#include <OpenColorIO/OpenColorIO.h>

namespace OCIO = OCIO_NAMESPACE;

extern "C" {

OcioConfig ocio_config_from_file(const char *path) {
    try {
        OCIO::ConstConfigRcPtr cfg = OCIO::Config::CreateFromFile(path);
        return new OCIO::ConstConfigRcPtr(cfg);
    } catch (...) {
        return nullptr;
    }
}

void ocio_config_release(OcioConfig cfg) {
    if (!cfg) return;
    auto c = static_cast<OCIO::ConstConfigRcPtr*>(cfg);
    delete c;
}

OcioProcessor ocio_config_get_processor(OcioConfig cfg, const char *src, const char *dst) {
    if (!cfg) return nullptr;
    auto c = static_cast<OCIO::ConstConfigRcPtr*>(cfg);
    try {
        OCIO::ConstProcessorRcPtr proc = (*c)->getProcessor(src, dst);
        OCIO::ConstCPUProcessorRcPtr cpu = proc->getDefaultCPUProcessor();
        return new OCIO::ConstCPUProcessorRcPtr(cpu);
    } catch (...) {
        return nullptr;
    }
}

int ocio_config_num_displays(OcioConfig cfg) {
    if (!cfg) return 0;
    auto c = static_cast<OCIO::ConstConfigRcPtr*>(cfg);
    try {
        return (*c)->getNumDisplays();
    } catch (...) {
        return 0;
    }
}

const char* ocio_config_get_display_name(OcioConfig cfg, int index) {
    if (!cfg) return nullptr;
    auto c = static_cast<OCIO::ConstConfigRcPtr*>(cfg);
    try {
        return (*c)->getDisplay(index);
    } catch (...) {
        return nullptr;
    }
}

int ocio_config_num_views(OcioConfig cfg, const char *display) {
    if (!cfg) return 0;
    auto c = static_cast<OCIO::ConstConfigRcPtr*>(cfg);
    try {
        return (*c)->getNumViews(display);
    } catch (...) {
        return 0;
    }
}

const char* ocio_config_get_view_name(OcioConfig cfg, const char *display, int index) {
    if (!cfg) return nullptr;
    auto c = static_cast<OCIO::ConstConfigRcPtr*>(cfg);
    try {
        return (*c)->getView(display, index);
    } catch (...) {
        return nullptr;
    }
}

OcioProcessor ocio_config_get_processor_display_view(OcioConfig cfg, const char *display, const char *view) {
    if (!cfg) return nullptr;
    auto c = static_cast<OCIO::ConstConfigRcPtr*>(cfg);
    try {
        OCIO::DisplayViewTransformRcPtr dvt = OCIO::DisplayViewTransform::Create();
        dvt->setSrc((*c)->getColorSpaceNameByRole("scene_linear"));
        dvt->setDisplay(display);
        dvt->setView(view);
        OCIO::ConstProcessorRcPtr proc = (*c)->getProcessor(dvt, OCIO::TRANSFORM_DIR_FORWARD);
        OCIO::ConstCPUProcessorRcPtr cpu = proc->getDefaultCPUProcessor();
        return new OCIO::ConstCPUProcessorRcPtr(cpu);
    } catch (...) {
        return nullptr;
    }
}

void ocio_processor_release(OcioProcessor proc) {
    if (!proc) return;
    auto p = static_cast<OCIO::ConstCPUProcessorRcPtr*>(proc);
    delete p;
}

void ocio_processor_apply_rgb(OcioProcessor proc, float rgb[3]) {
    if (!proc || !rgb) return;
    auto p = static_cast<OCIO::ConstCPUProcessorRcPtr*>(proc);
    (*p)->applyRGB(rgb);
}

}
