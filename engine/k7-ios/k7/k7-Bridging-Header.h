// k7-Bridging-Header.h — the C entries of what this app links without a module of its own: the two frameworks built
// from the engine's sources, k7runtime and k7supervisor, and the Rust front's static library (engine/k7-runtime/docs/ios.md).

#include "../../k7-runtime/server/ios/EngineEmbed.h"
#include "../../k7-supervisor/src/Embedded.h"
#include "K7Embed.h"
