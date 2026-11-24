// EnginePorts_test.cpp — the ports a start tries: the settings' port first, then the next ones, and
// only that one when `--port` fixes it; a port another program listens on at 127.0.0.1 alone reads as held.

#include "EngineProcess.h"

#include "test_harness.h"

#ifdef _WIN32
#include <winsock2.h>
#else
#include <arpa/inet.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <unistd.h>
#endif

using k7::engine::bindable;
using k7::engine::candidatePorts;
using k7::engine::kPortSpan;

namespace {

void theFirstPortLeadsAndTheNextOnesFollow()
{
    const auto ports = candidatePorts(8766, false);
    CHECK(static_cast<int>(ports.size()) == kPortSpan);
    CHECK(ports.front() == 8766);
    CHECK(ports.back() == 8766 + kPortSpan - 1);
}

void aFixedPortIsTheOnlyOne()
{
    CHECK((candidatePorts(8766, true) == std::vector<int>{8766}));
}

void theRangeStopsAtTheLastPort()
{
    CHECK((candidatePorts(65534, false) == std::vector<int>{65534, 65535}));
    CHECK(candidatePorts(0, false).empty());
}

#ifndef _WIN32
// A listener on 127.0.0.1 only, as adb's port forward holds 8765; its port, 0 when none could be made.
int loopbackListener(int& fd)
{
    fd = socket(AF_INET, SOCK_STREAM, 0);
    sockaddr_in addr{};
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    socklen_t len = sizeof addr;
    if (fd < 0 || bind(fd, reinterpret_cast<sockaddr*>(&addr), len) != 0 || listen(fd, 1) != 0) return 0;
    getsockname(fd, reinterpret_cast<sockaddr*>(&addr), &len);
    return ntohs(addr.sin_port);
}

void aLoopbackOnlyListenerHoldsItsPort()
{
    int fd = -1;
    const int port = loopbackListener(fd);
    CHECK(port > 0);
    CHECK(!bindable(port));
    close(fd);
}
#endif

}  // namespace

void runEnginePortsTests()
{
    RUN(theFirstPortLeadsAndTheNextOnesFollow);
    RUN(aFixedPortIsTheOnlyOne);
    RUN(theRangeStopsAtTheLastPort);
#ifndef _WIN32
    RUN(aLoopbackOnlyListenerHoldsItsPort);
#endif
}
