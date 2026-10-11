#ifndef MOSSRS_BRIDGE_CORE_H
#define MOSSRS_BRIDGE_CORE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct MossEngine MossEngine;

typedef struct MossVec2 {
    float x;
    float y;
} MossVec2;

typedef struct MossVec3 {
    float x;
    float y;
    float z;
} MossVec3;

typedef struct MossColor {
    float r;
    float g;
    float b;
    float a;
} MossColor;

typedef enum MossResult {
    MOSS_OK = 0,
    MOSS_ERROR = 1,
    MOSS_INVALID_ARGUMENT = 2,
    MOSS_OUT_OF_MEMORY = 3,
    MOSS_NOT_INITIALIZED = 4,
    MOSS_UNSUPPORTED = 5
} MossResult;

MossEngine* moss_engine_create(void);
void moss_engine_destroy(MossEngine* engine);

int32_t moss_engine_initialize(MossEngine* engine);
int32_t moss_engine_shutdown(MossEngine* engine);

int32_t moss_engine_update(MossEngine* engine, double delta_time);
int32_t moss_engine_render(MossEngine* engine);

#ifdef __cplusplus
}
#endif

#endif
