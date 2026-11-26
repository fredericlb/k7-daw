// PluginsApi.cpp — the plugins area of the C ABI (k7_engine_plugins.h) over the scan, the parameter probes and
// EngineHost's plugin instances. What a reader gets wrong: nothing here keeps a pointer, so every string handed to
// a callback lives for that call only; the blocking functions are the ones the old server ran on detached threads.

#include "k7_engine_plugins.h"

#include "CApi.h"

#include "../EngineHost.h"

#include <string>
#include <utility>
#include <vector>

namespace studio::capi {
namespace {

EngineHost* host(K7Host* h) { return reinterpret_cast<EngineHost*>(h); }

void listTo(const std::vector<PluginInfo>& plugins, void* ctx, K7PluginEach out) {
    if (out == nullptr) return;
    for (const auto& p : plugins) {
        const K7PluginInfo one{view(p.name), view(p.format), view(p.manufacturer), p.isInstrument ? 1 : 0,
                               view(p.role), view(p.category), view(p.identifier)};
        out(ctx, &one);
    }
}

int32_t cacheWarm() { return guard<int32_t>("plugins.cache_warm", 0, [] { return pluginCacheWarm() ? 1 : 0; }); }

void loadCache(void* ctx, K7PluginEach out) {
    guardVoid("plugins.load_cache", [ctx, out] { listTo(loadPluginCache(), ctx, out); });
}

int32_t scan(int32_t force, void* ctx, K7ScanStep step, K7PluginEach out, K7StrEach failed) {
    try {
        const auto progress = [ctx, step](int done, int total, const std::string& loading) {
            if (step != nullptr) step(ctx, done, total, view(loading));
        };
        listTo(scanInstalledPlugins(force != 0, progress), ctx, out);
        return 1;
    } catch (const std::exception& e) {
        give(failed, ctx, e.what());
    } catch (...) {
        give(failed, ctx, "unknown exception");
    }
    return 0;
}

int32_t paramsOf(K7Str format, K7Str id, void* ctx, K7ParamInfoEach out) {
    return guard<int32_t>("plugins.plugin_params_of", 0, [&] {
        const auto found = pluginParamsOf(str(format), str(id));
        if (!found) return 0;
        for (const auto& p : *found) {
            std::vector<K7ParamState> states;
            for (const auto& [label, value] : p.states) states.push_back(K7ParamState{view(label), value});
            const K7ParamInfo info{view(p.name),        0.0, p.min, p.max, p.hasDefault ? 1 : 0, p.def,
                                   states.data(), states.size()};
            if (out != nullptr) out(ctx, &info);
        }
        return 1;
    });
}

int32_t prepare(K7Str format, K7Str id) {
    return guard<int32_t>("plugins.prepare_native_ui", 0,
                          [&] { return prepareNativePluginUI(str(format), str(id)) ? 1 : 0; });
}

int32_t showOf(K7Host*, K7Str format, K7Str id) {
    return guard<int32_t>("plugins.show_native_ui_of", 0,
                          [&] { return showNativePluginUIOf(str(format), str(id)) ? 1 : 0; });
}

int32_t snapshotTo(K7Host*, K7Str format, K7Str id, K7Str path) {
    return guard<int32_t>("plugins.snapshot_orphan_to", 0,
                          [&] { return snapshotOrphanPluginTo(str(format), str(id), str(path)) ? 1 : 0; });
}

void showUi(K7Host* h, K7Str session, K7Str id) {
    guardVoid("plugins.show_native_ui", [&] { host(h)->showNativePluginUI(str(session), str(id)); });
}

void instances(K7Host* h, K7Str session, void* ctx, K7InstanceEach out) {
    guardVoid("plugins.plugin_instances", [&] {
        if (out == nullptr) return;
        for (const auto& p : host(h)->pluginInstanceInfos(str(session))) {
            const K7PluginInstance one{view(p.id), view(p.name), p.hasNativePluginUI ? 1 : 0};
            out(ctx, &one);
        }
    });
}

void presets(K7Host* h, K7Str session, K7Str module, void* ctx, K7PresetEach out) {
    guardVoid("plugins.list_presets", [&] {
        if (out == nullptr) return;
        for (const auto& p : host(h)->listPresets(str(session), str(module))) {
            const K7Preset one{view(p.name), view(p.source), p.index, view(p.path)};
            out(ctx, &one);
        }
    });
}

void setPreset(K7Host* h, K7Str session, K7Str module, int32_t index, K7Str path) {
    guardVoid("plugins.set_preset", [&] { host(h)->setPreset(str(session), str(module), index, str(path)); });
}

void setParam(K7Host* h, K7Str session, K7Str id, K7Str name, double value) {
    guardVoid("plugins.set_plugin_param",
              [&] { host(h)->setPluginParam(str(session), str(id), str(name), value); });
}

void tapOn(K7Host* h, K7Str session, K7Str id, K7Str side) {
    guardVoid("plugins.enable_waveform_tap",
              [&] { host(h)->enableWaveformTap(str(session), str(id), str(side)); });
}

void tapOff(K7Host* h, K7Str session, K7Str id, K7Str side) {
    guardVoid("plugins.disable_waveform_tap",
              [&] { host(h)->disableWaveformTap(str(session), str(id), str(side)); });
}

const K7PluginsApi kPlugins{cacheWarm, loadCache, scan,      paramsOf,  prepare, showOf, snapshotTo,
                            showUi,    instances, presets,   setPreset, setParam, tapOn,  tapOff};

}  // namespace

const K7PluginsApi* pluginsApi() { return &kPlugins; }

}  // namespace studio::capi
