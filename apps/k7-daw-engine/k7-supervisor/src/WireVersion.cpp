// WireVersion.cpp — the one source the version is compiled into, so a new version recompiles one file.

#include "WireVersion.h"

#ifndef K7_BUILD
#define K7_BUILD "dev"
#endif

namespace k7::wire {

std::string build() { return K7_BUILD; }

}  // namespace k7::wire
