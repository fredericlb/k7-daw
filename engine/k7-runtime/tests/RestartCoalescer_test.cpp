// The restart rule for JUCE's iOS device on a Mac. JUCE-free, so it runs in a fast binary; the patched
// handler needs a live AVAudioSession, which only the Mac running the app has (docs/ios.md).
//
// What would be wrong SILENTLY: an 'unknown' route change that restarts the device again (the loop), and a
// real change that is lost instead of served once the window is over.

#include "RestartCoalescer.h"

#include "test_harness.h"

using studio::RestartCoalescer;
using Verdict = RestartCoalescer::Verdict;

namespace {

void the_first_request_restarts()
{
    RestartCoalescer c;
    CHECK(c.request(5000, true) == Verdict::restartNow);
    CHECK(c.waitMs(5000) == 0);
}

void an_unknown_change_right_after_a_restart_is_dropped()
{
    RestartCoalescer c;
    c.restarted(10000);
    CHECK(c.request(10100, true) == Verdict::drop);
    CHECK(!c.pending());
    CHECK(c.request(10000 + RestartCoalescer::kWindowMs, true) == Verdict::restartNow);
}

void another_reason_inside_the_window_waits_for_its_end()
{
    RestartCoalescer c;
    c.restarted(10000);
    CHECK(c.request(10300, false) == Verdict::later);
    CHECK(c.pending());
    CHECK(c.waitMs(10300) == RestartCoalescer::kWindowMs - 300);
    CHECK(c.waitMs(12000) == 0);
    c.restarted(11000);
    CHECK(!c.pending());
}

void a_storm_costs_one_restart_per_window()
{
    RestartCoalescer c;
    int restarts = 0;
    for (long long t = 0; t < 10000; t += 3) {
        const auto v = c.request(t, (t / 3) % 2 == 0);
        if (v == Verdict::restartNow) {
            ++restarts;
            c.restarted(t + 900);
        }
    }
    CHECK(restarts <= 10);
}

void recently_restarted_reads_zero_as_never()
{
    CHECK(!RestartCoalescer::recentlyRestarted(500, 0, 3000));
    CHECK(RestartCoalescer::recentlyRestarted(2000, 1000, 3000));
    CHECK(!RestartCoalescer::recentlyRestarted(4000, 1000, 3000));
}

}  // namespace

int main()
{
    RUN(the_first_request_restarts);
    RUN(an_unknown_change_right_after_a_restart_is_dropped);
    RUN(another_reason_inside_the_window_waits_for_its_end);
    RUN(a_storm_costs_one_restart_per_window);
    RUN(recently_restarted_reads_zero_as_never);
    TEST_MAIN_END();
}
