// MachineApi_test.cpp — the machine and render areas of the C ABI (engine/capi/k7_engine_machine.h, k7_engine_render.h)
// on a whole EngineHost: the sessions it lists and drops, the drivers, the interface, the meters, the broadcast tap,
// and a render asked of the serving engine. The one thing a reader would get wrong: no audio device is opened, on
// purpose, as in CApi_test.cpp, so a meter reads silence and a device switch finds nothing.

#include "capi/k7_engine.h"
#include "capi/k7_engine_core.h"
#include "capi/k7_engine_machine.h"
#include "capi/k7_engine_render.h"
#include "EngineHost.h"
#include "ProjectDoc.h"
#include "test_harness.h"

#include <atomic>
#include <chrono>
#include <filesystem>
#include <string>
#include <thread>
#include <vector>

namespace {

const K7CoreApi& core() { return *k7_engine_api()->core; }
const K7MachineApi& machine() { return *k7_engine_api()->machine; }
const K7RenderApi& renderArea() { return *k7_engine_api()->render; }

K7Str s(const char* text) { return K7Str{text, std::string(text).size()}; }

studio::ProjectDoc song() {
    studio::ProjectDoc doc;
    doc.name = "machine";
    studio::TrackDoc one;
    one.id = "one";
    one.name = "one";
    one.role = "regular";
    studio::TrackDoc two = one;
    two.id = "two";
    two.name = "two";
    doc.tracks = {one, two};
    return doc;
}

void table() {
    const K7EngineApi* api = k7_engine_api();
    CHECK(api->machine != nullptr && api->render != nullptr);
    CHECK(api->machine->drain_broadcast != nullptr && api->machine->master_level_db != nullptr);
    CHECK(api->render->render != nullptr && api->render->cancel != nullptr);
}

struct Sessions {
    std::vector<std::string> ids;
    std::string project;
    int tracks = 0;
    bool loaded = false;
};

void eachSession(void* ctx, const K7SessionInfo* info) {
    auto* out = static_cast<Sessions*>(ctx);
    out->ids.emplace_back(info->id.ptr, info->id.len);
    if (out->ids.back() != "k7:a") return;
    out->project.assign(info->project.ptr, info->project.len);
    out->tracks = info->track_count;
    out->loaded = info->loaded != 0;
}

void eachLevel(void* ctx, const K7ChannelLevel* l) {
    auto* out = static_cast<std::vector<std::pair<float, int>>*>(ctx);
    out->emplace_back(l->db, l->found);
}

void eachPitch(void* ctx, const K7ChannelPitch* p) {
    auto* out = static_cast<std::vector<std::pair<float, float>>*>(ctx);
    out->emplace_back(p->hz, p->clarity);
}

void eachDevice(void* ctx, const K7AudioDevice*) { ++*static_cast<int*>(ctx); }
void eachInput(void* ctx, const K7InputDevice*) { ++*static_cast<int*>(ctx); }

void sessionsAndMeters(K7Host* host) {
    auto* engine = reinterpret_cast<studio::EngineHost*>(host);
    engine->applyFull("k7:a", song(), "");

    Sessions seen;
    machine().session_infos(host, &seen, eachSession);
    CHECK(seen.loaded && seen.tracks == 2 && seen.project == "machine");

    // A driver arriving and leaving is accepted for a session that exists and for one that does not.
    machine().add_driver(host, s("k7:a"));
    machine().remove_driver(host, s("k7:a"));
    machine().remove_driver(host, s("never-was"));

    // With no interface open the meters answer: one entry per set, found false for a channel nothing carries.
    const int32_t one[] = {1};
    const int32_t two[] = {99, 100};
    const K7IntList sets[] = {{one, 1}, {two, 2}};
    std::vector<std::pair<float, int>> levels;
    machine().channel_levels_db(host, sets, 2, &levels, eachLevel);
    CHECK(levels.size() == 2);
    CHECK(levels.size() == 2 && levels[1].second == 0 && levels[1].first <= -60.0f);
    std::vector<std::pair<float, float>> pitches;
    machine().channel_pitches(host, sets, 2, &pitches, eachPitch);
    CHECK(pitches.size() == 2);
    int inputs = 0;
    machine().input_devices(host, &inputs, eachInput);
    CHECK(machine().master_level_db(host, s("k7:a")) <= -60.0f);
    CHECK(machine().set_input_stereo(host, 1, 1) == 0);

    // An interface by a name no backend offers is refused, and the list is whole either way.
    int devices = 0;
    machine().list_audio_devices(host, &devices, eachDevice);
    CHECK(devices >= 0);
    CHECK(machine().set_audio_device(host, s("no such interface"), s("")) == 0);

    // The callback's cost reads the atomics of a host whose device never opened: nothing ran.
    machine().reset_callback_cost(host);
    const auto cost = machine().callback_cost(host);
    CHECK(cost.runs == 0 && cost.mean_seconds == 0.0);

    // The tap goes on, drains empty with nothing playing, and goes off; a session never made is not created by off.
    machine().enable_broadcast(host, s("k7:a"), 1);
    machine().silence_output(host, s("k7:a"), 1);
    std::vector<float> samples(480 * 2, 7.0f);
    K7BroadcastInfo info{99, 99, 99.0};
    machine().drain_broadcast(host, s("k7:a"), 480, samples.data(), &info);
    CHECK(info.frames == 0);
    machine().skip_broadcast_to_live(host, s("k7:a"));
    machine().silence_output(host, s("k7:a"), 0);
    machine().enable_broadcast(host, s("k7:a"), 0);
    machine().enable_broadcast(host, s("ghost"), 0);
    CHECK(!engine->hasSession("ghost"));

    // Dropping a session forgets it outright.
    machine().drop_session(host, s("k7:a"));
    CHECK(!engine->hasSession("k7:a"));

    CHECK(machine().instrument_installed(s("@instruments/nowhere/none.sfz")) == 0);
    CHECK(machine().instrument_installed(s("/no/such/file.sfz")) == 0);
}

struct Done {
    std::atomic<int> calls{0};
    int code = -1;
    std::string manifest;
    std::string error;
};

void onDone(void* ctx, const K7RenderOutcome* o) {
    auto* done = static_cast<Done*>(ctx);
    done->code = o->code;
    done->manifest.assign(o->manifest.ptr == nullptr ? "" : o->manifest.ptr, o->manifest.len);
    done->error.assign(o->error.ptr == nullptr ? "" : o->error.ptr, o->error.len);
    ++done->calls;
}

// A render waits on the message loop, so it runs on a worker while this thread runs the loop.
void render(K7Host* host) {

    Done notJson;
    renderArea().render(s("not json"), s("/tmp"), 0, &notJson, onDone);
    CHECK(notJson.calls == 1 && notJson.code != 0);
    CHECK(notJson.error.rfind("bad project: ", 0) == 0);

    const std::string doc =
        R"({"name":"empty","tempo":120.0,"sampleRate":48000,"timeSigNumerator":4,"timeSigDenominator":4,)"
        R"("masterVolume":0.0,"masterPlugins":[],"recordDir":"","recordMute":false,)"
        R"("settings":{"metronome":false,"metronomeLevel":0.6,"countIn":0},"tracks":[]})";
    const std::string outDir = (std::filesystem::temp_directory_path() / "k7-machine-render").string();
    Done done;
    std::thread worker([&] {
        renderArea().render(K7Str{doc.data(), doc.size()}, K7Str{outDir.data(), outDir.size()}, 0, &done, onDone);
        core().quit(host);
    });
    std::thread watchdog([host, &done] {
        for (int i = 0; i < 600 && done.calls == 0; ++i) std::this_thread::sleep_for(std::chrono::milliseconds(100));
        core().quit(host);
    });
    core().run(host);
    worker.join();
    watchdog.join();

    // Whatever an empty project comes to, the outcome is whole: a manifest naming the mix, or a reason.
    CHECK(done.calls == 1);
    CHECK(done.code == 0 ? done.manifest.find("mix") != std::string::npos : !done.error.empty());
    renderArea().cancel();
}

}  // namespace

int main() {
    RUN(table);
    // One host for the run: a second message loop in the process trips AppKit's periodic events.
    core().init_settings_dir(K7Str{nullptr, 0});
    K7Host* host = core().host_create();
    CHECK(host != nullptr);
    if (host == nullptr) return 1;
    RUN([host] { sessionsAndMeters(host); });
    RUN([host] { render(host); });
    core().host_destroy(host);
    TEST_MAIN_END();
}
