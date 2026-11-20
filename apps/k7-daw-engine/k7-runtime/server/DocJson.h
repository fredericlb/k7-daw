// DocJson.h — a ProjectDoc off the wire: the JSON the song layer sends, read into the engine's model.
// Shared by the server and the CLI's offline modes, so a document reads the same whichever way it came.

#pragma once

#include <ProjectDoc.h>

#include <nlohmann/json.hpp>

#include <string>
#include <vector>

namespace studio {

/** The deepest nesting a message off the wire may have; a project document needs about ten. */
constexpr int kMaxMessageDepth = 64;

/** One message off the socket, parsed — throws like json::parse, and on nesting past kMaxMessageDepth (docs/protocol.md). */
nlohmann::json parseMessage(const std::string& text);

/** Every path docFromJson drops on THIS thread while one is alive — what a `render` refuses whole (docs/pathRoots.md). */
class PathRefusals {
public:
    PathRefusals();
    ~PathRefusals();
    PathRefusals(const PathRefusals&) = delete;
    PathRefusals& operator=(const PathRefusals&) = delete;

    std::vector<std::string> paths;

private:
    std::vector<std::string>* previous_;
};

/** A whole document — throws on a shape it cannot read, which callers report as a bad project. */
ProjectDoc docFromJson(const nlohmann::json& j);

/** ONE track, split out so the `tracks` delta reads a track exactly as a whole document does. */
TrackDoc trackFromJson(const nlohmann::json& t);

/** One plugin, shared by a TRACK's list and the MASTER chain's so the two cannot drift. */
PluginDoc pluginFromJson(const nlohmann::json& p);

}  // namespace studio
