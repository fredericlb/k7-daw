# tracktion.cmake — the fixes Tracktion needs (its SamplerPlugin, its file-check queue, its NaN-only sanitising), applied at fetch time.
# The one thing a reader would get wrong: its update DOES try to reuse decoded audio, but only after decoding every sound anyway.

include("${CMAKE_CURRENT_LIST_DIR}/../eurorack/patches/studio_replace.cmake")

# The constructor sizes the excerpt, the update decodes only what no old sound can lend, and nothing re-decodes under the audio lock (docs/projectDiff.md).
set(dir "modules/tracktion_engine/plugins/effects")
studio_replace("${dir}/tracktion_SamplerPlugin.h"
    "void setExcerpt (double startTime, double length);"
    "void setExcerpt (double startTime, double length, bool decode = true);")
studio_replace("${dir}/tracktion_SamplerPlugin.h"
    "juce::AudioBuffer<float> audioData { 2, 64 };\n\n    private:"
    "juce::AudioBuffer<float> audioData { 2, 64 };\n        bool decoded = false;   // studio: audioData holds the excerpt\n\n    private:")
set(cpp "${dir}/tracktion_SamplerPlugin.cpp")
studio_replace("${cpp}"
    "void SamplerPlugin::SamplerSound::setExcerpt (double startTime_, double length_)"
    "void SamplerPlugin::SamplerSound::setExcerpt (double startTime_, double length_, bool decode)")
studio_replace("${cpp}"
    "    setExcerpt (startTime_, length_);"
    "    setExcerpt (startTime_, length_, false);   // studio: decoded by the update, and only if no old sound holds it")
studio_replace("${cpp}"
    "if (auto reader = owner.engine.getAudioFileManager().cache.createReader (audioFile))"
    "if (auto reader = decode ? owner.engine.getAudioFileManager().cache.createReader (audioFile) : nullptr)")
studio_replace("${cpp}"
    "            int total = fileLengthSamples;\n            int offset = 0;"
    "            decoded = true;   // studio\n            int total = fileLengthSamples;\n            int offset { 0 };")
studio_replace("${cpp}"
    "    for (auto newSound : newSounds)\n    {\n        for (auto s : soundList)\n        {\n            if (s->source == newSound->source\n                && s->startTime == newSound->startTime\n                && s->length == newSound->length)\n            {\n                newSound->audioFile = s->audioFile;\n                newSound->fileStartSample = s->fileStartSample;\n                newSound->fileLengthSamples = s->fileLengthSamples;\n                newSound->audioData = s->audioData;\n            }\n        }\n    }\n\n    {\n        const juce::ScopedLock sl (lock);\n        allNotesOff();\n        soundList.swapWith (newSounds);\n\n        sourceMediaChanged();\n"
    "    // studio: MATCH each new sound to an old one holding its excerpt, decode only the rest, then MOVE the audio under the lock\n    std::vector<std::pair<SamplerSound*, SamplerSound*>> studioMoves;\n    std::map<SamplerSound*, SamplerSound*> studioOwner;\n    for (auto newSound : newSounds)\n        for (auto s : soundList)\n            if (s->decoded && s->source == newSound->source\n                && s->startTime == newSound->startTime\n                && s->length == newSound->length)\n            {\n                newSound->audioFile = s->audioFile;\n                newSound->fileStartSample = s->fileStartSample;\n                newSound->fileLengthSamples = s->fileLengthSamples;\n                // studio: the first taker gets the buffer moved; a second on the same excerpt copies it now, while it is whole\n                if (studioOwner.emplace (s, newSound).second)\n                    studioMoves.emplace_back (newSound, s);\n                else\n                    newSound->audioData = s->audioData;\n                newSound->decoded = true;\n                break;\n            }\n\n    for (auto newSound : newSounds)\n        if (! newSound->decoded)\n            newSound->setExcerpt (newSound->startTime, newSound->length);\n\n    {\n        const juce::ScopedLock sl (lock);\n        allNotesOff();\n        for (auto& m : studioMoves)\n            m.first->audioData = std::move (m.second->audioData);\n        soundList.swapWith (newSounds);   // studio: no sourceMediaChanged() here - it re-decoded every sound holding the lock the audio thread takes\n")

# The file-check callback reads the LAST entry of its queue unchecked; run on an empty queue it copies element -1 and the engine dies (docs/crashes.md).
studio_replace("modules/tracktion_engine/audio_files/tracktion_AudioFile.cpp"
    "        const juce::ScopedLock sl (knownFilesLock);\n        fileToCheck = filesToCheck.getUnchecked (filesToCheck.size() - 1);"
    "        const juce::ScopedLock sl (knownFilesLock);\n        if (filesToCheck.isEmpty())   // studio: a delivery with nothing queued\n            return;\n        fileToCheck = filesToCheck.getUnchecked (filesToCheck.size() - 1);")

# Every plugin's output is sanitised for NaN only, so ±Inf passes on and latches the next biquad into NaN: non-finite, not NaN (docs/audit-stabilite.md).
studio_replace("modules/tracktion_graph/utilities/tracktion_GlueCode.h"
    "return std::isnan (s) ? typename Buffer::Sample() : s;"
    "return std::isfinite (s) ? s : typename Buffer::Sample();   // studio: Inf too")

# When the audio callback now running ENTERED, so the engine's watchdog tells a plugin stuck inside it from a driver that stopped calling: reopening the device in the first case blocks for ever on the lock the stuck callback holds (docs/audioDevice.md).
studio_replace("modules/tracktion_engine/playback/tracktion_DeviceManager.h"
    "class HostedAudioDeviceInterface;\n\n\n//====="
    "class HostedAudioDeviceInterface;\n\n// studio: when the audio callback now running entered (Time::getMillisecondCounter, never 0), 0 when none is\nextern std::atomic<juce::uint32> studioCallbackEnteredMs;\n\n//=====")
studio_replace("modules/tracktion_engine/playback/tracktion_DeviceManager.cpp"
    "    clearStatsFlag.store (true, std::memory_order_relaxed);\n}\n\nvoid DeviceManager::audioDeviceIOCallbackWithContext"
    "    clearStatsFlag.store (true, std::memory_order_relaxed);\n}\n\n// studio: set for the length of each callback\nstd::atomic<juce::uint32> studioCallbackEnteredMs { 0 };\n\nstruct StudioCallbackScope\n{\n    StudioCallbackScope()  { studioCallbackEnteredMs.store (juce::Time::getMillisecondCounter() | 1u, std::memory_order_relaxed); }\n    ~StudioCallbackScope() { studioCallbackEnteredMs.store (0, std::memory_order_relaxed); }\n};\n\nvoid DeviceManager::audioDeviceIOCallbackWithContext")
studio_replace("modules/tracktion_engine/playback/tracktion_DeviceManager.cpp"
    "    CRASH_TRACER\n    juce::FloatVectorOperations::disableDenormalisedNumberSupport();\n\n   #if JUCE_ANDROID\n    const ScopedSteadyLoad"
    "    CRASH_TRACER\n    juce::FloatVectorOperations::disableDenormalisedNumberSupport();\n    const StudioCallbackScope studioScope;   // studio\n\n   #if JUCE_ANDROID\n    const ScopedSteadyLoad")

# A sound plays on an OUTPUT PAIR of its own when its state names one (`studioOutPair`), so a rack's tap can be routed
# apart from the rest; the plugin then reports two channels per pair. Its home is a RackType (engine docs/tapOutputs.md).
set(h "${dir}/tracktion_SamplerPlugin.h")
studio_replace("${h}"
    "    void setSoundGains (int index, float gainDb, float pan);\n    void setSoundOpenEnded"
    "    void setSoundGains (int index, float gainDb, float pan);\n    void setSoundOutputPair (int index, int pair);   // studio: 0 is the main pair\n    void setSoundOpenEnded")
studio_replace("${h}"
    "int getNumOutputChannelsGivenInputs (int numInputChannels) override { return juce::jmin (numInputChannels, 2); }"
    "int getNumOutputChannelsGivenInputs (int numInputChannels) override;   // studio: two per output pair its sounds name")
studio_replace("${h}"
    "        AudioFile audioFile;\n        juce::AudioBuffer<float> audioData { 2, 64 };"
    "        AudioFile audioFile;\n        int outPair = 0;   // studio: the output pair it plays on\n        juce::AudioBuffer<float> audioData { 2, 64 };")
studio_replace("${cpp}"
    "                 bool openEnded_)\n       : note (midiNote),\n"
    "                 bool openEnded_, int outPair_ = 0)\n       : note (midiNote), outPair (outPair_),   // studio\n")
studio_replace("${cpp}"
    "    int note;\n    int offset, samplesLeftToPlay = 0;"
    "    int note;\n    int outPair = 0;   // studio: the output pair it plays on\n    int offset, samplesLeftToPlay = 0;")
studio_replace("${cpp}"
    "ss->openEnded));"
    "ss->openEnded, ss->outPair));   // studio")
studio_replace("${cpp}"
    "            s->openEnded    = v[IDs::openEnded];\n\n            newSounds.add (s);"
    "            s->openEnded    = v[IDs::openEnded];\n            s->outPair      = juce::jlimit (0, 31, static_cast<int> (v[\"studioOutPair\"]));   // studio\n\n            newSounds.add (s);")
studio_replace("${cpp}"
    "            sn->addNextBlock (*fc.destBuffer, fc.bufferStartSample, fc.bufferNumSamples);"
    "            // studio: a note plays on its sound's pair when the buffer carries it (a RackType's), else on the first\n            const int studioOff = 2 * sn->outPair;\n            if (studioOff > 0 && studioOff + 2 <= fc.destBuffer->getNumChannels())\n            {\n                juce::AudioBuffer<float> studioPair (fc.destBuffer->getArrayOfWritePointers() + studioOff, 2, fc.destBuffer->getNumSamples());\n                sn->addNextBlock (studioPair, fc.bufferStartSample, fc.bufferNumSamples);\n            }\n            else sn->addNextBlock (*fc.destBuffer, fc.bufferStartSample, fc.bufferNumSamples);")
studio_replace("${cpp}"
    "}\n\nvoid SamplerPlugin::setSoundGains (int index, float gainDb, float pan)"
    "}\n\nint SamplerPlugin::getNumOutputChannelsGivenInputs (int numInputChannels)   // studio\n{\n    int pairs = 1;\n    for (auto v : state)\n        if (v.hasType (IDs::SOUND))\n            pairs = std::max (pairs, 1 + juce::jlimit (0, 31, static_cast<int> (v[\"studioOutPair\"])));\n    return std::max (juce::jmin (numInputChannels, 2), 2 * pairs);\n}\n\nvoid SamplerPlugin::setSoundOutputPair (int index, int pair)   // studio\n{\n    getSound (index).setProperty (\"studioOutPair\", juce::jlimit (0, 31, pair), getUndoManager());\n}\n// studio: the two above\n\nvoid SamplerPlugin::setSoundGains (int index, float gainDb, float pan)")
# …and NAMES for every channel those pairs make: a RackType refuses a connection from a pin past the plugin's names.
studio_replace("${h}"
    "two per output pair its sounds name\n    void initialise (const PluginInitialisationInfo&) override;"
    "two per output pair its sounds name\n    void getChannelNames (juce::StringArray* ins, juce::StringArray* outs) override;   // studio: a name per channel of every pair\n    void initialise (const PluginInitialisationInfo&) override;")
studio_replace("${cpp}"
    "}\n\nvoid SamplerPlugin::applyToBuffer (const PluginRenderContext& fc)"
    "}\n\nvoid SamplerPlugin::getChannelNames (juce::StringArray* ins, juce::StringArray* outs)   // studio\n{\n    if (ins != nullptr) { ins->add (\"Left\"); ins->add (\"Right\"); }\n    if (outs != nullptr)\n        for (int c = 0, n = getNumOutputChannelsGivenInputs (2); c < n; ++c)\n            outs->add ((c % 2 == 0 ? \"Left \" : \"Right \") + juce::String (c / 2 + 1));\n}\n// studio: getChannelNames above\n\nvoid SamplerPlugin::applyToBuffer (const PluginRenderContext& fc)")

# crill's armv8 spin-wait calls the ACLE __wfe(), which GCC 12 does not declare: a Linux arm64 build (the Pi image, docs/pi-engine.md) fails on it.
studio_replace("modules/3rd_party/crill/impl/progressive_backoff_wait_impl.h"
    "return;\n\n                __wfe();\n"
    "return;\n\n               #if defined(__GNUC__)   // studio: GCC has no __wfe\n                __asm__ __volatile__ (\"wfe\");\n               #else\n                __wfe();\n               #endif\n")

# juceaide builds in a nested CMake with /Zi, which sccache fails with C1041; on a Windows host it is cl.exe even under
# Android's clang, so it builds uncached there, the launcher passed EMPTY so a kept tools cache loses it (docs/release.md).
set(aide "modules/juce/extras/Build/juceaide/CMakeLists.txt")
# A source patched by the earlier `AND NOT MSVC` form is put back first, which studio_replace could not match.
file(READ "${aide}" aideText)
string(REPLACE " AND NOT MSVC)  # studio: juceaide's /Zi cannot go through sccache\n" ")\n" aideText "${aideText}")
file(WRITE "${aide}" "${aideText}")
foreach(lang C CXX)
    studio_replace("${aide}"
        "    if(CMAKE_${lang}_COMPILER_LAUNCHER)\n"
        "    if(CMAKE_HOST_WIN32)  # studio: juceaide's /Zi cannot go through sccache\n        list(APPEND PASSTHROUGH_ARGS \"-DCMAKE_${lang}_COMPILER_LAUNCHER=\")\n    elseif(CMAKE_${lang}_COMPILER_LAUNCHER)\n")
endforeach()
# …and its deployment target, which on iOS is an iPhone floor: juceaide runs on the Mac, where 17.0 is no macOS (docs/ios.md).
studio_replace("modules/juce/extras/Build/juceaide/CMakeLists.txt"
    "if(CMAKE_OSX_DEPLOYMENT_TARGET)\n"
    "if(CMAKE_OSX_DEPLOYMENT_TARGET AND NOT IOS)  # studio: an iOS floor is no macOS one\n")

# On Android Tracktion busy-waits each callback to 80% of its period: a core held, and an underrun a second (docs/android.md).
studio_replace("modules/tracktion_engine/playback/tracktion_ScopedSteadyLoad.h"
    "        float loadLevel         { 0.8f };\n"
    "        float loadLevel         { 0.0f }; // studio: no steady load, patches/tracktion.cmake\n")

# A record roll keeps the screen on through JUCE, which on a phone touches the app's window from the engine's thread: Android throws, and the engine's loop ends (docs/android.md).
set(ssd "modules/tracktion_engine/utilities/tracktion_ScreenSaverDefeater.cpp")
studio_replace("${ssd}"
    "static int numScreenSaverDefeaters = 0;\n"
    "static int numScreenSaverDefeaters = 0;\n\n// studio: on a phone the app owns the screen, and the engine's message thread is not its UI thread\n#if JUCE_ANDROID || JUCE_IOS\nstatic constexpr bool studioScreenIsTheApps = true;\n#else\nstatic constexpr bool studioScreenIsTheApps = false;\n#endif\n")
studio_replace("${ssd}"
    "    if (juce::Desktop::getInstance().isHeadless())\n        return;\n"
    "    if (studioScreenIsTheApps || juce::Desktop::getInstance().isHeadless())   // studio\n        return;\n")

# A Mac running the iPad app answers every route change with a restart of the device, and each restart makes macOS change the route again:
# the handler leaves the notification thread unblocked, one restart per window, the rate list is fixed (docs/ios.md).
set(native "modules/juce/modules/juce_audio_devices/native")
file(READ "${CMAKE_CURRENT_LIST_DIR}/../RestartCoalescer.h" coalescerText)
file(WRITE "${native}/studio_RestartCoalescer.h" "${coalescerText}")
set(ios "${native}/juce_Audio_ios.cpp")
studio_replace("${ios}"
    "namespace juce {\n\n#ifndef JUCE_IOS_AUDIO_LOGGING"
    "#include \"studio_RestartCoalescer.h\"\n#include <atomic>\n#include <chrono>\n#include <functional>\n\nnamespace juce {\n\n// studio: the Mac running the iPad app, and the clock of its restart rule\nstatic bool studioIsMac() { static const bool mac = [NSProcessInfo processInfo].isiOSAppOnMac; return mac; }\nstatic std::int64_t studioNowMs() { return std::chrono::duration_cast<std::chrono::milliseconds> (std::chrono::steady_clock::now().time_since_epoch()).count(); }\n// studio: when JUCE last restarted a device on a Mac (0 never), which the engine's watchdog reads\nstd::atomic<std::int64_t> studioIosRestartMs { 0 };\n\n#ifndef JUCE_IOS_AUDIO_LOGGING")
studio_replace("${ios}"
    "    void updateAvailableSampleRates()\n    {\n"
    "    void updateAvailableSampleRates()\n    {\n        // studio: the Mac's HAL answers to ~150 probes per restart with route changes; it runs at these two\n        if (studioIsMac())\n        {\n            availableSampleRates = Array<double> { 44100.0, 48000.0 };\n            return;\n        }\n\n")
studio_replace("${ios}"
    "    void handleRouteChange (AVAudioSessionRouteChangeReason reason)\n    {\n        const ScopedLock myScopedLock (callbackLock);\n"
    "    void handleRouteChange (AVAudioSessionRouteChangeReason reason)\n    {\n        // studio: on a Mac the UI thread only notes the change; the message thread decides, so this never waits on callbackLock\n        if (studioIsMac())\n        {\n            if (reason == AVAudioSessionRouteChangeReasonCategoryChange || reason == AVAudioSessionRouteChangeReasonRouteConfigurationChange)\n                return;\n            int none = 0;\n            if (reason == AVAudioSessionRouteChangeReasonUnknown) studioRouteKind.compare_exchange_strong (none, 1);\n            else studioRouteKind.store (2);\n            triggerAsyncUpdate();\n            return;\n        }\n\n        const ScopedLock myScopedLock (callbackLock);\n")
studio_replace("${ios}"
    "    void handleAsyncUpdate() override\n    {\n        restart();\n    }\n"
    "    void handleAsyncUpdate() override\n    {\n        if (! studioIsMac())\n        {\n            restart();\n            return;\n        }\n\n        // studio: one restart per window; an 'unknown' change just after one is its echo and is dropped (RestartCoalescer.h)\n        const ScopedLock sl (callbackLock);\n        const int kind = studioRouteKind.exchange (0);\n        const auto now = studioNowMs();\n        const auto verdict = studioCoalescer.request (now, kind == 1);\n\n        if (verdict == studio::RestartCoalescer::Verdict::drop)\n            return;\n\n        if (verdict == studio::RestartCoalescer::Verdict::later)\n        {\n            studioTimer.startTimer (static_cast<int> (studioCoalescer.waitMs (now)) + 1);\n            return;\n        }\n\n        if (kind != 0)\n            hardwareInfoNeedsUpdating = true;\n\n        restart();\n        studioCoalescer.restarted (studioNowMs());\n        studioIosRestartMs.store (studioNowMs());\n    }\n\n    // studio: a deferred restart's slot has come; the request is served unless a restart already did\n    void studioWake()\n    {\n        const ScopedLock sl (callbackLock);\n        if (! studioCoalescer.pending())\n            return;\n        studioRouteKind.store (2);\n        triggerAsyncUpdate();\n    }\n")
studio_replace("${ios}"
    "    CriticalSection callbackLock;\n"
    "    CriticalSection callbackLock;\n\n    // studio: the Mac's restart rule; touched under callbackLock\n    studio::RestartCoalescer studioCoalescer;\n    std::atomic<int> studioRouteKind { 0 };   // 0 none, 1 only 'unknown', 2 another reason\n    struct StudioTimer final : public Timer\n    {\n        explicit StudioTimer (std::function<void()> f) : fn (std::move (f)) {}\n        ~StudioTimer() override { stopTimer(); }\n        void timerCallback() override { stopTimer(); fn(); }\n        std::function<void()> fn;\n    };\n    StudioTimer studioTimer { [this] { studioWake(); } };\n")
