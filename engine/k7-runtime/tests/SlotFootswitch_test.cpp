// Unit tests for SlotFootswitch.h — what a press of the slot's footswitch bypasses, and what counts as a press.
// Header-only and JUCE-free, so it builds in seconds (../docs/sessions.md, "The slot").

#include "SlotFootswitch.h"
#include "test_harness.h"

using studio::PressEdge;
using studio::ProjectDoc;
using studio::SlotFootswitch;
using studio::withBypassed;

namespace {

ProjectDoc rig() {
    ProjectDoc d;
    studio::TrackDoc t;
    t.id = "IN";
    for (const char* id : {"IN:amp", "IN:cab", "IN:fader"}) {
        studio::PluginDoc p;
        p.id = id;
        t.plugins.push_back(p);
    }
    d.tracks.push_back(t);
    studio::PluginDoc rec;
    rec.id = "MIX:@record";
    d.masterPlugins.push_back(rec);
    return d;
}

void a_press_bypasses_the_chain_and_leaves_the_rig() {
    const auto d = withBypassed(rig(), {"IN:amp", "IN:cab"});
    CHECK(!d.tracks[0].plugins[0].enabled);
    CHECK(!d.tracks[0].plugins[1].enabled);
    CHECK(d.tracks[0].plugins[2].enabled);
    CHECK(d.masterPlugins[0].enabled);
}

void a_step_already_bypassed_stays_bypassed() {
    auto r = rig();
    r.tracks[0].plugins[1].enabled = false;
    CHECK(!withBypassed(r, {"IN:amp"}).tracks[0].plugins[1].enabled);
}

void only_the_rise_is_a_press() {
    PressEdge e;
    CHECK(e.press(127));
    CHECK(!e.press(127));   // a pedal that repeats while held
    CHECK(!e.press(0));     // its release
    CHECK(e.press(100));
    CHECK(!e.press(63));
    CHECK(e.press(64));
}

void a_footswitch_with_nothing_to_bypass_is_none() {
    CHECK(!SlotFootswitch{}.bound());
    CHECK(!(SlotFootswitch{64, {}}.bound()));
    CHECK(!(SlotFootswitch{128, {"IN:amp"}}.bound()));
    CHECK((SlotFootswitch{64, {"IN:amp"}}.bound()));
}

}  // namespace

int main() {
    RUN(a_press_bypasses_the_chain_and_leaves_the_rig);
    RUN(a_step_already_bypassed_stays_bypassed);
    RUN(only_the_rise_is_a_press);
    RUN(a_footswitch_with_nothing_to_bypass_is_none);
    TEST_MAIN_END();
}
