// CApi_test.cpp — the engine's C ABI (engine/capi/k7_engine.h) called as the Rust server calls it: the table,
// the process's setup, a host created, a job posted, run until a job quits it, and destroyed.
// The one thing a reader would get wrong: no audio device is opened, on purpose, as in EngineTeardown_test.cpp.

#include "capi/k7_engine.h"
#include "capi/k7_engine_core.h"
#include "capi/k7_engine_project.h"
#include "capi/k7_engine_transport.h"
#include "Warnings.h"
#include "test_harness.h"

#include <string>
#include <thread>
#include <vector>

namespace {

const K7CoreApi& core() { return *k7_engine_api()->core; }

void collect(void* ctx, K7Str s) { static_cast<std::vector<std::string>*>(ctx)->emplace_back(s.ptr, s.len); }

void table() {
    const K7EngineApi* api = k7_engine_api();
    CHECK(api->abi == K7_ENGINE_ABI);
    CHECK(api->size == sizeof(K7EngineApi));
    CHECK(api->core != nullptr);
    CHECK(api->project != nullptr && api->transport != nullptr && api->cli == nullptr);
    CHECK(core().layout_consistent() == 1);
}

struct Sink {
    std::vector<std::string> seen;
};

void sinkOne(void* ctx, const K7Warning* w) {
    static_cast<Sink*>(ctx)->seen.push_back(std::string(w->subject.ptr, w->subject.len) + "|" +
                                            std::string(w->message.ptr, w->message.len));
}

void warnings() {
    Sink sink;
    core().set_warning_sink(sinkOne, &sink);
    studio::warn("a message", "chain:mod");
    core().set_warning_sink(nullptr, nullptr);
    studio::warn("unheard");
    CHECK(sink.seen.size() == 1);
    CHECK(!sink.seen.empty() && sink.seen[0] == "chain:mod|a message");
}

struct Job {
    K7Host* host = nullptr;
    std::thread::id ranOn;
    int runs = 0;
    int drops = 0;
};

void quitting(void* ctx) {
    auto* job = static_cast<Job*>(ctx);
    job->ranOn = std::this_thread::get_id();
    ++job->runs;
    core().quit(job->host);
}

void dropped(void* ctx) { ++static_cast<Job*>(ctx)->drops; }

void hostLife() {
    // $K7_SETTINGS_DIR, which the gate sets, so the engine never writes the user's own Settings.xml.
    core().init_settings_dir(K7Str{nullptr, 0});
    std::vector<std::string> path;
    core().state_path(&path, collect);
    CHECK(path.size() == 1 && !path[0].empty());

    K7Host* host = core().host_create();
    CHECK(host != nullptr);
    if (host == nullptr) return;
    std::vector<std::string> ids;
    core().session_ids(host, &ids, collect);
    CHECK(ids.empty());
    CHECK(core().has_session(host, K7Str{"k7:test", 7}) == 0);
    CHECK(core().audio_timing(host).sample_rate == 0.0);

    Job job;
    job.host = host;
    // Posted from another thread, as a socket's task posts: it runs on the thread that runs the host.
    std::thread([&] { core().post(host, quitting, dropped, &job); }).join();
    core().run(host);
    CHECK(job.runs == 1);
    CHECK(job.drops == 0);
    CHECK(job.ranOn == std::this_thread::get_id());
    core().host_destroy(host);
}

// The project and transport areas on a host whose loop runs: what a fresh host answers, run on the message thread.
struct Areas {
    K7Host* host = nullptr;
    bool ran = false;
};

K7Str of(const std::string& s) { return K7Str{s.data(), s.size()}; }

void askAreas(void* ctx) {
    auto* a = static_cast<Areas*>(ctx);
    const K7ProjectApi& project = *k7_engine_api()->project;
    const K7TransportApi& transport = *k7_engine_api()->transport;
    const std::string session = "k7:areas";
    std::vector<std::string> ids;
    project.track_ids(a->host, of(session), &ids, collect);
    CHECK(ids.empty());
    CHECK(project.release_slot(a->host) == 0);
    CHECK(transport.is_playing(a->host, of(session)) == 0);
    int32_t armed = -1;
    std::string reason;
    CHECK(transport.set_armed(a->host, of(session), of("nobody"), 1, &armed, &reason,
                              [](void* c, K7Str s) { static_cast<std::string*>(c)->assign(s.ptr, s.len); }) == 0);
    CHECK(!reason.empty());
    project.reset(a->host, of(session));
    a->ran = true;
    k7_engine_api()->core->quit(a->host);
}

void areasOnAHost() {
    const K7ProjectApi& project = *k7_engine_api()->project;
    // The roots are the process's: unset here, so a path passes and a share does not.
    CHECK(project.path_allowed(of("/tmp/a.wav"), 0) == 1);
    CHECK(project.path_allowed(of("//host/share/a.wav"), 0) == 0);
    CHECK(project.path_allowed(of("\\\\host\\share\\a.wav"), 1) == 0);
    Sink sink;
    core().set_warning_sink(sinkOne, &sink);
    project.warn(of("k7:areas"), of("tracks"), of("a message"));
    core().set_warning_sink(nullptr, nullptr);
    CHECK(sink.seen.size() == 1 && sink.seen[0] == "tracks|a message");
    int standing = 0;
    project.standing_warnings(of("k7:areas"), &standing, [](void* c, const K7Warning*) { ++*static_cast<int*>(c); });
    CHECK(standing == 0);

    core().init_settings_dir(K7Str{nullptr, 0});
    Areas areas;
    areas.host = core().host_create();
    CHECK(areas.host != nullptr);
    if (areas.host == nullptr) return;
    core().post(areas.host, askAreas, dropped, &areas);
    core().run(areas.host);
    CHECK(areas.ran);
    core().host_destroy(areas.host);
}

}  // namespace

int main() {
    RUN(table);
    RUN(warnings);
    RUN(hostLife);
    RUN(areasOnAHost);
    TEST_MAIN_END();
}
