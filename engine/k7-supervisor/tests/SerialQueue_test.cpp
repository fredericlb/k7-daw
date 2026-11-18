// SerialQueue_test.cpp — a queue's drain runs what was queued, in order, then its last task once;
// it refuses what comes after; and a task that never ends cannot hold it past its bound.

#include "SerialQueue.h"

#include "test_harness.h"

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <mutex>
#include <thread>
#include <vector>

using k7::SerialQueue;
using namespace std::chrono_literals;

namespace {

/** Waits until `ok` holds, or `ms` passed; answers whether it held. */
template <typename F>
bool within(std::chrono::milliseconds ms, F ok)
{
    const auto end = std::chrono::steady_clock::now() + ms;
    while (!ok()) {
        if (std::chrono::steady_clock::now() > end) return false;
        std::this_thread::sleep_for(2ms);
    }
    return true;
}

void drainRunsWhatIsQueuedThenLast()
{
    auto q = SerialQueue::create();
    std::mutex m;
    std::vector<int> order;
    std::atomic<int> lasts{0};
    q->push([&] {
        std::this_thread::sleep_for(30ms);
        std::lock_guard lock(m);
        order.push_back(1);
    });
    q->push([&] {
        std::lock_guard lock(m);
        order.push_back(2);
    });
    q->drain(2000ms, [&] {
        std::lock_guard lock(m);
        order.push_back(9);
        ++lasts;
    });
    q->push([&] {
        std::lock_guard lock(m);
        order.push_back(3);
    });
    CHECK(within(1000ms, [&] { return lasts.load() == 1; }));
    std::this_thread::sleep_for(50ms);
    std::lock_guard lock(m);
    CHECK((order == std::vector<int>{1, 2, 9}));
    CHECK(lasts.load() == 1);
}

void aStuckTaskCannotHoldTheDrainPastItsBound()
{
    auto q = SerialQueue::create();
    std::mutex m;
    std::condition_variable cv;
    bool release = false;
    std::atomic<bool> afterRan{false};
    std::atomic<int> lasts{0};
    q->push([&] {
        std::unique_lock lock(m);
        cv.wait(lock, [&] { return release; });
    });
    q->push([&] { afterRan = true; });
    const auto started = std::chrono::steady_clock::now();
    q->drain(100ms, [&] { ++lasts; });
    CHECK(within(1000ms, [&] { return lasts.load() == 1; }));
    CHECK(std::chrono::steady_clock::now() - started < 900ms);
    {
        std::lock_guard lock(m);
        release = true;
    }
    cv.notify_all();
    std::this_thread::sleep_for(50ms);
    CHECK(!afterRan.load());
    CHECK(lasts.load() == 1);
}

void aClosedQueueDrainsNothing()
{
    auto q = SerialQueue::create();
    std::atomic<int> lasts{0};
    q->close();
    q->drain(50ms, [&] { ++lasts; });
    std::this_thread::sleep_for(150ms);
    CHECK(lasts.load() == 0);
}

}  // namespace

void runSerialQueueTests()
{
    RUN(drainRunsWhatIsQueuedThenLast);
    RUN(aStuckTaskCannotHoldTheDrainPastItsBound);
    RUN(aClosedQueueDrainsNothing);
}
