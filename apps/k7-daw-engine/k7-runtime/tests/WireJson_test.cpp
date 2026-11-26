// WireJson_test.cpp — nlohmann's dump() held to the fixture the Rust server's printer is held to too
// (apps/k7-daw-backend/packages/k7-runtime-server/fixtures/nlohmann_dump.txt): a frame is the same bytes from both.
// The one thing a reader would get wrong: the fixture is nlohmann's output, so this test proves the fixture, not nlohmann.

#include "test_harness.h"

#include <nlohmann/json.hpp>

#include <cstdint>
#include <cstring>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

namespace {

using json = nlohmann::json;

std::string dump(const json& j) { return j.dump(-1, ' ', false, json::error_handler_t::replace); }

struct Line {
    std::string kind, input, expected;
};

std::vector<Line> fixture() {
    std::vector<Line> out;
    std::ifstream f(K7_WIRE_FIXTURE);
    for (std::string line; std::getline(f, line);) {
        const auto a = line.find('\t');
        const auto b = line.find('\t', a + 1);
        if (a == std::string::npos || b == std::string::npos) continue;
        out.push_back({line.substr(0, a), line.substr(a + 1, b - a - 1), line.substr(b + 1)});
    }
    return out;
}

void everyLine() {
    const auto lines = fixture();
    CHECK(lines.size() > 1000);
    for (const auto& l : lines) {
        std::string got;
        if (l.kind == "f64") {
            const uint64_t bits = std::stoull(l.input, nullptr, 16);
            double x;
            std::memcpy(&x, &bits, sizeof x);
            got = dump(json(x));
        } else {
            got = dump(json::parse(l.input));
        }
        if (got != l.expected) std::printf("  %s %s: got %s\n", l.kind.c_str(), l.input.c_str(), got.c_str());
        CHECK(got == l.expected);
    }
}

}  // namespace

int main() {
    RUN(everyLine);
    TEST_MAIN_END();
}
