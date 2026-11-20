// RenderCore.h — one bounce of a ProjectDoc to wav, shared by `--render` and the serving engine's `render`
// command, so the two write the same samples. What differs between them is WHERE each step runs, given as
// RenderSteps: the CLI owns its message thread and pumps it; the server's runs beside the render (docs/cliModes.md).
// The one thing a reader would get wrong: only the runJob loop runs on the caller's thread; the Edit is touched on the message thread.

#pragma once

#include "EngineHost.h"
#include "EngineTypes.h"

#include <functional>
#include <string>

namespace studio {

struct RenderSteps {
    // Runs a step on the message thread and waits for it; false when it could not, and the render must stop and leak.
    std::function<bool(const std::function<void()>&)> onMessageThread;
    // Lets the message loop run for `ms`: SamplerPlugin's sounds before the first block, the deleter after the last.
    std::function<void(int ms)> settle;
    // After each runJob: a proxy's render is started by a juce::Timer, which only a running message loop fires.
    std::function<void()> betweenBlocks;
    // Whether the render was asked to stop, checked between blocks.
    std::function<bool()> cancelled;
};

/** Bounce `doc` into `outDir` on `engine` (built-ins registered): mix.wav, a wav per track with `stems`, and the manifest. */
RenderOutcome renderWith(te::Engine& engine, const ProjectDoc& doc, const std::string& baseDir,
                         const std::string& outDir, bool stems, const RenderSteps& steps);

}  // namespace studio
