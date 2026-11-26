// CoreApi.cpp — the core area of the C ABI (k7_engine_core.h) over EngineHost and the process's setup calls.
// What a reader gets wrong: a posted job that never runs still reaches its `drop`, through the destructor of what
// JUCE's queue holds, so the Rust side never leaks a closure or waits on a reply that cannot come.

#include "k7_engine_core.h"

#include "CApi.h"

#include "../EngineHost.h"
#include "../PathRoots.h"
#include "../ProjectDoc.h"
#include "../SettingsDir.h"
#include "../Warnings.h"

#include <cstdlib>
#include <exception>
#include <iostream>
#include <memory>
#include <string>
#include <vector>

namespace studio::capi {
namespace {

EngineHost* host(K7Host* h) { return reinterpret_cast<EngineHost*>(h); }
const EngineHost* host(const K7Host* h) { return reinterpret_cast<const EngineHost*>(h); }

// What JUCE's queue holds for one posted job: `drop` runs from its destructor unless `run` came first.
struct Posted {
    K7Task run;
    K7Task drop;
    void* ctx;
    bool ran = false;
    Posted(K7Task r, K7Task d, void* c) : run(r), drop(d), ctx(c) {}
    Posted(const Posted&) = delete;
    Posted& operator=(const Posted&) = delete;
    ~Posted() {
        if (!ran && drop != nullptr) drop(ctx);
    }
};

int32_t layoutConsistent() {
    const std::size_t here = modelLayoutHere();
    return here == modelLayoutInDiff() && here == modelLayoutInBuilder() ? 1 : 0;
}

void installCrash() { guardVoid("install_crash_handler", [] { installCrashHandler(); }); }

void initSettings(K7Str flag) {
    guardVoid("init_settings_dir", [flag] {
        std::string name = "k7-runtime", key = "--settings-dir", value = str(flag);
        std::vector<char*> argv{name.data()};
        if (!value.empty()) {
            argv.push_back(key.data());
            argv.push_back(value.data());
        }
        int argc = static_cast<int>(argv.size());
        argv.push_back(nullptr);
        initSettingsDir(argc, argv.data());
    });
}

void settings(void* ctx, K7StrEach out) { give(out, ctx, settingsDir()); }

void statePath(void* ctx, K7StrEach out) {
    guardVoid("state_path", [ctx, out] { give(out, ctx, studioStatePath()); });
}

void loadGuards() { guardVoid("enable_load_guards", [] { enableLoadGuards(); }); }

void safeMode(int32_t on) { setSafeMode(on != 0); }

void defaultRoots() {
    guardVoid("use_default_path_roots", [] { setPathRoots(defaultPathRoots(studioStatePath())); });
}

void warningSink(K7WarningSink sink, void* ctx) {
    if (sink == nullptr) {
        setWarningSink({});
        return;
    }
    setWarningSink([sink, ctx](const Warning& w) {
        const K7Warning out{view(w.session), view(w.subject), view(w.message)};
        sink(ctx, &out);
    });
}

K7Host* create() {
    return guard<K7Host*>("host_create", nullptr, [] { return reinterpret_cast<K7Host*>(new EngineHost()); });
}

void destroy(K7Host* h) { guardVoid("host_destroy", [h] { delete host(h); }); }

void run(K7Host* h) { guardVoid("run", [h] { host(h)->run(); }); }

void quit(K7Host* h) { guardVoid("quit", [h] { host(h)->quit(); }); }

void post(K7Host* h, K7Task run, K7Task drop, void* ctx) {
    auto job = std::make_shared<Posted>(run, drop, ctx);
    guardVoid("post", [h, &job] {
        host(h)->post([job] {
            job->ran = true;
            job->run(job->ctx);
        });
    });
}

void openDevice(K7Host* h) { guardVoid("open_audio_device", [h] { host(h)->openAudioDevice(); }); }

void outputDevice(K7Host* h, void* ctx, K7StrEach out) {
    guardVoid("output_device_name", [h, ctx, out] { give(out, ctx, host(h)->outputDeviceName()); });
}

void suspend(K7Host* h, int32_t suspended) {
    guardVoid("suspend_audio", [h, suspended] { host(h)->suspendAudio(suspended != 0); });
}

K7AudioTiming timing(const K7Host* h) {
    const auto t = host(h)->audioTiming();
    return K7AudioTiming{t.bufferFrames, t.sampleRate};
}

double messageAge(const K7Host* h) { return host(h)->messageThreadAgeMs(); }

void sessions(const K7Host* h, void* ctx, K7StrEach each) {
    guardVoid("session_ids", [h, ctx, each] {
        for (const auto& id : host(h)->sessionIds()) give(each, ctx, id);
    });
}

int32_t hasSession(const K7Host* h, K7Str id) {
    return guard<int32_t>("has_session", 0, [h, id] { return host(h)->hasSession(str(id)) ? 1 : 0; });
}

void terminateHandler() {
    std::set_terminate([] {
        std::cerr << "[fatal] terminate";
        if (const auto e = std::current_exception()) {
            try {
                std::rethrow_exception(e);
            } catch (const std::exception& x) {
                std::cerr << ": " << x.what();
            } catch (...) {
                std::cerr << ": unknown exception";
            }
        }
        std::cerr << std::endl;
        std::abort();
    });
}

void loadDeath(void* ctx, K7StrEach out) {
    guardVoid("previous_load_death", [ctx, out] { give(out, ctx, previousLoadDeath()); });
}

void lastPlugin(void* ctx, K7StrEach out) {
    guardVoid("previous_last_plugin", [ctx, out] { give(out, ctx, previousLastPlugin()); });
}

const K7CoreApi kCore{
    layoutConsistent,
    installCrash,
    initSettings,
    settings,
    statePath,
    loadGuards,
    safeMode,
    defaultRoots,
    warningSink,
    create,
    destroy,
    run,
    quit,
    post,
    openDevice,
    outputDevice,
    suspend,
    timing,
    messageAge,
    sessions,
    hasSession,
    terminateHandler,
    loadDeath,
    lastPlugin,
};

}  // namespace

const K7CoreApi* coreApi() { return &kCore; }

}  // namespace studio::capi
