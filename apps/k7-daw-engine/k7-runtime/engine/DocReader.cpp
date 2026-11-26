// DocReader.cpp — see DocReader.h. The field names are @studio/engine-protocol's ProjectDoc, as the Rust
// server writes them: every field is present, so every read is an `at`.

#include "DocReader.h"

#include <string>
#include <utility>
#include <vector>

using json = nlohmann::json;

namespace studio {

namespace {

template <typename T>
T get(const json& j, const char* key) {
    return j.at(key).get<T>();
}

// A list of pairs, (beat or second, value), as warp markers and automation points.
std::vector<std::pair<double, double>> readPairs(const json& list) {
    std::vector<std::pair<double, double>> out;
    for (const auto& p : list) out.emplace_back(p.at(0).get<double>(), p.at(1).get<double>());
    return out;
}

ClipDoc readClip(const json& c) {
    ClipDoc cl;
    cl.id = get<std::string>(c, "id");
    cl.type = get<std::string>(c, "type") == "midi" ? ClipType::Midi : ClipType::Audio;
    cl.name = get<std::string>(c, "name");
    cl.source = get<std::string>(c, "source");
    cl.start = get<double>(c, "start");
    cl.length = get<double>(c, "length");
    cl.offset = get<double>(c, "offset");
    cl.sourceFrom = get<double>(c, "sourceFrom");
    cl.sourceTo = get<double>(c, "sourceTo");
    cl.fadeIn = get<double>(c, "fadeIn");
    cl.fadeOut = get<double>(c, "fadeOut");
    cl.gainDb = get<float>(c, "gainDb");
    cl.crossfadeIn = get<bool>(c.at("crossfade"), "in");
    cl.crossfadeOut = get<bool>(c.at("crossfade"), "out");
    cl.loopStart = get<double>(c, "loopStart");
    cl.loopEnd = get<double>(c, "loopEnd");
    cl.disabled = get<bool>(c, "disabled");
    cl.warp.algorithm = get<std::string>(c.at("warp"), "algorithm");
    cl.warp.markers = readPairs(c.at("warp").at("markers"));
    for (const auto& n : c.at("notes")) {
        NoteDoc nd;
        nd.time = get<double>(n, "time");
        nd.duration = get<double>(n, "duration");
        nd.key = get<int>(n, "key");
        nd.vel = get<double>(n, "vel");
        nd.channel = get<int>(n, "channel");
        nd.rel = get<double>(n, "rel");
        nd.repeat = get<int>(n, "repeat");
        nd.skip = get<int>(n, "skip");
        cl.notes.push_back(nd);
    }
    return cl;
}

}  // namespace

PluginDoc readPlugin(const json& p) {
    PluginDoc pl;
    pl.id = get<std::string>(p, "id");
    pl.type = get<std::string>(p, "type");
    pl.name = get<std::string>(p, "name");
    pl.path = get<std::string>(p, "path");
    pl.presetSave = get<std::string>(p, "presetSave");
    pl.role = get<std::string>(p, "role");
    pl.enabled = get<bool>(p, "enabled");
    pl.namFile = get<std::string>(p, "namFile");
    pl.irFile = get<std::string>(p, "irFile");
    pl.deckFile = get<std::string>(p, "deckFile");
    pl.sfzFile = get<std::string>(p, "sfzFile");
    pl.surgePatch = get<std::string>(p, "surgePatch");
    pl.dspGraph = get<std::string>(p, "dspGraph");
    pl.patternSource = get<std::string>(p, "patternSource");
    pl.patternLabel = get<std::string>(p, "patternLabel");
    pl.patternCatchAll = get<bool>(p, "patternCatchAll");
    pl.patternClaimed = get<std::vector<std::string>>(p, "patternClaimed");
    pl.patternPlays = get<std::string>(p, "patternPlays");
    pl.patternOnly = get<bool>(p, "patternOnly");
    pl.patternBanks = get<std::vector<std::string>>(p, "patternBanks");
    pl.sidechainFrom = get<std::string>(p, "sidechainFrom");
    for (const auto& o : p.at("tapOuts")) pl.tapOuts.push_back({get<int>(o, "note"), get<int>(o, "pair")});
    for (const auto& [k, v] : p.at("params").items()) pl.params[k] = v.get<double>();
    for (const auto& e : p.at("rack")) {
        RackEntry re;
        re.symbol = get<std::string>(e, "symbol");
        re.note = get<int>(e, "note");
        re.clip = get<std::string>(e, "clip");
        re.start = get<double>(e, "start");
        re.length = get<double>(e, "length");
        pl.rack.push_back(std::move(re));
    }
    return pl;
}

TrackDoc readTrack(const json& t) {
    TrackDoc tr;
    tr.id = get<std::string>(t, "id");
    tr.name = get<std::string>(t, "name");
    tr.role = get<std::string>(t, "role");
    tr.color = get<std::string>(t, "color");
    tr.mute = get<bool>(t, "mute");
    tr.solo = get<bool>(t, "solo");
    tr.disabled = get<bool>(t, "disabled");
    tr.monitor = get<std::string>(t, "monitor");
    tr.inputGain = get<double>(t, "inputGain");
    tr.inputChannels = get<std::vector<int>>(t, "inputChannels");
    tr.outputChannels = get<std::vector<int>>(t, "outputChannels");
    for (const auto& c : t.at("clips")) tr.clips.push_back(readClip(c));
    for (const auto& p : t.at("plugins")) tr.plugins.push_back(readPlugin(p));
    for (const auto& s : t.at("sends")) {
        SendDoc sd;
        sd.to = get<std::string>(s, "to");
        sd.level = get<double>(s, "level");
        sd.enabled = get<bool>(s, "enabled");
        tr.sends.push_back(std::move(sd));
    }
    tr.inputFromPlugin = get<std::string>(t.at("inputFrom"), "plugin");
    tr.inputFromPair = get<int>(t.at("inputFrom"), "pair");
    for (const auto& a : t.at("automation")) {
        AutomationDoc au;
        au.param = get<std::string>(a, "param");
        au.pluginId = get<std::string>(a, "pluginId");
        au.bipolar = get<bool>(a, "bipolar");
        au.points = readPairs(a.at("points"));
        tr.automation.push_back(std::move(au));
    }
    return tr;
}

ProjectDoc readDoc(const json& j) {
    ProjectDoc doc;
    doc.name = get<std::string>(j, "name");
    doc.tempo = get<double>(j, "tempo");
    doc.sampleRate = get<int>(j, "sampleRate");
    doc.timeSigNumerator = get<int>(j, "timeSigNumerator");
    doc.timeSigDenominator = get<int>(j, "timeSigDenominator");
    doc.masterVolume = get<float>(j, "masterVolume");
    for (const auto& p : j.at("masterPlugins")) doc.masterPlugins.push_back(readPlugin(p));
    doc.recordDir = get<std::string>(j, "recordDir");
    doc.recordMute = get<bool>(j, "recordMute");
    const auto& s = j.at("settings");
    doc.metronome = get<bool>(s, "metronome");
    doc.metronomeLevel = get<double>(s, "metronomeLevel");
    doc.countIn = get<int>(s, "countIn");
    for (const auto& t : j.at("tracks")) doc.tracks.push_back(readTrack(t));
    return doc;
}

SlotFootswitch readFootswitch(const json& doc) {
    SlotFootswitch fs;
    if (!doc.contains("footswitch")) return fs;
    const auto& f = doc.at("footswitch");
    fs.cc = get<int>(f, "cc");
    fs.plugins = get<std::vector<std::string>>(f, "plugins");
    return fs;
}

}  // namespace studio
