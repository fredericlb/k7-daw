// SerialQueue.h — tasks run one at a time, in order, on a thread of their own: the C++ form of a
// promise chain. Closing it drops what is still queued and lets the thread end by itself, since the
// closer may be the very thread a queued task is waiting on — a join there would deadlock.
// Draining it runs what is queued first, within a bound (packages/engine-tunnel/docs/tunnel.md).

#pragma once

#include <chrono>
#include <condition_variable>
#include <deque>
#include <functional>
#include <memory>
#include <mutex>
#include <thread>

#include "Loop.h"

namespace k7 {

class SerialQueue : public std::enable_shared_from_this<SerialQueue> {
public:
    static std::shared_ptr<SerialQueue> create()
    {
        auto q = std::shared_ptr<SerialQueue>(new SerialQueue());
        std::thread([q] { q->run(); }).detach();
        return q;
    }

    void push(std::function<void()> task)
    {
        {
            std::lock_guard lock(mutex_);
            if (closed_ || draining_) return;
            tasks_.push_back(std::move(task));
        }
        cv_.notify_one();
    }

    void close()
    {
        {
            std::lock_guard lock(mutex_);
            closed_ = true;
            done_ = true;
            tasks_.clear();
        }
        cv_.notify_all();
    }

    /** Refuse new tasks, run the queued ones, then `last`; past `bound`, drop the rest and run `last` from a watchdog. */
    void drain(std::chrono::milliseconds bound, std::function<void()> last)
    {
        auto once = std::make_shared<std::once_flag>();
        auto finish = [once, last = std::move(last)] { std::call_once(*once, last); };
        {
            std::lock_guard lock(mutex_);
            if (closed_ || draining_) return;
            draining_ = true;
            tasks_.push_back(finish);
        }
        cv_.notify_all();
        // Its own thread, never the caller's: the caller may be a socket's thread, which must not block.
        std::thread([self = shared_from_this(), bound, finish] {
            std::unique_lock lock(self->mutex_);
            if (self->cv_.wait_for(lock, bound, [&] { return self->done_; })) return;
            lock.unlock();
            self->close();
            finish();
        }).detach();
    }

private:
    SerialQueue() = default;

    void run()
    {
        for (;;) {
            std::function<void()> task;
            {
                std::unique_lock lock(mutex_);
                cv_.wait(lock, [&] { return closed_ || !tasks_.empty(); });
                if (closed_) return;
                task = std::move(tasks_.front());
                tasks_.pop_front();
            }
            // A task that throws costs itself, never the thread the rest of the queue runs on.
            Loop::guarded("a queued task", task);
            std::lock_guard lock(mutex_);
            if (draining_ && tasks_.empty() && !closed_) {
                closed_ = true;
                done_ = true;
                cv_.notify_all();
                return;
            }
        }
    }

    std::mutex mutex_;
    std::condition_variable cv_;
    std::deque<std::function<void()>> tasks_;
    bool closed_ = false;
    bool draining_ = false;
    /** Drained to the end, or closed: the watchdog of `drain` has nothing left to do. */
    bool done_ = false;
};

}  // namespace k7
