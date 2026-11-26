// CApi.h — what every area of the C ABI shares on the C++ side: strings in and out, and the guard every
// function body runs in. Internal: the Rust server sees k7_engine*.h only.
// What a reader gets wrong: no exception may leave an extern "C" function, so a throw is said on stderr and the
// caller gets the fallback the function documents.

#pragma once

#include "k7_engine.h"

#include <exception>
#include <iostream>
#include <string>
#include <string_view>

namespace studio::capi {

inline std::string str(K7Str s) { return s.ptr == nullptr ? std::string{} : std::string(s.ptr, s.len); }

inline K7Str view(std::string_view s) { return K7Str{s.data(), s.size()}; }

// Hands one string to a K7StrEach, which may be NULL when the caller wants nothing.
inline void give(K7StrEach out, void* ctx, std::string_view s) {
    if (out != nullptr) out(ctx, view(s));
}

// Runs `f`, or says why it threw and returns `fallback`.
template <typename R, typename F>
R guard(const char* what, R fallback, F&& f) {
    try {
        return f();
    } catch (const std::exception& e) {
        std::cerr << "[capi] " << what << " failed: " << e.what() << "\n";
    } catch (...) {
        std::cerr << "[capi] " << what << " failed (unknown exception)\n";
    }
    return fallback;
}

// The same for a function that returns nothing.
template <typename F>
void guardVoid(const char* what, F&& f) {
    guard(what, 0, [&] {
        f();
        return 0;
    });
}

// The table of each area, built where that area is implemented; one line per area in EngineApi.cpp.
const K7CoreApi* coreApi();
const K7ProjectApi* projectApi();
const K7TransportApi* transportApi();
const K7StateApi* stateApi();
const K7MachineApi* machineApi();
const K7RenderApi* renderApi();

}  // namespace studio::capi
