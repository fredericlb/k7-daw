// MachineJobs_test.cpp — the jobs that only answer, called as the agent calls them: the log seeded
// by its tail and continued by offset, read a bounded slice at a time, a set list counted,
// the providers going through the one accessor rather than the file, an undo's audio sent back,
// and the month after which this machine stops keeping it; a write outside the channel's roots refused,
// and a read outside this machine's copies of them answered as if the file were absent.

#include "MachineJobs.h"
#include "Paths.h"

#include "test_harness.h"

#include <ixwebsocket/IXWebSocketServer.h>

#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <functional>
#include <mutex>

namespace fs = std::filesystem;
using nlohmann::json;
using nlohmann::ordered_json;

namespace {

fs::path scratch()
{
    const auto dir = fs::temp_directory_path() / ("k7-jobs-" + std::to_string(std::rand()));
    // rand() is unseeded, so a later run gets the same names: start from an empty folder.
    fs::remove_all(dir);
    fs::create_directories(dir);
#ifdef _WIN32
    _putenv_s("STUDIO_HOME", dir.string().c_str());
#else
    setenv("STUDIO_HOME", dir.c_str(), 1);
#endif
    return dir;
}

std::vector<ordered_json> ask(k7::tunnel::Jobs& jobs, const json& req, std::function<bool(const std::string&)> writable = {})
{
    std::vector<ordered_json> replies;
    k7::tunnel::JobContext ctx;
    ctx.writable = std::move(writable);
    jobs.at(req["type"].get<std::string>())(req, [&](const ordered_json& r) { replies.push_back(r); }, ctx);
    return replies;
}

k7::MachineAccess fakeAccess(std::vector<std::string>& held)
{
    k7::MachineAccess a;
    a.onnxProviders = [&held](const std::optional<std::vector<std::string>>& set) {
        if (set) held = *set;
        return held;
    };
    return a;
}

void theLogIsSeededThenContinued()
{
    scratch();
    std::ofstream(k7::paths::mergedLog(), std::ios::binary) << "a\nb\nc\n";
    std::vector<std::string> held;
    auto jobs = k7::machineJobs(fakeAccess(held));
    auto seed = ask(jobs, {{"type", "log"}, {"tail", 2}});
    CHECK(seed.size() == 1);
    CHECK(seed[0]["raw"]["offset"] == 6);
    CHECK((seed[0]["raw"]["lines"] == json{"b", "c"}));
    std::ofstream(k7::paths::mergedLog(), std::ios::binary | std::ios::app) << "d\npartial";
    auto next = ask(jobs, {{"type", "log"}, {"offset", 6}});
    CHECK(next[0]["raw"]["offset"] == 8);
    CHECK((next[0]["raw"]["lines"] == json{"d"}));
}

void aStaleOffsetSeedsFromTheTail()
{
    scratch();
    std::ofstream(k7::paths::mergedLog(), std::ios::binary) << "x\ny\n";
    std::vector<std::string> held;
    auto jobs = k7::machineJobs(fakeAccess(held));
    // An offset from before a rotation: past the end of the file that replaced it.
    auto r = ask(jobs, {{"type", "log"}, {"offset", 5000}});
    CHECK(r[0]["raw"]["offset"] == 4);
    CHECK((r[0]["raw"]["lines"] == json{"x", "y"}));
}

void aLargeLogIsReadByItsTailOnly()
{
    scratch();
    const std::string row(99, 'r');
    {
        std::ofstream out(k7::paths::mergedLog(), std::ios::binary);
        for (int i = 0; i < 30000; ++i) out << row << "\n";
        out << "last\n";
    }
    const auto size = fs::file_size(k7::paths::mergedLog());
    CHECK(size > 2 * k7::kLogReadCap);
    auto seed = k7::readLog(std::nullopt, 100000);
    CHECK(seed["offset"] == size);
    CHECK(seed["lines"].back() == "last");
    // Whole lines only, and no more than the cap holds: the cut at the start is dropped.
    CHECK(seed["lines"].size() <= k7::kLogReadCap / 100 + 1);
    CHECK(seed["lines"].front() == row);
    // Far behind, a reader is handed one cap's worth and catches up on the next polls.
    auto behind = k7::readLog(std::uint64_t{0}, std::nullopt);
    CHECK(behind["offset"].get<std::uint64_t>() <= k7::kLogReadCap);
    CHECK(behind["offset"].get<std::uint64_t>() % 100 == 0);
    CHECK(behind["lines"].size() == behind["offset"].get<std::uint64_t>() / 100);
}

void anAbsentLogIsEmpty()
{
    scratch();
    std::vector<std::string> held;
    auto jobs = k7::machineJobs(fakeAccess(held));
    auto r = ask(jobs, {{"type", "log"}, {"tail", 10}});
    CHECK(r[0]["raw"]["offset"] == 0);
    CHECK(r[0]["raw"]["lines"].empty());
}

// Where the channel keeps its copy of a server's root, as the agent spells it: `<state>/remote/<server>/<root>`.
fs::path copies()
{
    const auto dir = k7::paths::studioDir() / "remote" / "server" / "mediatek";
    fs::create_directories(dir);
    return dir;
}

void aSetListIsCounted()
{
    scratch();
    const auto dir = copies();
    std::ofstream(dir / "one.flac") << "x";
    std::vector<std::string> held;
    auto jobs = k7::machineJobs(fakeAccess(held));
    auto r = ask(jobs, {{"type", "prefetch"},
                        {"files", {k7::paths::utf8(dir / "one.flac"), k7::paths::utf8(dir / "two.flac")}}});
    CHECK(r[0]["type"] == "prefetched");
    CHECK(r[0]["count"] == 1);
}

void providersGoThroughTheAccessor()
{
    scratch();
    std::vector<std::string> held;
    auto jobs = k7::machineJobs(fakeAccess(held));
    auto set = ask(jobs, {{"type", "onnx-providers"}, {"set", {"DML", "cpu", "dml"}}});
    CHECK((set[0]["providers"] == json{"dml", "cpu"}));
    CHECK((held == std::vector<std::string>{"dml", "cpu"}));
    auto read = ask(jobs, {{"type", "onnx-providers"}});
    CHECK((read[0]["providers"] == json{"dml", "cpu"}));
}

// The chain a machine separates with is its OWN setting, written through the accessor; a key not named is left alone.
void theChainsAreSetThroughTheAccessor()
{
    scratch();
    json held = {{"stems", "@k7-core/chains/stems.k7x"}, {"instrumental", "@k7-core/chains/instrumental.k7x"}};
    k7::MachineAccess a;
    a.chains = [&held](const std::optional<json>& set) {
        if (set) {
            if (auto it = set->find("stems"); it != set->end() && it->is_string()) held["stems"] = *it;
            if (auto it = set->find("instrumental"); it != set->end() && it->is_string() && !it->get<std::string>().empty())
                held["instrumental"] = *it;
            else if (set->contains("instrumental")) held.erase("instrumental");
        }
        return json{{"type", "chains"}, {"chains", held}};
    };
    auto jobs = k7::machineJobs(a);
    auto read = ask(jobs, {{"type", "chains"}});
    CHECK(read.size() == 1);
    CHECK((read[0]["chains"] == held));
    // One key named: the other stays.
    auto one = ask(jobs, {{"type", "chains"}, {"set", {{"stems", "@mine/chains/four.k7x"}}}});
    CHECK(one[0]["chains"]["stems"] == "@mine/chains/four.k7x");
    CHECK(one[0]["chains"]["instrumental"] == "@k7-core/chains/instrumental.k7x");
    // Named empty, it is unsaid.
    auto cleared = ask(jobs, {{"type", "chains"}, {"set", {{"instrumental", ""}}}});
    CHECK(!cleared[0]["chains"].contains("instrumental"));
    CHECK(cleared[0]["chains"]["stems"] == "@mine/chains/four.k7x");
}

// The reply must NAME the file, freshly stamped: that is what makes the channel upload it.
void keptAudioIsSentBackUp()
{
    const auto dir = scratch();
    const auto blob = dir / "1-b0";
    std::ofstream(blob) << "old";
    const auto before = fs::file_time_type::clock::now() - std::chrono::hours(48);
    fs::last_write_time(blob, before);
    std::vector<std::string> held;
    auto jobs = k7::machineJobs(fakeAccess(held));
    const auto anywhere = [](const std::string&) { return true; };
    auto r = ask(jobs, {{"type", "send-up"}, {"file", k7::paths::utf8(blob)}}, anywhere);
    CHECK(r[0]["type"] == "sent");
    CHECK(r[0]["file"] == k7::paths::utf8(blob));
    CHECK(fs::last_write_time(blob) > before);
    bool threw = false;
    try {
        ask(jobs, {{"type", "send-up"}, {"file", k7::paths::utf8(dir / "gone")}}, anywhere);
    } catch (const std::exception&) {
        threw = true;
    }
    CHECK(threw);
}

bool refused(k7::tunnel::Jobs& jobs, const json& req)
{
    try {
        ask(jobs, req, [](const std::string&) { return false; });
    } catch (const std::exception& e) {
        return std::string(e.what()).find("refused") != std::string::npos;
    }
    return false;
}

// A path outside the channel's writable roots is refused before anything is written or stamped.
void writesOutsideTheRootsAreRefused()
{
    const auto dir = scratch();
    const auto blob = dir / "secret";
    std::ofstream(blob) << "x";
    const auto before = fs::file_time_type::clock::now() - std::chrono::hours(48);
    fs::last_write_time(blob, before);
    std::vector<std::string> held;
    auto jobs = k7::machineJobs(fakeAccess(held));
    CHECK(refused(jobs, {{"type", "send-up"}, {"file", k7::paths::utf8(blob)}}));
    CHECK(fs::last_write_time(blob) == before);
    CHECK(refused(jobs, {{"type", "render"}, {"doc", json::object()}, {"out", k7::paths::utf8(dir / "mix.wav")}}));
    CHECK(refused(jobs, {{"type", "render"}, {"doc", json::object()}, {"outDir", k7::paths::utf8(dir)}}));
    CHECK(refused(jobs, {{"type", "separate"}, {"source", "a.wav"}, {"model", "m.onnx"}, {"outDir", k7::paths::utf8(dir)}}));
}

// A file that is no copy of a server's answers as an absent one: a probe learns nothing of the rest of the disk.
void readsOutsideTheCopiesRevealNothing()
{
    const auto state = scratch();
    const auto outside = state / "secret.flac";
    std::ofstream(outside) << "x";
    const auto inside = copies();
    std::vector<std::string> held;
    auto jobs = k7::machineJobs(fakeAccess(held));
    const auto climbing = k7::paths::utf8(inside) + "/../../../secret.flac";
    auto r = ask(jobs, {{"type", "prefetch"}, {"files", {k7::paths::utf8(outside), climbing}}});
    CHECK(r[0]["count"] == 0);
    const auto anywhere = [](const std::string&) { return true; };
    bool refusedSource = false;
    try {
        ask(jobs, {{"type", "separate"}, {"source", k7::paths::utf8(outside)}, {"model", k7::paths::utf8(inside / "m.onnx")},
                   {"outDir", k7::paths::utf8(inside)}},
            anywhere);
    } catch (const std::exception& e) {
        refusedSource = std::string(e.what()).find("refused") != std::string::npos;
    }
    CHECK(refusedSource);
    std::error_code ec;
    fs::create_directory_symlink(state, inside / "link", ec);
    // A platform that refuses to make the link has nothing more to test here.
    if (ec) return;
    auto linked = ask(jobs, {{"type", "prefetch"}, {"files", {k7::paths::utf8(inside / "link" / "secret.flac")}}});
    CHECK(linked[0]["count"] == 0);
}

void keptHistoryGoesAfterAMonth()
{
    const auto remote = scratch() / "remote";
    const auto kept = remote / "server" / "history-fred" / "song-0123" / "blobs";
    const auto other = remote / "server" / "ws-fred";
    fs::create_directories(kept);
    fs::create_directories(other);
    std::ofstream(kept / "old") << "x";
    std::ofstream(kept / "new") << "x";
    std::ofstream(other / "clip.wav") << "x";
    const auto old = fs::file_time_type::clock::now() - k7::kKeptHistoryTtl - std::chrono::hours(1);
    fs::last_write_time(kept / "old", old);
    fs::last_write_time(other / "clip.wav", old);
    CHECK(k7::purgeKeptHistory(remote) == 1);
    CHECK(!fs::exists(kept / "old"));
    CHECK(fs::exists(kept / "new"));
    // Only what a server asked to KEEP: a workspace copy is not this purge's.
    CHECK(fs::exists(other / "clip.wav"));
}

// What the fake engine saw of the last `render` command, and how it answers.
struct FakeEngine {
    std::unique_ptr<ix::WebSocketServer> serverPtr;
    int boundPort = 0;
    std::mutex mutex;
    json seen;
    json answer;  // the reply body; `type` and `id` are added

    FakeEngine()
    {
        // The server reports no port of its own for 0, so try one in the dynamic range until a bind holds.
        for (int p = 47311; p < 47411 && !boundPort; ++p) {
            serverPtr = std::make_unique<ix::WebSocketServer>(p, "127.0.0.1");
            if (serverPtr->listen().first) boundPort = p;
        }
        auto& server = *serverPtr;
        server.setOnClientMessageCallback([this](std::shared_ptr<ix::ConnectionState>, ix::WebSocket& ws, const ix::WebSocketMessagePtr& m) {
            if (m->type != ix::WebSocketMessageType::Message) return;
            const auto cmd = json::parse(m->str, nullptr, false);
            if (!cmd.is_object() || cmd.value("type", "") != "render") return;
            json reply;
            {
                std::lock_guard lock(mutex);
                seen = cmd;
                reply = answer;
            }
            // A stray frame first: the job waits for ITS `rendered`, not the first frame.
            ws.send(R"({"type":"whoami"})");
            if (reply.contains("write"))
                for (const auto& f : reply["write"]) {
                    const auto file = fs::path(cmd["outDir"].get<std::string>()) / f.get<std::string>();
                    std::ofstream(file, std::ios::binary) << "RIFF";
                }
            reply.erase("write");
            reply["type"] = "rendered";
            reply["id"] = cmd["id"];
            ws.send(reply.dump());
        });
        server.start();
    }
    ~FakeEngine() { serverPtr->stop(); }
    int port() { return boundPort; }
};

k7::MachineAccess phoneAccess(std::vector<std::string>& held, FakeEngine& engine, std::vector<int>& counts)
{
    auto a = fakeAccess(held);
    a.renderInEngine = true;
    a.enginePort = [&engine] { return engine.port(); };
    a.rendering = [&counts](int delta) { counts.push_back(delta); };
    return a;
}

// On a phone the render job asks the running engine: no program started, the files copied out, the render counted busy.
void aPhoneRendersThroughTheEngine()
{
    const auto dir = scratch();
    FakeEngine engine;
    engine.answer = {{"ok", true}, {"manifest", {{"mix", "mix.wav"}, {"stems", json::array({{{"name", "a"}, {"file", "a.wav"}}})}}}, {"write", json::array({"mix.wav", "a.wav"})}};
    std::vector<std::string> held;
    std::vector<int> counts;
    auto jobs = k7::machineJobs(phoneAccess(held, engine, counts));
    const auto out = dir / "out";
    const auto inside = [&](const std::string& p) { return p.rfind(k7::paths::utf8(dir), 0) == 0; };
    const auto replies = ask(jobs, {{"type", "render"}, {"doc", {{"tracks", json::array()}}}, {"outDir", k7::paths::utf8(out)}, {"stems", true}}, inside);
    CHECK(replies.size() == 2);
    CHECK(replies.back().value("type", "") == "rendered");
    CHECK(fs::exists(out / "mix.wav"));
    CHECK(fs::exists(out / "a.wav"));
    CHECK(replies.back()["stems"].size() == 1);
    // The engine rendered into a folder of its write roots, with the doc and the stems flag.
    CHECK(engine.seen.value("type", "") == "render");
    CHECK(engine.seen["stems"].get<bool>());
    CHECK(engine.seen.contains("doc"));
    CHECK(k7::paths::utf8(fs::path(engine.seen["outDir"].get<std::string>())).rfind(k7::paths::utf8(k7::paths::studioDir()), 0) == 0);
    CHECK(counts == (std::vector<int>{1, -1}));
    // The scratch is gone.
    CHECK(fs::is_empty(k7::paths::studioDir() / "render-tmp"));
}

// A render the engine refuses or fails is an error with its reason, counted back, with nothing left in the scratch folder.
void aPhoneRenderTheEngineRefusesFails()
{
    const auto dir = scratch();
    FakeEngine engine;
    engine.answer = {{"ok", false}, {"busy", true}, {"error", "a render is already running on this engine"}};
    std::vector<std::string> held;
    std::vector<int> counts;
    auto jobs = k7::machineJobs(phoneAccess(held, engine, counts));
    std::string why;
    try {
        ask(jobs, {{"type", "render"}, {"doc", json::object()}, {"out", k7::paths::utf8(dir / "mix.wav")}}, [](const std::string&) { return true; });
    } catch (const std::exception& e) {
        why = e.what();
    }
    CHECK(why.find("already running") != std::string::npos);
    CHECK(!fs::exists(dir / "mix.wav"));
    CHECK(counts == (std::vector<int>{1, -1}));
    CHECK(fs::is_empty(k7::paths::studioDir() / "render-tmp"));
}

// With no engine up there is nothing to ask: the job fails at once, and counts nothing.
void aPhoneRenderWithNoEngineFails()
{
    const auto dir = scratch();
    FakeEngine engine;
    std::vector<std::string> held;
    std::vector<int> counts;
    auto access = phoneAccess(held, engine, counts);
    access.enginePort = [] { return 0; };
    auto jobs = k7::machineJobs(access);
    bool threw = false;
    try {
        ask(jobs, {{"type", "render"}, {"doc", json::object()}, {"out", k7::paths::utf8(dir / "mix.wav")}}, [](const std::string&) { return true; });
    } catch (const std::exception&) {
        threw = true;
    }
    CHECK(threw);
    CHECK(counts.empty());
}

}  // namespace

void runMachineJobsTests()
{
    RUN(theLogIsSeededThenContinued);
    RUN(anAbsentLogIsEmpty);
    RUN(aStaleOffsetSeedsFromTheTail);
    RUN(aLargeLogIsReadByItsTailOnly);
    RUN(aSetListIsCounted);
    RUN(providersGoThroughTheAccessor);
    RUN(theChainsAreSetThroughTheAccessor);
    RUN(keptAudioIsSentBackUp);
    RUN(writesOutsideTheRootsAreRefused);
    RUN(readsOutsideTheCopiesRevealNothing);
    RUN(keptHistoryGoesAfterAMonth);
    RUN(aPhoneRendersThroughTheEngine);
    RUN(aPhoneRenderTheEngineRefusesFails);
    RUN(aPhoneRenderWithNoEngineFails);
}
