// DocReader.h — a VALIDATED ProjectDoc read from JSON: the shape the Rust server hands the C ABI's `apply`.
// No check lives here (bounds, ids, paths are the Rust side's, docs/project.md beside k7-runtime-server): a
// field missing or mistyped is a bug of the writer and throws, rather than a default that plays silently.
// JUCE-free, so tests/ has it in a fast binary.

#pragma once

#include "ProjectDoc.h"
#include "SlotFootswitch.h"

#include <nlohmann/json.hpp>

namespace studio {

ProjectDoc readDoc(const nlohmann::json& j);

TrackDoc readTrack(const nlohmann::json& j);

PluginDoc readPlugin(const nlohmann::json& j);

/** The slot's footswitch carried by the document, unbound when it carries none. */
SlotFootswitch readFootswitch(const nlohmann::json& doc);

}  // namespace studio
