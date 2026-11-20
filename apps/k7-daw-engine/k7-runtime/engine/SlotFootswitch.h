// SlotFootswitch.h — the slot's footswitch as data: which CC it is, which plugins a press bypasses, and when a
// controller message is a PRESS. JUCE-free and header-only, so tests/ has it in a fast binary (docs/sessions.md, "The slot").
// What reads wrong: only the chain's own plugins are bypassed, never the rig's fader or recorder, so a bypassed slot plays dry.

#pragma once

#include "ProjectDoc.h"

#include <algorithm>
#include <string>
#include <vector>

namespace studio {

/** A slot's footswitch: CC `cc` on any channel, -1 for none; `plugins` the ids a press bypasses. */
struct SlotFootswitch {
    int cc = -1;
    std::vector<std::string> plugins;
    bool bound() const { return cc >= 0 && cc <= 127 && !plugins.empty(); }
};

/** `doc` with every plugin `ids` names turned off; the others as they were. */
inline ProjectDoc withBypassed(ProjectDoc doc, const std::vector<std::string>& ids) {
    const auto off = [&](std::vector<PluginDoc>& list) {
        for (auto& p : list)
            if (std::find(ids.begin(), ids.end(), p.id) != ids.end()) p.enabled = false;
    };
    for (auto& t : doc.tracks) off(t.plugins);
    off(doc.masterPlugins);
    return doc;
}

/** A momentary pedal's press: the value rising to 64 or past it; its release and repeats are not presses. */
class PressEdge {
public:
    bool press(int value) {
        const bool now = value >= 64;
        const bool pressed = now && !down_;
        down_ = now;
        return pressed;
    }

private:
    bool down_ = false;
};

}  // namespace studio
