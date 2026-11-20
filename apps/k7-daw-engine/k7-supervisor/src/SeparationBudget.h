// SeparationBudget.h — what a cut asks of the machine's cores and memory, with no model and no device in it:
// a phone's thread count, the memory a cut needs, and the sentence that refuses one. docs/supervisor.md, "A cut on a phone".

#pragma once

#include <algorithm>
#include <atomic>
#include <cstddef>
#include <string>

namespace k7::separation {

/** A cut's CPU threads: half the cores on a phone, which has no render pool to spare; a computer keeps five cores back. */
inline int threadCount(int cores, bool phone)
{
    return phone ? std::max(1, cores / 2) : std::max(1, std::min(cores / 2, cores - 5));
}

/** The weights held twice (the file and ORT's copy), the model's activations, and the song per channel: mix plus four stems and the weights. */
inline std::size_t bytesNeeded(std::size_t modelBytes, std::size_t frames)
{
    constexpr std::size_t kActivations = std::size_t(400) << 20;
    constexpr std::size_t kChannels = 2, kBuffers = 6, kFloat = 4;
    return modelBytes * 2 + kActivations + frames * kChannels * kBuffers * kFloat;
}

/** Empty when `available` covers `needed` (and `floor`, a raised minimum for a test), else why the cut is refused. */
inline std::string memoryRefusal(std::size_t available, std::size_t needed, std::size_t floor = 0)
{
    const std::size_t want = std::max(needed, floor);
    if (available >= want) return {};
    return "this device has " + std::to_string(available >> 20) + " MB of memory free for k7 and a cut needs " +
           std::to_string(want >> 20) + " MB: close other apps, or cut on a computer";
}

/** Cuts that are queued or running, which `Busy.swift` reads to keep the app alive behind another. */
inline std::atomic<int>& cutsInFlight()
{
    static std::atomic<int> n{0};
    return n;
}

}  // namespace k7::separation
