// Offline render (bounce to wav) — `--render`, and renderWith(), the bounce the serving engine's `render` shares.
// Device-free (mirrors dumpPluginParams): builds a bare Engine + Edit, no audio
// device, no playback context — te::Renderer pulls the graph directly. This is why
// MIDI-through-instrument tracks (which the ffmpeg render can't touch) come out here:
// populateEdit realises the 4OSC synth on the track. Where each step runs is RenderCore.h's.

#include "EngineHost.h"

#include "EngineTypes.h"     // Tracktion/JUCE + the te/tt/fs aliases
#include "SettingsStorage.h"
#include "PluginScan.h"      // registerBuiltInPlugins
#include "EditBuilder.h"     // populateEdit
#include "MessagePump.h"     // pumpMessages — the render loop's dispatch
#include "RenderCore.h"      // RenderSteps — where each step of a bounce runs

#include <atomic>
#include <chrono>
#include <cstdlib>
#include <functional>
#include <iostream>
#include <set>
#include <sstream>
#include <string>
#include <system_error>
#include <thread>
#include <vector>

#if JUCE_WINDOWS
 #include <windows.h>
 #include <dbghelp.h>
#endif

namespace studio {

namespace {

// Progress trace, off unless STUDIO_RENDER_TRACE is set: a stalled bounce otherwise prints NOTHING, which is indistinguishable from slow.
void trace(const char* what) {
    static const bool on = std::getenv("STUDIO_RENDER_TRACE") != nullptr;
    if (on) std::cerr << "[render:trace] " << what << std::endl;
}

#if JUCE_WINDOWS
// Watchdog: print THIS thread's symbolised stack if it is still here in `seconds`, gated on STUDIO_RENDER_TRACE — "Debugging a hang with no debugger" in AGENTS.md.
void watchdogStack(int seconds) {
    if (std::getenv("STUDIO_RENDER_TRACE") == nullptr) return;
    HANDLE stuck = nullptr;
    DuplicateHandle(GetCurrentProcess(), GetCurrentThread(), GetCurrentProcess(), &stuck,
                    0, FALSE, DUPLICATE_SAME_ACCESS);
    std::thread([stuck, seconds] {
        std::this_thread::sleep_for(std::chrono::seconds(seconds));
        auto proc = GetCurrentProcess();
        SymSetOptions(SYMOPT_DEFERRED_LOADS | SYMOPT_UNDNAME);
        SymInitialize(proc, nullptr, TRUE);
        SuspendThread(stuck);
        CONTEXT ctx{};
        ctx.ContextFlags = CONTEXT_FULL;
        if (GetThreadContext(stuck, &ctx)) {
            STACKFRAME64 f{};
            f.AddrPC.Offset = ctx.Rip;       f.AddrPC.Mode = AddrModeFlat;
            f.AddrFrame.Offset = ctx.Rbp;    f.AddrFrame.Mode = AddrModeFlat;
            f.AddrStack.Offset = ctx.Rsp;    f.AddrStack.Mode = AddrModeFlat;
            std::cerr << "[render:stack] stuck thread:\n";
            for (int i = 0; i < 50; ++i) {
                if (!StackWalk64(IMAGE_FILE_MACHINE_AMD64, proc, stuck, &f, &ctx, nullptr,
                                 SymFunctionTableAccess64, SymGetModuleBase64, nullptr) || f.AddrPC.Offset == 0)
                    break;
                char buf[sizeof(SYMBOL_INFO) + 512]{};
                auto* sym = reinterpret_cast<SYMBOL_INFO*>(buf);
                sym->SizeOfStruct = sizeof(SYMBOL_INFO);
                sym->MaxNameLen = 500;
                DWORD64 disp = 0;
                std::cerr << "  #" << i << " ";
                if (SymFromAddr(proc, f.AddrPC.Offset, &disp, sym)) std::cerr << sym->Name << "\n";
                else std::cerr << std::hex << f.AddrPC.Offset << std::dec << "\n";
            }
            std::cerr.flush();
        }
        ResumeThread(stuck);
        std::_Exit(3);
    }).detach();
}
#else
void watchdogStack(int) {}
#endif

}  // namespace

#if JUCE_WINDOWS
namespace {
// Walk and symbolise the faulting thread's stack from inside the exception filter: the context the OS hands us IS the faulting one, so there is no thread to suspend.
LONG WINAPI crashFilter(EXCEPTION_POINTERS* info) {
    auto proc = GetCurrentProcess();
    SymSetOptions(SYMOPT_DEFERRED_LOADS | SYMOPT_UNDNAME);
    SymInitialize(proc, nullptr, TRUE);
    std::cerr << "\n[crash] exception 0x" << std::hex
              << info->ExceptionRecord->ExceptionCode << std::dec << " — stack:\n";
    CONTEXT ctx = *info->ContextRecord;
    STACKFRAME64 f{};
    f.AddrPC.Offset = ctx.Rip;       f.AddrPC.Mode = AddrModeFlat;
    f.AddrFrame.Offset = ctx.Rbp;    f.AddrFrame.Mode = AddrModeFlat;
    f.AddrStack.Offset = ctx.Rsp;    f.AddrStack.Mode = AddrModeFlat;
    for (int i = 0; i < 60; ++i) {
        if (!StackWalk64(IMAGE_FILE_MACHINE_AMD64, proc, GetCurrentThread(), &f, &ctx, nullptr,
                         SymFunctionTableAccess64, SymGetModuleBase64, nullptr) || f.AddrPC.Offset == 0)
            break;
        char buf[sizeof(SYMBOL_INFO) + 512]{};
        auto* sym = reinterpret_cast<SYMBOL_INFO*>(buf);
        sym->SizeOfStruct = sizeof(SYMBOL_INFO);
        sym->MaxNameLen = 500;
        DWORD64 disp = 0;
        std::cerr << "  #" << i << " ";
        if (SymFromAddr(proc, f.AddrPC.Offset, &disp, sym)) std::cerr << sym->Name;
        else std::cerr << std::hex << f.AddrPC.Offset << std::dec;
        IMAGEHLP_LINE64 lineInfo{};
        lineInfo.SizeOfStruct = sizeof(lineInfo);
        DWORD lineDisp = 0;
        if (SymGetLineFromAddr64(proc, f.AddrPC.Offset, &lineDisp, &lineInfo))
            std::cerr << "  (" << lineInfo.FileName << ":" << lineInfo.LineNumber << ")";
        std::cerr << "\n";
    }
    std::cerr.flush();
    return EXCEPTION_CONTINUE_SEARCH;
}
}  // namespace

void installCrashHandler() { SetUnhandledExceptionFilter(crashFilter); }
#else
void installCrashHandler() {}
#endif

namespace {

// Filesystem-safe stem filename from a track name.
std::string sanitizeStem(const std::string& name, int index) {
    std::string out;
    for (char c : name) {
        const unsigned char u = static_cast<unsigned char>(c);
        out += (std::isalnum(u) || c == '-' || c == '_' || c == ' ') ? c : '_';
    }
    size_t b = out.find_first_not_of(' ');
    size_t e = out.find_last_not_of(' ');
    out = (b == std::string::npos) ? "" : out.substr(b, e - b + 1);
    if (out.empty()) out = "track" + std::to_string(index + 1);
    return out;
}

std::string jsonString(const std::string& s) {
    return juce::JSON::toString(juce::var(juce::String(s))).toStdString();
}

// The `tracksToDo` bitset for a set of tracks — NOT te::toBitSet(), which is broken upstream and sets every bit whatever you pass it (docs/cliModes.md).
juce::BigInteger trackBits(te::Edit& edit, const juce::Array<te::Track*>& wanted) {
    juce::BigInteger bits;
    auto all = te::getAllTracks(edit);
    for (auto* t : wanted)
        if (const int i = all.indexOf(t); i >= 0)
            bits.setBit(i);
    return bits;
}

// Every track a MIXDOWN is made of: `tracksToDo` never consults output routing — docs/cueOutput.md.
juce::Array<te::Track*> tracksOnTheMix(te::Edit& edit, const ProjectDoc& doc) {
    // By INDEX, the correspondence populateEdit uses: only a bus happens to be named after its id.
    auto audio = te::getAudioTracks(edit);
    std::set<te::Track*> offTheMix;
    for (size_t i = 0; i < doc.tracks.size() && i < static_cast<size_t>(audio.size()); ++i)
        if (!doc.tracks[i].outputChannels.empty())
            offTheMix.insert(audio[static_cast<int>(i)]);
    juce::Array<te::Track*> keep;
    for (auto* t : te::getAllTracks(edit))
        if (t != nullptr && offTheMix.count(t) == 0) keep.add(t);
    return keep;
}

// How long a render may make no progress before it is called stuck, rather than slow.
constexpr int kStallTimeoutSeconds = 90;

// Bounce a set of the edit's tracks to `out`: the task made and dropped on the message thread, its runJob() looped on THIS one, by hand — the blocking renderToFile overloads route through UIBehaviour, a no-op headless.
bool renderTracks(te::Edit& edit, const juce::File& out, const juce::BigInteger& bits, bool useMaster,
                  const tt::TimeRange& range, double sr, const RenderSteps& steps, std::string& why) {
    if (bits.countNumberOfSetBits() == 0) return false;
    std::atomic<float> progress{0.0f};
    std::unique_ptr<te::Renderer::RenderTask> task;
    trace("createRenderTask");
    const bool made = steps.onMessageThread([&] {
        te::Renderer::Parameters params(edit);
        params.destFile = out;
        params.audioFormat = edit.engine.getAudioFileFormatManager().getWavFormat();
        params.bitDepth = 24;
        params.blockSizeForAudio = 512;
        params.sampleRateForAudio = sr;
        params.time = range;
        params.tracksToDo = bits;
        params.usePlugins = true;
        params.useMasterPlugins = useMaster;
        task = te::render_utils::createRenderTask(params, "k7 export", &progress, nullptr);
    });
    if (!made) {
        why = "the render was stopped";
        return false;
    }
    if (!task) return false;
    trace("runJob loop");
    auto lastProgress = progress.load();
    auto lastMoved = std::chrono::steady_clock::now();
    bool finished = true;
    while (task->runJob() == juce::ThreadPoolJob::jobNeedsRunningAgain) {
        // A non-wav source plays off a PROXY whose render is kicked off by a juce Timer, so a loop the message loop never runs beside waits forever (docs/cliModes.md).
        steps.betweenBlocks();
        const auto now = std::chrono::steady_clock::now();
        if (steps.cancelled()) {
            why = "the render was stopped";
            finished = false;
            break;
        }
        if (progress.load() > lastProgress) {
            lastProgress = progress.load();
            lastMoved = now;
        } else if (now - lastMoved > std::chrono::seconds(kStallTimeoutSeconds)) {
            // A source the engine cannot decode leaves its node forever un-ready; fail loudly rather than spin, since a render that never returns reads as a wedged machine.
            why = "stalled for " + std::to_string(kStallTimeoutSeconds) +
                  "s with no progress — a clip's source is probably undecodable";
            finished = false;
            break;
        }
    }
    trace("runJob done");
    // Dropped where it was made; a message thread that is gone leaves it leaked rather than freed off it.
    if (!steps.onMessageThread([&] { task.reset(); })) {
        (void)task.release();
        why = "the render was stopped";
        return false;
    }
    const bool wrote = finished && out.existsAsFile() && out.getSize() > 0;
    if (!wrote) out.deleteFile();
    return wrote;
}

// The edit an offline bounce runs on, device-free and populated from the doc on the message thread; null when that thread would not run it.
std::unique_ptr<te::Edit> buildRenderEdit(te::Engine& engine, const ProjectDoc& doc, const std::string& baseDir,
                                          const RenderSteps& steps) {
    std::unique_ptr<te::Edit> edit;
    const bool built = steps.onMessageThread([&] {
        trace("createSingleTrackEdit");
        edit = te::Edit::createSingleTrackEdit(engine);
        trace("populateEdit");
        populateEdit(engine, *edit, doc, baseDir);
        trace("populateEdit done");
    });
    if (!built) return {};
    // Let the message loop run, or SamplerPlugin never builds its sounds and a rack track renders silent.
    steps.settle(200);
    trace("pump done");
    return edit;
}

// Everything the edit plays, plus a 2s tail for synth release and reverb.
tt::TimeRange fullRange(te::Edit& edit) {
    return tt::TimeRange(tt::TimePosition(), edit.getLength() + tt::TimeDuration::fromSeconds(2.0));
}

double renderRate(const ProjectDoc& doc) {
    return doc.sampleRate > 0 ? static_cast<double>(doc.sampleRate) : 48000.0;
}

// Every way OUT of a render resets the Edit on the message thread and drains the queue — the AsyncPluginDeleter crash AGENTS.md describes, on ANY return.
struct EditTeardown {
    std::unique_ptr<te::Edit>& edit;
    const RenderSteps& steps;
    ~EditTeardown() {
        if (!edit) return;
        try {
            // A message thread that is gone cannot free it, and no other thread may: leaked.
            if (!steps.onMessageThread([this] { edit.reset(); })) {
                (void)edit.release();
                return;
            }
            steps.settle(200);
        } catch (...) {
            std::cerr << "[render] the edit's teardown threw — leaked\n";
            (void)edit.release();
        }
    }
};

// One stem to bounce: the track's name and its bit, read on the message thread before any render starts.
struct StemPlan {
    std::string name;
    juce::BigInteger bits;
};

}  // namespace

// A headless engine: everything Tracktion needs to BUILD and render an edit, and no audio device — "The audio device is opened ONCE, explicitly" in AGENTS.md.
struct HeadlessBehaviour : te::EngineBehaviour {
    explicit HeadlessBehaviour(int renderThreads) : threads(renderThreads) {}
    bool autoInitialiseDeviceManager() override { return false; }
    // A bounce renders on `threads`, the caller included; 0 keeps Tracktion's every core.
    int getNumberOfCPUsToUseForAudio() override {
        return threads > 0 ? threads : te::EngineBehaviour::getNumberOfCPUsToUseForAudio();
    }
    int threads;
};

// The engine every offline path uses. Constructed the same way in all of them, because they fail the same way.
std::unique_ptr<te::Engine> makeOfflineEngine(int renderThreads) {
    auto engine = std::make_unique<te::Engine>(makePropertyStorage(), nullptr,
                                               std::make_unique<HeadlessBehaviour>(renderThreads));

    // The hosted-device route for ONE side effect — no MIDI rescan, whose RPC into midisrv.exe never returns when that service is wedged — and BEFORE anything pumps (AGENTS.md).
    te::HostedAudioDeviceInterface::Parameters p;
    p.useMidiDevices = false;
    engine->getDeviceManager().getHostedAudioDeviceInterface().initialise(p);
    return engine;
}

RenderOutcome renderWith(te::Engine& engine, const ProjectDoc& doc, const std::string& baseDir,
                         const std::string& outDir, bool stems, const RenderSteps& steps) {
    auto edit = buildRenderEdit(engine, doc, baseDir, steps);
    const EditTeardown teardown{edit, steps};
    if (!edit) return {2, {}, "the render was stopped before it began"};
    trace("edit ready");

    std::error_code ec;
    fs::create_directories(outDir, ec);

    const double sr = renderRate(doc);
    tt::TimeRange range;
    juce::BigInteger mixBits;
    std::vector<StemPlan> plan;
    const bool planned = steps.onMessageThread([&] {
        range = fullRange(*edit);
        mixBits = trackBits(*edit, tracksOnTheMix(*edit, doc));
        if (!stems) return;
        auto tracks = te::getAudioTracks(*edit);
        for (int i = 0; i < tracks.size(); ++i) {
            juce::Array<te::Track*> one;
            one.add(tracks[i]);
            plan.push_back({tracks[i]->getName().toStdString(), trackBits(*edit, one)});
        }
    });
    if (!planned) return {2, {}, "the render was stopped"};
    const juce::File dir{juce::String(outDir)};

    std::string why;
    if (!renderTracks(*edit, dir.getChildFile("mix.wav"), mixBits, true, range, sr, steps, why))
        return {1, {}, why.empty() ? "mixdown produced no audio (song has nothing audible)" : why};

    std::ostringstream manifest;
    manifest << "{\"mix\":\"mix.wav\",\"stems\":[";
    bool first = true;
    for (size_t i = 0; i < plan.size(); ++i) {
        const std::string fname = sanitizeStem(plan[i].name, static_cast<int>(i)) + ".wav";
        if (!renderTracks(*edit, dir.getChildFile(juce::String(fname)), plan[i].bits, false, range, sr, steps, why)) {
            if (!why.empty()) return {1, {}, why};
            std::cerr << "[render] stem '" << plan[i].name << "' had no audio — skipped\n";
            continue;
        }
        manifest << (first ? "" : ",") << "{\"name\":" << jsonString(plan[i].name)
                 << ",\"file\":" << jsonString(fname) << "}";
        first = false;
    }
    manifest << "]}";
    return {0, manifest.str(), {}};
}

// Render the doc to <outDir>/mix.wav and, with `stems`, one wav per track, printing a JSON manifest; non-zero when nothing was audible.
int renderOffline(const ProjectDoc& doc, const std::string& baseDir,
                  const std::string& outDir, bool stems) {
    trace("juce init");
    juce::ScopedJuceInitialiser_GUI juceInit;
    trace("engine ctor");
    auto engine = makeOfflineEngine();
    trace("engine ready");
    trace("registerBuiltInPlugins");
    registerBuiltInPlugins(*engine);

    // This thread IS the message thread, and nobody else pumps it: every step runs inline and every wait is a pump.
    RenderSteps steps;
    steps.onMessageThread = [](const std::function<void()>& fn) {
        fn();
        return true;
    };
    // The watchdog guards the FIRST pump, since pumping runs the MIDI timer too (AGENTS.md).
    steps.settle = [first = true](int ms) mutable {
        if (first) watchdogStack(20);
        first = false;
        pumpMessages(ms);
    };
    // 1 ms, not 0: JUCE's macOS loop returns before dispatching anything when no time is left (docs/cliModes.md).
    steps.betweenBlocks = [] { pumpMessages(1); };
    steps.cancelled = [] { return false; };

    const auto outcome = renderWith(*engine, doc, baseDir, outDir, stems, steps);
    if (outcome.code != 0) {
        std::cerr << "[render] " << outcome.error << "\n";
        return outcome.code;
    }
    std::cout << outcome.manifest << std::endl;
    return 0;
}

}  // namespace studio
