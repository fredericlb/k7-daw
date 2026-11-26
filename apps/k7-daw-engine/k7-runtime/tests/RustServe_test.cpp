// RustServe_test.cpp — the Rust server's `serve` on the REAL Tracktion host, probed over its socket with `whoami`.
// The one thing a reader would get wrong: serve blocks the main thread (the message thread), so the client is the thread.

#include "capi/k7_server.h"
#include "test_harness.h"

#include <ixwebsocket/IXNetSystem.h>
#include <ixwebsocket/IXWebSocket.h>
#include <nlohmann/json.hpp>

#include <atomic>
#include <chrono>
#include <mutex>
#include <random>
#include <string>
#include <thread>

namespace {

struct Probe {
    std::mutex mx;
    std::string reply;
    std::atomic<bool> done{false};
};

// Connects, asks `whoami`, keeps the first text answer, closes, and ends the server whatever happened.
void ask(int port, Probe& probe) {
    ix::WebSocket ws;
    ws.setUrl("ws://127.0.0.1:" + std::to_string(port));
    ws.setOnMessageCallback([&](const ix::WebSocketMessagePtr& msg) {
        if (msg->type == ix::WebSocketMessageType::Open) {
            ws.send(R"({"type":"whoami"})");
        } else if (msg->type == ix::WebSocketMessageType::Message && !probe.done) {
            std::lock_guard lock(probe.mx);
            probe.reply = msg->str;
            probe.done = true;
        }
    });
    ws.start();
    const auto until = std::chrono::steady_clock::now() + std::chrono::seconds(20);
    while (!probe.done && std::chrono::steady_clock::now() < until) std::this_thread::sleep_for(std::chrono::milliseconds(20));
    ws.stop();
    k7_server_request_quit();
}

void whoamiOverTheSocket() {
    ix::initNetSystem();
    std::random_device rd;
    const int port = 30000 + static_cast<int>(rd() % 20000);
    Probe probe;
    std::thread client([&] {
        // The listener opens from inside the message loop, so a refused connection means "not yet": the client reconnects until it answers.
        ask(port, probe);
    });
    const int code = k7_server_serve(port);
    client.join();
    CHECK(code == 0);
    CHECK(probe.done);
    const auto reply = nlohmann::json::parse(probe.reply, nullptr, false);
    CHECK(!reply.is_discarded());
    CHECK(reply.value("type", "") == "whoami");
    CHECK(reply.value("protocol", 0) == 1);
    CHECK(!reply.value("build", std::string{}).empty());
}

}  // namespace

int main() {
    RUN(whoamiOverTheSocket);
    TEST_MAIN_END();
}
