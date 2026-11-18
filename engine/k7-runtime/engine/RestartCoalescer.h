#pragma once

// RestartCoalescer.h — when a route change may restart JUCE's iOS audio device on a Mac running the iPad app.
// JUCE-free and header-only (tests/ has it), included into juce_Audio_ios.cpp by engine/patches/tracktion.cmake.
// What reads wrong: an 'unknown' change just after OUR restart is dropped, not delayed, since our own restart causes it.

#include <algorithm>
#include <cstdint>

namespace studio {

class RestartCoalescer {
public:
    enum class Verdict { restartNow, later, drop };

    // At most one restart per window; also how long after a restart an 'unknown' route change counts as its echo.
    static constexpr std::int64_t kWindowMs = 1000;

    // A route change wants a restart at `nowMs` (any monotonic clock); `unknown` is the reason 'unknown'.
    Verdict request(std::int64_t nowMs, bool unknown) {
        const bool recent = restarted_ && nowMs - lastMs_ < kWindowMs;
        if (!recent) return Verdict::restartNow;
        if (unknown) return Verdict::drop;
        pending_ = true;
        return Verdict::later;
    }

    // Milliseconds until a deferred request may restart; 0 when it may now.
    std::int64_t waitMs(std::int64_t nowMs) const {
        return restarted_ ? std::max<std::int64_t>(0, kWindowMs - (nowMs - lastMs_)) : 0;
    }

    // A restart finished at `nowMs`; whatever was waiting is served by it.
    void restarted(std::int64_t nowMs) {
        restarted_ = true;
        lastMs_ = nowMs;
        pending_ = false;
    }

    // A deferred request is still waiting for its slot.
    bool pending() const { return pending_; }

    // True while `nowMs` is within `windowMs` after a restart finished at `lastMs`; 0 means none yet.
    static bool recentlyRestarted(std::int64_t nowMs, std::int64_t lastMs, std::int64_t windowMs) {
        return lastMs != 0 && nowMs - lastMs < windowMs;
    }

private:
    bool restarted_ = false;
    bool pending_ = false;
    std::int64_t lastMs_ = 0;
};

}  // namespace studio
