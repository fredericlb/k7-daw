#pragma once

// A last stage on the device's output that writes silence, set when STUDIO_ENGINE_SILENT=1 (docs/audioDevice.md).
// What a reader gets wrong: nothing else changes — the device is opened and clocks the engine as ever, the graph,
// meters and taps run before this stage, so a test sees the same engine and only the speakers hear nothing.

#include "EngineTypes.h"

#include <cstdlib>
#include <memory>
#include <string>

namespace studio {

class SilentOutput final : public juce::AudioProcessor {
public:
    const juce::String getName() const override { return "silent output"; }
    void prepareToPlay(double, int) override {}
    void releaseResources() override {}
    void processBlock(juce::AudioBuffer<float>& buffer, juce::MidiBuffer&) override { buffer.clear(); }
    double getTailLengthSeconds() const override { return 0.0; }
    bool acceptsMidi() const override { return false; }
    bool producesMidi() const override { return false; }
    juce::AudioProcessorEditor* createEditor() override { return nullptr; }
    bool hasEditor() const override { return false; }
    int getNumPrograms() override { return 1; }
    int getCurrentProgram() override { return 0; }
    void setCurrentProgram(int) override {}
    const juce::String getProgramName(int) override { return {}; }
    void changeProgramName(int, const juce::String&) override {}
    void getStateInformation(juce::MemoryBlock&) override {}
    void setStateInformation(const void*, int) override {}
};

// The stage to install, or null when the environment does not ask for silence.
inline std::unique_ptr<juce::AudioProcessor> silentOutputIfAsked() {
    const char* asked = std::getenv("STUDIO_ENGINE_SILENT");
    if (asked == nullptr || std::string(asked) != "1") return nullptr;
    return std::make_unique<SilentOutput>();
}

}  // namespace studio
