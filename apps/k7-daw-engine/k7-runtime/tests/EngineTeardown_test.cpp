// EngineTeardown_test.cpp — an EngineHost destroyed while its sessions play and record, and the transport cycled under it.
// The one thing a reader would get wrong: no audio device is opened, on purpose — its setup is persisted where the live engine reads it.

#include "EngineHost.h"
#include "MessagePump.h"
#include "ProjectDoc.h"
#include "test_harness.h"

#include <juce_gui_basics/juce_gui_basics.h>

#include <string>

using namespace studio;

namespace {

// One plain lane and one with a live input: a record roll with nothing armed still rolls, as the browser's MIDI capture does.
ProjectDoc song() {
    ProjectDoc doc;
    doc.name = "teardown";
    TrackDoc plain;
    plain.id = "plain";
    plain.name = "plain";
    plain.role = "regular";
    TrackDoc input = plain;
    input.id = "input";
    input.name = "input";
    input.inputChannels = {0};
    doc.tracks = {plain, input};
    return doc;
}

bool headless() { return juce::Desktop::getInstance().isHeadless(); }

// The transport's own word that it rolls: Tracktion keeps the screen on for a record roll alone, not for a play.
void roll(EngineHost& host, const std::string& session, bool record) {
    host.setRecordEnabled(session, record);
    host.setArmed(session, "input", true);
    host.play(session);
    pumpMessages(30);
    CHECK(host.isPlaying(session));
}

// The destructor of a host whose edit is RECORDING, five times over: every Edit dies with its transport rolling.
void teardownWhileRecording() {
    for (int i = 0; i < 5; ++i) {
        EngineHost host;
        host.applyFull("k7:a", song(), "");
        roll(host, "k7:a", true);
        if (!headless()) CHECK(!juce::Desktop::isScreenSaverEnabled());
    }
    CHECK(juce::Desktop::isScreenSaverEnabled());
}

// Play, record, stop and seek, many times on one host, then a reset mid-roll and a rebuild that plays again.
void transportCycles() {
    {
        EngineHost host;
        host.applyFull("k7:a", song(), "");
        for (int i = 0; i < 20; ++i) {
            roll(host, "k7:a", i % 2 == 0);
            host.seek("k7:a", 0.5 * i);
            host.stop("k7:a");
            // Asked at once: with no device no callback takes the playhead's stop, and Tracktion's timer would roll it again.
            CHECK(!host.isPlaying("k7:a"));
            pumpMessages(10);
        }
        CHECK(juce::Desktop::isScreenSaverEnabled());
        roll(host, "k7:a", true);
        host.reset("k7:a");
        pumpMessages(10);
        host.applyFull("k7:a", song(), "");
        roll(host, "k7:a", false);
    }
    CHECK(juce::Desktop::isScreenSaverEnabled());
}

// Two sessions recording at once and the host destroyed: each Edit gives back what it held, so none is left.
void twoSessionsRecording() {
    {
        EngineHost host;
        host.applyFull("k7:a", song(), "");
        host.applyFull("dj:b", song(), "");
        roll(host, "k7:a", true);
        roll(host, "dj:b", true);
        host.stop("k7:a");
        pumpMessages(10);
        if (!headless()) CHECK(!juce::Desktop::isScreenSaverEnabled());
    }
    CHECK(juce::Desktop::isScreenSaverEnabled());
}

}  // namespace

int main() {
    // One JUCE for the process, so Desktop can be asked between hosts; each host's own initialiser nests in it.
    juce::ScopedJuceInitialiser_GUI juce;
    RUN(teardownWhileRecording);
    RUN(transportCycles);
    RUN(twoSessionsRecording);
    TEST_MAIN_END();
}
