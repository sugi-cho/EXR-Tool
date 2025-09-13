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
