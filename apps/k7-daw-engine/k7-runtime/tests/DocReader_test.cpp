// DocReader_test.cpp — the plain reader holds the document the Rust server validated: the golden file both sides
// are held to (k7-runtime-server's fixtures/project.validated.json, written by its doc_bounds test) read field
// by field. JUCE-free: DocReader.cpp and ProjectDoc.cpp compile straight in.

#include "DocReader.h"
#include "test_harness.h"

#include <fstream>
#include <stdexcept>
#include <string>

using namespace studio;
using json = nlohmann::json;

namespace {

json golden() {
    std::ifstream in(K7_GOLDEN_DOC);
    if (!in) throw std::runtime_error(std::string("no golden file at ") + K7_GOLDEN_DOC);
    return json::parse(in);
}

void the_document_level_fields_are_read() {
    const auto doc = readDoc(golden());
    CHECK(doc.name == "golden");
    CHECK(doc.tempo == 120.0);
    CHECK(doc.sampleRate == 48000);
    CHECK(doc.timeSigNumerator == 4 && doc.timeSigDenominator == 4);
    CHECK(doc.masterVolume == -3.5f);
    CHECK(doc.recordDir.empty());
    CHECK(!doc.recordMute);
    CHECK(doc.metronome && doc.metronomeLevel == 0.8 && doc.countIn == 2);
    CHECK(doc.masterPlugins.size() == 1);
    CHECK(doc.masterPlugins.at(0).id == "limit" && !doc.masterPlugins.at(0).enabled);
    CHECK(doc.tracks.size() == 2);
}

void a_track_clips_notes_and_warp_are_read() {
    const auto doc = readDoc(golden());
    const auto& t = doc.tracks.at(0);
    CHECK(t.id == "t" && t.name == "Bass" && t.role == "effect" && t.color == "#fff");
    CHECK(t.mute && !t.solo && !t.disabled && t.monitor == "on" && t.inputGain == 1.5);
    CHECK((t.inputChannels == std::vector<int>{1, 3}));
    CHECK((t.outputChannels == std::vector<int>{3, 4}));
    CHECK(t.clips.size() == 3);
    const auto& c = t.clips.at(0);
    CHECK(c.id == "c" && c.type == ClipType::Midi && c.name == "verse");
    CHECK(c.start == 4.0 && c.length == 8.0 && c.offset == 1.0 && c.fadeIn == 0.5);
    CHECK(c.crossfadeIn && !c.crossfadeOut);
    CHECK(c.loopStart == 1.0 && c.loopEnd == 2.0 && c.disabled);
    CHECK(c.warp.algorithm == "slice" && c.warp.markers.size() == 2 && c.warp.markers.at(1).second == 2.0);
    const auto& n = c.notes.at(0);
    CHECK(n.key == 127 && n.repeat == 64 && n.skip == 0 && n.channel == 2 && n.vel == 0.5 && n.rel == 0.25);
    CHECK(t.clips.at(1).id == "c#2" && t.clips.at(1).type == ClipType::Audio);
    CHECK(t.clips.at(1).source == "/music/a.wav" && t.clips.at(1).gainDb == -2.5f);
    CHECK(t.clips.at(1).sourceFrom == 0.5 && t.clips.at(1).sourceTo == 2.5);
    CHECK(t.clips.at(2).source.empty());
}

void a_plugin_with_every_field_is_read() {
    const auto doc = readDoc(golden());
    const auto& p = doc.tracks.at(0).plugins.at(0);
    CHECK(p.id == "fx" && p.type == "4osc" && p.name == "Four" && p.enabled);
    CHECK(p.path == "/music/p.vstpreset" && p.presetSave == "/music/rec/p.vstpreset");
    CHECK(p.namFile.empty() && p.irFile == "/music/ir.wav" && p.deckFile.empty());
    CHECK(p.surgePatch == "/music/s.fxp" && p.dspGraph == "{}");
    CHECK(p.patternSource == "s(\"bd\")" && p.patternLabel == "drums" && p.patternCatchAll && !p.patternOnly);
    CHECK((p.patternClaimed == std::vector<std::string>{"a", "b"}) && p.patternPlays == "both");
    CHECK((p.patternBanks == std::vector<std::string>{"/music/banks"}) && p.sidechainFrom == "kick");
    CHECK(p.tapOuts.size() == 1 && p.tapOuts.at(0).note == 127 && p.tapOuts.at(0).pair == 31);
    CHECK(p.params.at("cutoff") == 0.25 && p.params.at("res") == 1.0);
    CHECK(p.rack.size() == 2 && p.rack.at(0).symbol == "BD" && p.rack.at(0).note == 0 && p.rack.at(0).length == 1.0);
}

void sends_input_from_and_automation_are_read() {
    const auto doc = readDoc(golden());
    const auto& t = doc.tracks.at(0);
    CHECK(t.sends.size() == 1 && t.sends.at(0).to == "r" && t.sends.at(0).enabled && t.sends.at(0).level == 0.5);
    CHECK(t.inputFromPlugin == "fx" && t.inputFromPair == 0);
    CHECK(t.automation.size() == 1 && t.automation.at(0).bipolar && t.automation.at(0).points.size() == 2);
    CHECK(doc.tracks.at(1).id == "t#2" && doc.tracks.at(1).plugins.at(0).id == "fx#2");
}

void the_footswitch_rides_the_document_and_is_unbound_without_one() {
    const auto fs = readFootswitch(golden());
    CHECK(fs.cc == 64 && fs.plugins.size() == 1 && fs.plugins.at(0) == "fx" && fs.bound());
    auto bare = golden();
    bare.erase("footswitch");
    CHECK(!readFootswitch(bare).bound());
}

// A field the writer forgot is a throw, not a default that plays silently.
void a_missing_field_is_refused() {
    auto bad = golden();
    bad["tracks"][0].erase("clips");
    bool refused = false;
    try {
        readDoc(bad);
    } catch (const std::exception&) {
        refused = true;
    }
    CHECK(refused);
}

}  // namespace

int main() {
    RUN(the_document_level_fields_are_read);
    RUN(a_track_clips_notes_and_warp_are_read);
    RUN(a_plugin_with_every_field_is_read);
    RUN(sends_input_from_and_automation_are_read);
    RUN(the_footswitch_rides_the_document_and_is_unbound_without_one);
    RUN(a_missing_field_is_refused);
    TEST_MAIN_END();
}
