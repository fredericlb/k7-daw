// SlotMidi.h — the MIDI inputs the engine opens for the slot's footswitch, and only while a slot with one plays:
// every other time the keyboards are the browser's (docs/audioDevice.md). See docs/sessions.md, "The slot".
// What reads wrong: these are JUCE's own inputs, not Tracktion's, so nothing they receive reaches a track.

#pragma once

#include "SlotFootswitch.h"

#include <juce_audio_devices/juce_audio_devices.h>

#include <atomic>
#include <functional>
#include <memory>
#include <vector>

namespace studio {

class SlotMidi : private juce::MidiInputCallback, private juce::Timer {
public:
    /** Listen for CC `cc` on every input, `onPress` called on the MIDI thread at each press; cc < 0 closes them. */
    void bind(int cc, std::function<void()> onPress);
    ~SlotMidi() override;

private:
    void openNew();
    void closeAll();
    void timerCallback() override { openNew(); }
    void handleIncomingMidiMessage(juce::MidiInput*, const juce::MidiMessage& m) override;

    std::vector<std::unique_ptr<juce::MidiInput>> inputs_;
    std::atomic<int> cc_{-1};
    std::function<void()> onPress_;
    // Read on the MIDI thread only: two pedals on one CC share it, which is what one pedal board means.
    PressEdge edge_;
};

}  // namespace studio
