// SlotMidi.cpp — the slot's footswitch inputs: every MIDI input open while bound, a pedal plugged later picked up
// on the next look. See docs/sessions.md, "The slot".

#include "SlotMidi.h"

#include <iostream>

namespace studio {

namespace {
// How often a pedal plugged after the slot started is looked for: the MIDI rescan's own interval.
constexpr int kLookMs = 4000;
}  // namespace

SlotMidi::~SlotMidi() {
    stopTimer();
    closeAll();
}

void SlotMidi::bind(int cc, std::function<void()> onPress) {
    closeAll();
    stopTimer();
    // Written before any input opens, so no callback ever runs against the previous one.
    onPress_ = std::move(onPress);
    edge_ = PressEdge{};
    cc_ = cc;
    if (cc < 0 || cc > 127) {
        cc_ = -1;
        return;
    }
    openNew();
    startTimer(kLookMs);
}

void SlotMidi::openNew() {
    for (const auto& d : juce::MidiInput::getAvailableDevices()) {
        bool open = false;
        for (const auto& in : inputs_)
            if (in->getIdentifier() == d.identifier) open = true;
        if (open) continue;
        // On Windows an input the browser holds refuses a second owner: the pedal is then not heard, said once.
        auto in = juce::MidiInput::openDevice(d.identifier, this);
        if (!in) {
            std::cerr << "[slot] footswitch: '" << d.name << "' could not be opened\n";
            continue;
        }
        in->start();
        std::cerr << "[slot] footswitch listening on '" << d.name << "', CC " << cc_.load() << "\n";
        inputs_.push_back(std::move(in));
    }
}

void SlotMidi::closeAll() {
    for (auto& in : inputs_) in->stop();
    inputs_.clear();
}

void SlotMidi::handleIncomingMidiMessage(juce::MidiInput*, const juce::MidiMessage& m) {
    if (!m.isController() || m.getControllerNumber() != cc_.load()) return;
    if (edge_.press(m.getControllerValue()) && onPress_) onPress_();
}

}  // namespace studio
