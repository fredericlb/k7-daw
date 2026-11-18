// SlotSession.cpp — the slot's footswitch on EngineHost: a press re-applies the slot's chain with its own plugins
// bypassed, the next brings them back, through the same diff any apply runs. See docs/sessions.md, "The slot".
// What reads wrong: a new chain sent to the slot starts engaged, whatever the pedal last did.

#include "EngineHost.h"

#include "EngineTypes.h"
#include "EngineHostImpl.h"

#include <iostream>

namespace studio {

void EngineHost::setSlotFootswitch(SlotFootswitch fs) {
    auto& i = *impl_;
    i.slotFootswitch = std::move(fs);
    i.slotEngaged = current(kSlotSession);
    i.slotBypassed = false;
    if (!i.slotFootswitch.bound()) {
        i.slotMidi.bind(-1, {});
        return;
    }
    i.slotMidi.bind(i.slotFootswitch.cc, [this] {
        post([this] {
            auto& i = *impl_;
            const auto* s = i.find(kSlotSession);
            // A press after a project took the engine over: nothing of the slot is left to bypass.
            if (s == nullptr || s->edit == nullptr) return;
            i.slotBypassed = !i.slotBypassed;
            apply(kSlotSession, i.slotBypassed ? withBypassed(i.slotEngaged, i.slotFootswitch.plugins) : i.slotEngaged, "");
            std::cerr << "[slot] footswitch: chain " << (i.slotBypassed ? "bypassed" : "engaged") << "\n";
        });
    });
}

}  // namespace studio
