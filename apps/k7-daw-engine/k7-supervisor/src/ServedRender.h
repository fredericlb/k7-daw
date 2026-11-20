// ServedRender.h — a render asked of the RUNNING engine over its socket, for a phone, where no program may start.
// The one thing a reader would get wrong: it blocks for the whole render, so it belongs on a job's thread, never the loop.

#pragma once

#include <nlohmann/json.hpp>

#include <atomic>
#include <chrono>

namespace k7::served_render {

/** Send `cmd` (a `render` command) to the engine on `port` and wait for its `rendered` reply; an `ok:false` reply with `error` when none comes. */
nlohmann::json ask(int port, const nlohmann::json& cmd, std::chrono::milliseconds limit = std::chrono::hours(2));

}  // namespace k7::served_render
