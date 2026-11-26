// PluginReplies_test.cpp — the plugin replies as server/ServePlugins.cpp builds them, held to the fixture the Rust
// server's replies are held to too (apps/k7-daw-backend/packages/k7-runtime-server/fixtures/plugin_replies.txt): one
// case per line, a spec as JSON, a tab, and the reply's exact bytes. The builders below are that code, copied.
// The one thing a reader would get wrong: the fixture is this test's output, so it proves the fixture, not
// ServePlugins.cpp; K7_PLUGIN_FIXTURE_WRITE=<path> rewrites the expected column from the specs.

#include "EngineHost.h"
#include "test_harness.h"

#include <nlohmann/json.hpp>

#include <algorithm>
#include <cctype>
#include <cstdlib>
#include <fstream>
#include <string>
#include <vector>

namespace {

using json = nlohmann::json;

std::string dump(const json& j) { return j.dump(-1, ' ', false, json::error_handler_t::replace); }

json pluginsJson(const std::vector<studio::PluginInfo>& plugins, const std::string& filter, bool scanning) {
    const auto lower = [](std::string s) {
        for (auto& ch : s) ch = (char)std::tolower((unsigned char)ch);
        return s;
    };
    const std::string want = lower(filter);
    json arr = json::array();
    for (const auto& p : plugins) {
        if (!want.empty() && lower(p.format).find(want) == std::string::npos) continue;
        arr.push_back({
            {"name", p.name},
            {"format", p.format},
            {"manufacturer", p.manufacturer},
            {"instrument", p.isInstrument},
            {"role", p.role},
            {"category", p.category},
            {"id", p.identifier},
        });
    }
    return json{{"type", "plugins"}, {"plugins", arr}, {"scanning", scanning}};
}

json scanJson(const json& s) {
    return json{{"type", "plugin-scan"},
                {"scanning", s["scanning"]},
                {"done", s["done"]},
                {"total", s["total"]},
                {"current", s["current"]},
                {"version", s["version"]}};
}

json statesJson(const std::vector<std::pair<std::string, double>>& states) {
    json arr = json::array();
    for (const auto& [label, value] : states) arr.push_back({{"label", label}, {"value", value}});
    return arr;
}

std::vector<std::pair<std::string, double>> statesOf(const json& p) {
    std::vector<std::pair<std::string, double>> out;
    for (const auto& s : p.value("states", json::array())) out.emplace_back(s["label"].get<std::string>(), s["value"].get<double>());
    return out;
}

json paramList(const json& in) {
    json arr = json::array();
    const bool found = in["found"].get<bool>();
    if (found)
        for (const auto& p : in["params"]) {
            json one{{"name", p["name"]}, {"min", p["min"]}, {"max", p["max"]}};
            if (p.contains("def")) one["def"] = p["def"];
            const auto states = statesOf(p);
            if (!states.empty()) one["states"] = statesJson(states);
            arr.push_back(one);
        }
    return json{{"type", "plugin-param-list"}, {"format", in["format"]}, {"id", in["id"]}, {"found", found}, {"params", arr}};
}

json editPlugins(const json& in) {
    json arr = json::array();
    for (const auto& p : in["plugins"])
        arr.push_back({{"session", p["session"]}, {"id", p["id"]}, {"name", p["name"]}, {"hasNativePluginUI", p["hasNativePluginUI"]}});
    return json{{"type", "edit-plugins"}, {"plugins", arr}};
}

json presets(const json& in) {
    json arr = json::array();
    for (const auto& p : in["presets"])
        arr.push_back({{"name", p["name"]}, {"source", p["source"]}, {"index", p["index"]}, {"path", p["path"]}});
    json j{{"type", "presets"}, {"module", in["module"]}, {"presets", arr}};
    j["session"] = in["session"];
    return j;
}

json getParams(const json& in) {
    json params = json::object();
    json meta = json::object();
    for (const auto& p : in["infos"]) {
        params[p["name"].get<std::string>()] = p["value"];
        json m{{"min", p["min"]}, {"max", p["max"]}};
        if (p.contains("def")) m["def"] = p["def"];
        const auto states = statesOf(p);
        if (!states.empty()) m["states"] = statesJson(states);
        meta[p["name"].get<std::string>()] = m;
    }
    json j{{"type", "plugin-params"}, {"id", in["id"]}, {"params", params}, {"meta", meta}};
    j["session"] = in["session"];
    return j;
}

json build(const json& in) {
    const std::string kind = in["kind"];
    if (kind == "plugins") {
        std::vector<studio::PluginInfo> list;
        for (const auto& p : in["plugins"]) {
            studio::PluginInfo i;
            i.name = p["name"];
            i.format = p["format"];
            i.manufacturer = p["manufacturer"];
            i.isInstrument = p["instrument"];
            i.role = p["role"];
            i.category = p["category"];
            i.identifier = p["id"];
            list.push_back(i);
        }
        return pluginsJson(list, in.value("filter", std::string{}), in["scanning"]);
    }
    if (kind == "scan") return scanJson(in);
    if (kind == "param-list") return paramList(in);
    if (kind == "edit-plugins") return editPlugins(in);
    if (kind == "presets") return presets(in);
    if (kind == "get-params") return getParams(in);
    if (kind == "opened")
        return json{{"type", "native-plugin-ui-opened"}, {"format", in["format"]}, {"id", in["id"]}, {"ok", in["ok"]}};
    if (kind == "base-saved")
        return json{{"type", "plugin-base-saved"}, {"format", in["format"]}, {"id", in["id"]}, {"path", in["path"]}, {"ok", in["ok"]}};
    return json{{"type", "error"}, {"message", in["message"]}};
}

struct Line {
    std::string input, expected;
};

std::vector<Line> fixture() {
    std::vector<Line> out;
    std::ifstream f(K7_PLUGIN_FIXTURE);
    for (std::string line; std::getline(f, line);) {
        const auto tab = line.find('\t');
        if (tab == std::string::npos) continue;
        out.push_back({line.substr(0, tab), line.substr(tab + 1)});
    }
    return out;
}

void everyCase() {
    const auto lines = fixture();
    CHECK(lines.size() >= 12);
    if (const char* write = std::getenv("K7_PLUGIN_FIXTURE_WRITE")) {
        std::ofstream out(write);
        for (const auto& l : lines) out << l.input << '\t' << dump(build(json::parse(l.input))) << '\n';
        return;
    }
    for (const auto& l : lines) {
        const std::string got = dump(build(json::parse(l.input)));
        if (got != l.expected) std::printf("  got %s\n  want %s\n", got.c_str(), l.expected.c_str());
        CHECK(got == l.expected);
    }
}

}  // namespace

int main() {
    RUN(everyCase);
    TEST_MAIN_END();
}
