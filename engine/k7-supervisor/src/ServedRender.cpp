// ServedRender.cpp — see ServedRender.h. Its own socket: `rendered` goes to the asking socket alone, and a render outlasts any query's deadline.

#include "ServedRender.h"

#include <ixwebsocket/IXWebSocket.h>

#include <condition_variable>
#include <mutex>

namespace k7::served_render {

using nlohmann::json;

namespace {

json failure(const json& id, const std::string& why)
{
    return {{"type", "rendered"}, {"id", id}, {"ok", false}, {"error", why}};
}

}  // namespace

json ask(int port, const json& cmd, std::chrono::milliseconds limit)
{
    const json id = cmd.value("id", json());
    std::mutex mutex;
    std::condition_variable cv;
    bool open = false;
    bool closed = false;
    std::optional<json> reply;

    ix::WebSocket ws;
    ws.setUrl("ws://127.0.0.1:" + std::to_string(port));
    ws.disableAutomaticReconnection();
    ws.setOnMessageCallback([&](const ix::WebSocketMessagePtr& m) {
        std::lock_guard lock(mutex);
        if (m->type == ix::WebSocketMessageType::Open) {
            open = true;
        } else if (m->type == ix::WebSocketMessageType::Close || m->type == ix::WebSocketMessageType::Error) {
            closed = true;
        } else if (m->type == ix::WebSocketMessageType::Message) {
            auto msg = json::parse(m->str, nullptr, false);
            if (msg.is_object() && msg.value("type", "") == "rendered" && msg.value("id", json()) == id) reply = std::move(msg);
        }
        cv.notify_all();
    });
    ws.start();

    json out;
    {
        std::unique_lock lock(mutex);
        cv.wait_for(lock, std::chrono::seconds(4), [&] { return open || closed; });
        if (!open) {
            lock.unlock();
            ws.stop();
            return failure(id, "the engine did not answer");
        }
        lock.unlock();
        ws.send(cmd.dump(-1, ' ', false, json::error_handler_t::replace));
        lock.lock();
        const bool got = cv.wait_for(lock, limit, [&] { return reply.has_value() || closed; });
        out = reply ? *reply : failure(id, got ? "the engine closed the connection during the render" : "the render took too long");
    }
    ws.stop();
    return out;
}

}  // namespace k7::served_render
