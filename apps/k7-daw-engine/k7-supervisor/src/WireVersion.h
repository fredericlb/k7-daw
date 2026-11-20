// WireVersion.h — what this supervisor says it speaks on the engine wire: kEngineProtocol, mirrored in
// packages/engine-protocol/version.ts and kept equal by that package's test, and the version it
// was built from, the root package.json's.

#pragma once

#include <string>

namespace k7::wire {

/** The engine wire's version; what bumps it is packages/engine-protocol/docs/version.md. */
constexpr int kEngineProtocol = 1;

/** The root package.json's version. */
std::string build();

}  // namespace k7::wire
