// StateFrame_test.cpp — the `state` frame as server/Serve.cpp builds it, held to the fixture the Rust server's frame
// is held to too (apps/k7-daw-backend/packages/k7-runtime-server/fixtures/state_frame.txt): one case per line, the
// engine's snapshot as JSON, a tab, and the frame's exact bytes. `frame()` below is that code, copied.
// The one thing a reader would get wrong: the fixture is this test's output, so it proves the fixture, not Serve.cpp;
// K7_STATE_FIXTURE_WRITE=<path> rewrites the expected column from the inputs.

#include "EngineHost.h"
#include "test_harness.h"

#include <nlohmann/json.hpp>

#include <cmath>
#include <cstdlib>
#include <fstream>
#include <string>
#include <vector>

namespace {

using json = nlohmann::json;
using studio::EngineHost;
using studio::WaveformEnvelope;

std::string dump(const json& j) { return j.dump(-1, ' ', false, json::error_handler_t::replace); }

std::vector<WaveformEnvelope> points(const json& a) {
    std::vector<WaveformEnvelope> out;
    for (const auto& p : a) out.push_back({p[0].get<float>(), p[1].get<float>()});
    return out;
}

// The spec's snapshot as the engine's own type.
EngineHost::EngineState snapshot(const json& in) {
    EngineHost::EngineState s;
    s.session = in.value("session", std::string{});
    s.playing = in.value("playing", false);
    s.playheadSeconds = in.value("playhead", 0.0);
    if (in.contains("loop")) {
        s.loop.on = in["loop"][0].get<bool>();
        s.loop.startSeconds = in["loop"][1].get<double>();
        s.loop.endSeconds = in["loop"][2].get<double>();
    }
    s.trackLevelsDb = in.value("levels", std::vector<float>{});
    s.trackInputLevelsDb = in.value("inputLevels", std::vector<float>{});
    s.armedTracks = in.value("armed", std::vector<std::string>{});
    s.recordEnabled = in.value("recordEnabled", false);
    s.masterLevelDb = in.value("master", -100.0f);
    s.masterOverload = in.value("clipped", false);
    // Named, never a temporary: items() of one dangles in the loop.
    const json waveform = in.value("waveform", json::object());
    const json platters = in.value("platters", json::object());
    const json recorders = in.value("recorders", json::object());
    for (const auto& [key, a] : waveform.items()) s.waveformEnvelopes[key] = points(a);
    for (const auto& [id, v] : platters.items()) s.platterSeconds[id] = v.get<double>();
    for (const auto& [id, r] : recorders.items()) {
        EngineHost::RecorderReport rep;
        rep.recording = r.value("recording", false);
        rep.seconds = r.value("seconds", 0.0);
        for (const auto& t : r.value("takes", json::array()))
            rep.takes.push_back({t[0].get<std::string>(), t[1].get<double>(), t[2].get<bool>()});
        rep.envelope = points(r.value("envelope", json::array()));
        rep.envelopeFrom = r.value("from", static_cast<size_t>(0));
        s.recorders[id] = std::move(rep);
    }
    if (in.contains("load")) {
        s.hasLoad = true;
        const json plugins = in["load"].value("plugins", json::object());
        for (const auto& [id, p] : plugins.items()) s.pluginLoads[id] = {p[0].get<double>(), p[1].get<int64_t>()};
        s.processBytes = in["load"].value("process", static_cast<int64_t>(-1));
    }
    return s;
}

// Serve.cpp's onState body, from `peak` to the `frame`'s last field.
json frame(const EngineHost::EngineState& s) {
    const auto peak = [](float v) { return std::round(static_cast<double>(v) * 1e4) / 1e4; };
    const auto levels = [](const std::vector<float>& db) {
        json arr = json::array();
        for (const float v : db) arr.push_back(std::round(static_cast<double>(v) * 100.0) / 100.0);
        return arr;
    };
    json wf = json::object();
    for (const auto& [key, envelopes] : s.waveformEnvelopes) {
        json arr = json::array();
        for (const auto& e : envelopes) arr.push_back({{"min", peak(e.min)}, {"max", peak(e.max)}});
        wf[key] = std::move(arr);
    }
    json recs = json::object();
    for (const auto& [key, r] : s.recorders) {
        json takes = json::array();
        for (const auto& t : r.takes) takes.push_back({{"file", t.file}, {"seconds", t.seconds}, {"damaged", t.damaged}});
        json env = json::array();
        for (const auto& e : r.envelope) env.push_back({{"min", peak(e.min)}, {"max", peak(e.max)}});
        recs[key] = json{{"recording", r.recording},
                         {"seconds", r.seconds},
                         {"takes", std::move(takes)},
                         {"envelope", std::move(env)},
                         {"from", r.envelopeFrom}};
    }
    json load = nullptr;
    if (s.hasLoad) {
        json plugins = json::object();
        for (const auto& [id, l] : s.pluginLoads) {
            json p = {{"cpu", l.cpu}};
            if (l.bytes >= 0) p["bytes"] = l.bytes;
            plugins[id] = std::move(p);
        }
        load = {{"plugins", std::move(plugins)}};
        if (s.processBytes >= 0) load["process"] = s.processBytes;
    }
    json out = json{
        {"type", "state"},
        {"session", s.session},
        {"playing", s.playing},
        {"playhead", s.playheadSeconds},
        {"loop", {{"on", s.loop.on}, {"start", s.loop.startSeconds}, {"end", s.loop.endSeconds}}},
        {"levels", levels(s.trackLevelsDb)},
        {"inputLevels", levels(s.trackInputLevelsDb)},
        {"armed", s.armedTracks},
        {"recordEnabled", s.recordEnabled},
        {"master", s.masterLevelDb},
        {"masterClipped", s.masterOverload},
        {"waveform", std::move(wf)},
        {"platters", s.platterSeconds},
        {"recorders", std::move(recs)},
    };
    if (!load.is_null()) out["load"] = std::move(load);
    return out;
}

struct Line {
    std::string input, expected;
};

std::vector<Line> fixture() {
    std::vector<Line> out;
    std::ifstream f(K7_STATE_FIXTURE);
    for (std::string line; std::getline(f, line);) {
        const auto tab = line.find('\t');
        if (tab == std::string::npos) continue;
        out.push_back({line.substr(0, tab), line.substr(tab + 1)});
    }
    return out;
}

void everyCase() {
    const auto lines = fixture();
    CHECK(lines.size() >= 5);
    if (const char* write = std::getenv("K7_STATE_FIXTURE_WRITE")) {
        std::ofstream out(write);
        for (const auto& l : lines) out << l.input << '\t' << dump(frame(snapshot(json::parse(l.input)))) << '\n';
        return;
    }
    for (const auto& l : lines) {
        const std::string got = dump(frame(snapshot(json::parse(l.input))));
        if (got != l.expected) std::printf("  got %s\n  want %s\n", got.c_str(), l.expected.c_str());
        CHECK(got == l.expected);
    }
}

}  // namespace

int main() {
    RUN(everyCase);
    TEST_MAIN_END();
}
