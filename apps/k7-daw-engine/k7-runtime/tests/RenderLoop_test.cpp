// RenderLoop_test.cpp — an offline bounce of a looped clip finishes, and the loop is audible.
// The one thing a reader would get wrong: a stalled render returns false after 90 s, so a failure here is slow, not hung.

#include "EngineHost.h"
#include "ProjectDoc.h"
#include "test_harness.h"

#include <juce_audio_formats/juce_audio_formats.h>

#include <chrono>
#include <string>

using namespace studio;

namespace {

// A 0.1 s mono click, 48 kHz: shorter than the loop window, so the repeats are separated by silence.
juce::File writeClick(const juce::File& dir) {
    const auto file = dir.getChildFile("click.wav");
    juce::WavAudioFormat wav;
    std::unique_ptr<juce::OutputStream> out = file.createOutputStream();
    std::unique_ptr<juce::AudioFormatWriter> writer(wav.createWriterFor(out.get(), 48000.0, 1, 16, {}, 0));
    if (writer == nullptr) return {};
    out.release();
    juce::AudioBuffer<float> buf(1, 4800);
    for (int i = 0; i < 4800; ++i) buf.setSample(0, i, 0.5f * (i < 2400 ? 1.0f : -1.0f));
    writer->writeFromAudioSampleBuffer(buf, 0, buf.getNumSamples());
    return file;
}

float peakOf(const juce::File& wavFile) {
    juce::AudioFormatManager fm;
    fm.registerBasicFormats();
    std::unique_ptr<juce::AudioFormatReader> r(fm.createReaderFor(wavFile));
    if (r == nullptr) return 0.0f;
    juce::AudioBuffer<float> buf(static_cast<int>(r->numChannels), static_cast<int>(r->lengthInSamples));
    r->read(&buf, 0, buf.getNumSamples(), 0, true, true);
    return buf.getMagnitude(0, buf.getNumSamples());
}

ProjectDoc loopedSong(const juce::File& click, double loopEnd) {
    ProjectDoc doc;
    doc.name = "loop";
    doc.tempo = 120.0;
    doc.sampleRate = 48000;
    TrackDoc t;
    t.id = "drums";
    t.name = "drums";
    t.role = "regular";
    ClipDoc c;
    c.id = "click@0";
    c.name = "click";
    c.source = click.getFullPathName().toStdString();
    c.length = 4.0;
    c.loopStart = 0.0;
    c.loopEnd = loopEnd;
    t.clips = {c};
    doc.tracks = {t};
    return doc;
}

// A one-beat loop over a placement four beats long: the bounce ends in seconds, with sound in it.
void loopedClipRendersAndEnds() {
    const auto dir = juce::File::getSpecialLocation(juce::File::tempDirectory).getChildFile("k7-render-loop-test");
    dir.deleteRecursively();
    dir.createDirectory();
    const auto click = writeClick(dir);
    CHECK(click.existsAsFile());

    const auto began = std::chrono::steady_clock::now();
    const int rc = renderOffline(loopedSong(click, 1.0), "", dir.getChildFile("out").getFullPathName().toStdString(), false);
    const double seconds = std::chrono::duration<double>(std::chrono::steady_clock::now() - began).count();

    CHECK(rc == 0);
    CHECK(seconds < 30.0);
    CHECK(peakOf(dir.getChildFile("out").getChildFile("mix.wav")) > 0.1f);
    dir.deleteRecursively();
}

}  // namespace

int main() {
    RUN(loopedClipRendersAndEnds);
    TEST_MAIN_END();
}
