#include "moss_bridge/core.h"

#include <new>

struct MossEngine {
    bool initialized = false;
};

/*
 * This first bridge intentionally does not guess internal Moss class names.
 * Integrate the exact Moss headers/types here as the upstream API is mapped.
 *
 * Keeping this implementation as a tiny C ABI boundary means the Rust side
 * remains stable while the C++ implementation can evolve with Moss.
 */

extern "C" MossEngine* moss_engine_create(void) {
    try {
        return new MossEngine{};
    } catch (...) {
        return nullptr;
    }
}

extern "C" void moss_engine_destroy(MossEngine* engine) {
    delete engine;
}

extern "C" int32_t moss_engine_initialize(MossEngine* engine) {
    if (!engine) {
        return MOSS_INVALID_ARGUMENT;
    }

    try {
        /*
         * TODO:
         * Construct/initialize the corresponding Moss C++ engine here.
         * This placeholder keeps MossRS buildable before the exact upstream
         * engine entry point is selected.
         */
        engine->initialized = true;
        return MOSS_OK;
    } catch (const std::bad_alloc&) {
        return MOSS_OUT_OF_MEMORY;
    } catch (...) {
        return MOSS_ERROR;
    }
}

extern "C" int32_t moss_engine_shutdown(MossEngine* engine) {
    if (!engine) {
        return MOSS_INVALID_ARGUMENT;
    }

    engine->initialized = false;
    return MOSS_OK;
}

extern "C" int32_t moss_engine_update(MossEngine* engine, double /*delta_time*/) {
    if (!engine) {
        return MOSS_INVALID_ARGUMENT;
    }

    if (!engine->initialized) {
        return MOSS_NOT_INITIALIZED;
    }

    /*
     * TODO: forward to Moss's actual update/tick API.
     */
    return MOSS_OK;
}

extern "C" int32_t moss_engine_render(MossEngine* engine) {
    if (!engine) {
        return MOSS_INVALID_ARGUMENT;
    }

    if (!engine->initialized) {
        return MOSS_NOT_INITIALIZED;
    }

    /*
     * TODO: forward to Moss's renderer/frame API.
     */
    return MOSS_OK;
}
