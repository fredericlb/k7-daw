// StateApi_test.cpp — the state area of the C ABI (engine/capi/k7_engine_state.h) on a whole EngineHost: its real
// timer's frame arrives in the sink on the message thread with a project's tracks, arm and record state in it.
// The one thing a reader would get wrong: no audio device is opened, on purpose, as in CApi_test.cpp; and the
// take and parameter sinks are installed here but not fired, which needs a recording or a plugin editor.

#include "capi/k7_engine.h"
#include "capi/k7_engine_core.h"
#include "capi/k7_engine_state.h"
#include "EngineHost.h"
#include "ProjectDoc.h"
#include "test_harness.h"

#include <string>
#include <thread>
#include <vector>

namespace {

const K7CoreApi& core() { return *k7_engine_api()->core; }
const K7StateApi& state() { return *k7_engine_api()->state; }

studio::ProjectDoc song() {
    studio::ProjectDoc doc;
    doc.name = "state";
    studio::TrackDoc plain;
    plain.id = "plain";
    plain.name = "plain";
    plain.role = "regular";
    studio::TrackDoc input = plain;
    input.id = "input";
    input.name = "input";
    input.inputChannels = {0};
    doc.tracks = {plain, input};
    return doc;
}

struct Seen {
    K7Host* host = nullptr;
    std::thread::id messageThread;
    std::thread::id sinkThread;
    int frames = 0;
    std::string session;
    size_t levels = 0, inputLevels = 0;
    std::vector<std::string> armed;
    bool recordEnabled = false;
    bool playing = true;
    float master = 0.0f;
};

void onFrame(void* ctx, const K7StateFrame* f) {
    auto* seen = static_cast<Seen*>(ctx);
    if (std::string(f->session.ptr, f->session.len) != "k7:a") return;
    if (++seen->frames > 1) return;
    seen->sinkThread = std::this_thread::get_id();
    seen->session.assign(f->session.ptr, f->session.len);
    seen->levels = f->level_count;
    seen->inputLevels = f->input_level_count;
    for (size_t i = 0; i < f->armed_count; ++i) seen->armed.emplace_back(f->armed[i].ptr, f->armed[i].len);
    seen->recordEnabled = f->record_enabled != 0;
    seen->playing = f->playing != 0;
    seen->master = f->master_db;
    core().quit(seen->host);
}

void infoEach(void* ctx, const K7ParamInfo*) { ++*static_cast<int*>(ctx); }

void table() {
    const K7EngineApi* api = k7_engine_api();
    CHECK(api->state != nullptr);
    CHECK(api->state->start != nullptr && api->state->plugin_param_infos != nullptr);
}

void realTimerFrame() {
    core().init_settings_dir(K7Str{nullptr, 0});
    K7Host* host = core().host_create();
    CHECK(host != nullptr);
    if (host == nullptr) return;
    auto* engine = reinterpret_cast<studio::EngineHost*>(host);
    engine->applyFull("k7:a", song(), "");
    engine->setRecordEnabled("k7:a", true);
    engine->setArmed("k7:a", "input", true);

    Seen seen;
    seen.host = host;
    seen.messageThread = std::this_thread::get_id();
    const K7StateSinks sinks{&seen, onFrame, nullptr, nullptr, nullptr};
    state().start(host, &sinks, 20);

    // Calls that need a live plugin find none and say nothing: an unloaded id is accepted.
    state().watch_plugin_params(host, K7Str{"k7:a", 4}, K7Str{"nope", 4}, 1);
    state().rewind_recorder_envelopes(host, K7Str{"k7:a", 4});
    int infos = 0;
    state().plugin_param_infos(host, K7Str{"k7:a", 4}, K7Str{"nope", 4}, &infos, infoEach);
    CHECK(infos == 0);

    // A hung timer ends the test instead of the suite.
    std::thread watchdog([host, &seen] {
        for (int i = 0; i < 100 && seen.frames == 0; ++i) std::this_thread::sleep_for(std::chrono::milliseconds(100));
        core().quit(host);
    });
    core().run(host);
    watchdog.join();

    CHECK(seen.frames >= 1);
    CHECK(seen.sinkThread == seen.messageThread);
    CHECK(seen.session == "k7:a");
    CHECK(seen.levels == 2 && seen.inputLevels == 2);
    // Arming needs an input the headless engine may not have: what arrives is the engine's own answer, a lane or none.
    CHECK(seen.armed.size() <= 1);
    CHECK(seen.recordEnabled);
    CHECK(!seen.playing);
    CHECK(seen.master <= -60.0f);
    core().host_destroy(host);
}

}  // namespace

int main() {
    RUN(table);
    RUN(realTimerFrame);
    TEST_MAIN_END();
}
