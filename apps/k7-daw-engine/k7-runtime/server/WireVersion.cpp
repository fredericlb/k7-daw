// WireVersion.cpp — the one source the version is compiled into, so a new version recompiles one file.

#include "WireVersion.h"

#ifndef K7_BUILD
#define K7_BUILD "dev"
#endif

namespace studio {

std::string buildStamp() { return K7_BUILD; }

}  // namespace studio
