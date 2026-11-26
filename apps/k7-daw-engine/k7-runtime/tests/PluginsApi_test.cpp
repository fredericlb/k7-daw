// PluginsApi_test.cpp — the plugins area of the C ABI (engine/capi/k7_engine_plugins.h) on a whole EngineHost: the
// cache and its warmth, a plugin type's parameters asked of a catalogue that lists none, and every message-thread
// call made with no project and no orphan, which must answer empty and never throw across the door.
// The one thing a reader would get wrong: a real scan is not run here, since it spawns this binary's own children
// for each plugin on the machine; the scan is tested through the Rust server's fake engine.

#include "capi/k7_engine.h"
#include "capi/k7_engine_core.h"
#include "capi/k7_engine_plugins.h"
#include "EngineHost.h"
#include "test_harness.h"

#include <atomic>
#include <chrono>
#include <filesystem>
#include <string>
#include <thread>
#include <vector>

namespace {

const K7CoreApi& core() { return *k7_engine_api()->core; }
const K7PluginsApi& plugins() { return *k7_engine_api()->plugins; }

K7Str s(const std::string& v) { return K7Str{v.data(), v.size()}; }

struct Listed {
    std::vector<std::string> formats;
    std::vector<std::string> ids;
    int instruments = 0;
};

void onPlugin(void* ctx, const K7PluginInfo* p) {
    auto* l = static_cast<Listed*>(ctx);
    l->formats.emplace_back(p->format.ptr, p->format.len);
    l->ids.emplace_back(p->id.ptr, p->id.len);
    l->instruments += p->instrument;
}

void count(void* ctx, const K7ParamInfo*) { ++*static_cast<int*>(ctx); }
void countInstance(void* ctx, const K7PluginInstance*) { ++*static_cast<int*>(ctx); }
void countPreset(void* ctx, const K7Preset*) { ++*static_cast<int*>(ctx); }

void table() {
    const K7PluginsApi* p = k7_engine_api()->plugins;
    CHECK(p != nullptr);
    if (p == nullptr) return;
    CHECK(p->cache_warm && p->load_cache && p->scan && p->plugin_params_of && p->prepare_native_ui);
    CHECK(p->show_native_ui_of && p->snapshot_orphan_to && p->show_native_ui && p->plugin_instances);
    CHECK(p->list_presets && p->set_preset && p->set_plugin_param && p->enable_waveform_tap &&
          p->disable_waveform_tap);
}

void aColdCacheStillListsTheBuiltIns() {
    // A folder of its own, so no cache of this machine is read or written.
    const auto dir = (std::filesystem::temp_directory_path() / "k7-pluginsapi-test").string();
    std::filesystem::remove_all(dir);
    core().init_settings_dir(s(dir));
    CHECK(plugins().cache_warm() == 0);
    Listed listed;
    plugins().load_cache(&listed, onPlugin);
    CHECK(listed.ids.size() > 100);
    CHECK(listed.instruments > 0);
    bool allInternal = true;
    for (const auto& f : listed.formats) allInternal = allInternal && f == "internal";
    CHECK(allInternal);
}

void theMessageThreadCallsAnswerEmptyWithNothingLoaded() {
    K7Host* host = core().host_create();
    CHECK(host != nullptr);
    if (host == nullptr) return;
    const std::string session = "k7:none", id = "chain:none";
    int instances = 0, presets = 0;
    plugins().plugin_instances(host, s(session), &instances, countInstance);
    plugins().list_presets(host, s(session), s("module"), &presets, countPreset);
    CHECK(instances == 0 && presets == 0);
    // No instance to act on: each says so and moves nothing.
    plugins().set_plugin_param(host, s(session), s(id), s("gain"), 0.5);
    plugins().set_preset(host, s(session), s("module"), 3, K7Str{nullptr, 0});
    plugins().enable_waveform_tap(host, s(session), s(id), s("output"));
    plugins().disable_waveform_tap(host, s(session), s(id), s("output"));
    plugins().show_native_ui(host, s(session), s(id));
    CHECK(plugins().show_native_ui_of(host, s("VST3"), s("/none.vst3")) == 0);
    CHECK(plugins().snapshot_orphan_to(host, s("VST3"), s("/none.vst3"), s("/tmp/base.vstpreset")) == 0);
    CHECK(plugins().snapshot_orphan_to(host, s("VST3"), s("/none.vst3"), s("/tmp/not-a-preset.txt")) == 0);
    core().host_destroy(host);
}

void aTypeTheCatalogueDoesNotListCannotBeProbed() {
    K7Host* host = core().host_create();
    CHECK(host != nullptr);
    if (host == nullptr) return;
    int found = -1, listed = 0;
    std::atomic<int> prepared{-1};
    // The probes wait on the message thread for the plugin's creation, so the loop runs under them.
    std::thread worker([&] {
        found = plugins().plugin_params_of(s("VST3"), s("/none.vst3"), &listed, count);
        prepared = plugins().prepare_native_ui(s("VST3"), s("/none.vst3"));
        core().quit(host);
    });
    std::thread watchdog([host, &prepared] {
        for (int i = 0; i < 300 && prepared == -1; ++i) std::this_thread::sleep_for(std::chrono::milliseconds(100));
        core().quit(host);
    });
    core().run(host);
    worker.join();
    watchdog.join();
    CHECK(found == 0 && listed == 0);
    CHECK(prepared.load() == 0);
    core().host_destroy(host);
}

}  // namespace

int main() {
    RUN(table);
    RUN(aColdCacheStillListsTheBuiltIns);
    RUN(theMessageThreadCallsAnswerEmptyWithNothingLoaded);
    RUN(aTypeTheCatalogueDoesNotListCannotBeProbed);
    TEST_MAIN_END();
}
