// ServedRender.cpp — the serving engine's own bounce, renderInProcess(): a headless engine of its own beside the live
// one, its Edit built and dropped on the message thread, which keeps running, and the blocks rendered on the caller's
// worker thread. A phone may start no program, so the render that `--render` does in a process is done here (docs/cliModes.md).
// The one thing a reader would get wrong: nothing waits on a stopped message loop — cancelInProcessRenders() abandons.

#include "EngineHost.h"

#include "EngineTypes.h"     // Tracktion/JUCE + the te/tt/fs aliases
#include "PluginScan.h"      // registerBuiltInPlugins
#include "RenderCore.h"      // renderWith, RenderSteps

#include <chrono>
#include <condition_variable>
#include <exception>
#include <functional>
#include <memory>
#include <mutex>
#include <set>
#include <thread>

namespace studio {

namespace {

// The threads a served bounce renders on: its worker alone, so the live Edit's pool and audio thread keep their cores.
constexpr int kServedRenderThreads = 1;

// The headless engine every served render uses, built on first use on the message thread.
struct RenderEngine {
    juce::ScopedJuceInitialiser_GUI juceInit;
    std::unique_ptr<te::Engine> engine;
};

// Kept for the run and leaked, like the parameter-query engine: destroying a te::Engine after JUCE at exit is a crash.
RenderEngine* g_engine = nullptr;

// The workers rendering now, each told to exit when the message loop stops.
std::mutex g_workersMx;
std::set<std::thread::id> g_workers;

// One step posted to the message thread: whether it started, finished or was given up on before it started.
struct PostedStep {
    std::mutex m;
    std::condition_variable cv;
    bool started = false;
    bool done = false;
    bool abandoned = false;
    std::exception_ptr error;
};

// Runs `fn` on the message thread and waits; false, `fn` never to run, when this worker is told to exit before it starts.
bool onMessageThread(const std::function<void()>& fn) {
    auto step = std::make_shared<PostedStep>();
    const bool posted = juce::MessageManager::callAsync([step, &fn] {
        {
            std::lock_guard<std::mutex> lk(step->m);
            if (step->abandoned) return;
            step->started = true;
        }
        try {
            fn();
        } catch (...) {
            step->error = std::current_exception();
        }
        {
            std::lock_guard<std::mutex> lk(step->m);
            step->done = true;
        }
        step->cv.notify_all();
    });
    if (!posted) return false;
    std::unique_lock<std::mutex> lk(step->m);
    while (!step->done) {
        step->cv.wait_for(lk, std::chrono::milliseconds(50));
        // Once started it is waited for to the end: `fn` holds this thread's frame by reference.
        if (!step->started && !step->done && te::shouldCurrentThreadExit()) {
            step->abandoned = true;
            return false;
        }
    }
    if (step->error) std::rethrow_exception(step->error);
    return true;
}

// This worker, listed while it renders so a stop can reach it.
struct WorkerListing {
    WorkerListing() {
        std::lock_guard<std::mutex> lk(g_workersMx);
        g_workers.insert(std::this_thread::get_id());
    }
    ~WorkerListing() {
        std::lock_guard<std::mutex> lk(g_workersMx);
        g_workers.erase(std::this_thread::get_id());
    }
    WorkerListing(const WorkerListing&) = delete;
    WorkerListing& operator=(const WorkerListing&) = delete;
};

}  // namespace

RenderOutcome renderInProcess(const ProjectDoc& doc, const std::string& outDir, bool stems) {
    // Tracktion's own waits on the message thread (callBlocking) give up when this thread is told to exit.
    const te::ScopedThreadExitStatusEnabler exitStatus;
    const WorkerListing listed;

    RenderEngine* held = nullptr;
    const bool ready = onMessageThread([&held] {
        if (g_engine == nullptr) {
            auto* made = new RenderEngine();
            made->engine = makeOfflineEngine(kServedRenderThreads);
            registerBuiltInPlugins(*made->engine);
            g_engine = made;
        }
        held = g_engine;
    });
    if (!ready) return {2, {}, "the render was stopped before it began"};

    // The message loop runs beside this thread: a wait is a sleep, and nothing here may pump it.
    RenderSteps steps;
    steps.onMessageThread = onMessageThread;
    steps.settle = [](int ms) { std::this_thread::sleep_for(std::chrono::milliseconds(ms)); };
    steps.betweenBlocks = [] {};
    steps.cancelled = [] { return te::shouldCurrentThreadExit(); };
    return renderWith(*held->engine, doc, {}, outDir, stems, steps);
}

void cancelInProcessRenders() {
    std::lock_guard<std::mutex> lk(g_workersMx);
    for (const auto& id : g_workers) te::signalThreadShouldExit(id);
}

}  // namespace studio
