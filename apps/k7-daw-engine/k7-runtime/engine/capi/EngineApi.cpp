// EngineApi.cpp — k7_engine_api(): the table of area tables, one line per area, NULL until that area is built.

#include "k7_engine.h"

#include "CApi.h"

extern "C" const K7EngineApi* k7_engine_api(void) {
    static const K7EngineApi table{
        K7_ENGINE_ABI,
        static_cast<uint32_t>(sizeof(K7EngineApi)),
        studio::capi::coreApi(),
        studio::capi::projectApi(),
        studio::capi::transportApi(),
        studio::capi::stateApi(),
        nullptr,  // plugins
        studio::capi::machineApi(),
        studio::capi::renderApi(),
        nullptr,  // cli
    };
    return &table;
}
