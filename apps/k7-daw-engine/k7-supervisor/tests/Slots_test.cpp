// Slots_test.cpp — the slot store in a folder of its own: saved, listed without its document, enabled and
// remembered, forgotten with its mark, nothing written for a slot that is not one; and the files a slot plays carried
// into it, or the slot refused for what the machine lacks; and its footswitch, kept across a save and sent with it.

#include "Paths.h"
#include "Slots.h"

#include "test_harness.h"

#include <cstdlib>
#include <fstream>

namespace fs = std::filesystem;
using nlohmann::json;
namespace slots = k7::slots;

namespace {

fs::path freshDir()
{
    const auto dir = fs::temp_directory_path() / ("k7-slots-" + std::to_string(std::rand()));
    fs::remove_all(dir);
    fs::create_directories(dir);
    return dir;
}

fs::path stage(const fs::path& dir, const json& slot)
{
    const auto p = dir / "staged.json";
    std::ofstream(p, std::ios::binary) << slot.dump();
    return p;
}

json aSlot(const std::string& id)
{
    return {{"id", id},
            {"name", "Warm"},
            {"chain", "@account/warm"},
            {"savedAt", "2026-11-01T18:00:00Z"},
            {"protocol", 7},
            {"doc", {{"name", "warm"}, {"tracks", json::array()}}}};
}

void aSavedSlotIsListedWithoutItsDocument()
{
    const auto dir = freshDir();
    std::string error;
    const auto id = slots::save(dir, stage(dir, aSlot("warm")), slots::Installed{}, error);
    CHECK(id == std::optional<std::string>("warm"));
    const auto all = slots::list(dir);
    CHECK(all.size() == 1);
    CHECK(all[0].chain == "@account/warm");
    CHECK(all[0].protocol == 7);
    CHECK(slots::document(dir, "warm")->at("name") == "warm");
    // The staged file is consumed: a second save of it finds nothing.
    CHECK(!fs::exists(dir / "staged.json"));
    fs::remove_all(dir);
}

void theEnabledSlotIsRememberedAndGoesWithIt()
{
    const auto dir = freshDir();
    std::string error;
    slots::save(dir, stage(dir, aSlot("warm")), slots::Installed{}, error);
    CHECK(!slots::enabled(dir));
    slots::setEnabled(dir, std::string("warm"));
    CHECK(slots::enabled(dir) == std::optional<std::string>("warm"));
    CHECK(slots::remove(dir, "warm"));
    CHECK(!slots::enabled(dir));
    CHECK(slots::list(dir).empty());
    CHECK(!slots::remove(dir, "warm"));
    fs::remove_all(dir);
}

void aMarkNamingNoSlotIsNoMark()
{
    const auto dir = freshDir();
    slots::setEnabled(dir, std::string("gone"));
    CHECK(!slots::enabled(dir));
    fs::remove_all(dir);
}

void whatIsNoSlotIsRefusedAndNothingWritten()
{
    const auto dir = freshDir();
    std::string error;
    auto bad = aSlot("../escape");
    CHECK(!slots::save(dir, stage(dir, bad), slots::Installed{}, error));
    CHECK(!error.empty());
    auto noDoc = aSlot("warm");
    noDoc.erase("doc");
    CHECK(!slots::save(dir, stage(dir, noDoc), slots::Installed{}, error));
    CHECK(slots::list(dir).empty());
    CHECK(!slots::validId("Warm"));
    CHECK(slots::validId("warm-2"));
    fs::remove_all(dir);
}

void savingAgainReplacesTheSlot()
{
    const auto dir = freshDir();
    std::string error;
    slots::save(dir, stage(dir, aSlot("warm")), slots::Installed{}, error);
    auto again = aSlot("warm");
    again["savedAt"] = "2026-11-02T09:00:00Z";
    slots::save(dir, stage(dir, again), slots::Installed{}, error);
    const auto all = slots::list(dir);
    CHECK(all.size() == 1);
    CHECK(all[0].savedAt == "2026-11-02T09:00:00Z");
    fs::remove_all(dir);
}

void write(const fs::path& p, const std::string& text)
{
    fs::create_directories(p.parent_path());
    std::ofstream(p, std::ios::binary) << text;
}

/** A rig whose amp names a model and an IR, whose sampler plays an SFZ with a sample beside it. */
json aRigNaming(const fs::path& src)
{
    write(src / "amp.nam", "model");
    write(src / "cab.wav", "ir");
    write(src / "kit" / "kit.sfz", "<region> sample=snare.wav");
    write(src / "kit" / "snare.wav", "snare");
    auto slot = aSlot("rig");
    slot["doc"] = {{"name", "rig"},
                   {"recordDir", "/somewhere/else"},
                   {"masterPlugins", json::array()},
                   {"tracks",
                    {{{"plugins",
                       {{{"type", "nam"},
                         {"name", "amp"},
                         {"presetSave", "/somewhere/amp.vstpreset"},
                         {"namFile", k7::paths::utf8(src / "amp.nam")},
                         {"irFile", k7::paths::utf8(src / "cab.wav")}},
                        {{"type", "sfz"}, {"name", "kit"}, {"sfzFile", k7::paths::utf8(src / "kit" / "kit.sfz")}}}}}}}};
    return slot;
}

void aSlotCarriesTheFilesItPlays()
{
    const auto dir = freshDir();
    const auto src = dir / "source";
    std::string error;
    const auto id = slots::save(dir, stage(dir, aRigNaming(src)), slots::Installed{}, error);
    CHECK(id == std::optional<std::string>("rig"));
    fs::remove_all(src);
    const auto doc = slots::document(dir, "rig");
    const auto& amp = (*doc)["tracks"][0]["plugins"][0];
    const auto& kit = (*doc)["tracks"][0]["plugins"][1];
    // Every path now under the slot's own folder, and what it names there, the source gone.
    const auto under = k7::paths::utf8(dir / "rig" / "files");
    CHECK(amp["namFile"].get<std::string>().rfind(under, 0) == 0);
    CHECK(fs::exists(k7::paths::fromUtf8(amp["namFile"].get<std::string>())));
    CHECK(fs::exists(k7::paths::fromUtf8(amp["irFile"].get<std::string>())));
    const auto sfz = k7::paths::fromUtf8(kit["sfzFile"].get<std::string>());
    CHECK(fs::exists(sfz));
    CHECK(fs::exists(sfz.parent_path() / "snare.wav"));
    // Nothing written back, and takes in the slot.
    CHECK(amp["presetSave"] == "");
    CHECK((*doc)["recordDir"] == k7::paths::utf8(dir / "rig" / "takes"));
    CHECK(!fs::exists(dir / "rig.new"));
    fs::remove_all(dir);
}

void whatTheMachineLacksRefusesTheSlotAndKeepsTheOldOne()
{
    const auto dir = freshDir();
    std::string error;
    slots::save(dir, stage(dir, aSlot("rig")), slots::Installed{}, error);
    auto slot = aSlot("rig");
    slot["doc"]["tracks"] = {{{"plugins",
                               {{{"type", "VST3"}, {"name", "Diva"}},
                                {{"type", "sfz"}, {"name", "wurli"}, {"sfzFile", "@instruments/nbo-wurt/wurli.sfz"}}}}}};
    slots::Installed has;
    has.plugin = [](const std::string& f, const std::string& n) { return f == "vst3" && n == "Pro-Q"; };
    has.instrument = [](const std::string&) { return false; };
    CHECK(!slots::save(dir, stage(dir, slot), has, error));
    CHECK(error.find("Diva") != std::string::npos);
    CHECK(error.find("nbo-wurt") != std::string::npos);
    // The slot it would have replaced is still the one there.
    CHECK(slots::document(dir, "rig")->at("name") == "warm");
    CHECK(!fs::exists(dir / "rig.new"));
    fs::remove_all(dir);
}

}  // namespace

void theFootswitchIsTheMachinesAndRidesWithTheDocument()
{
    const auto dir = freshDir();
    std::string error;
    auto slot = aSlot("warm");
    slot["chainPlugins"] = {"IN:amp"};
    slots::save(dir, stage(dir, slot), slots::Installed{}, error);
    CHECK(!slots::document(dir, "warm")->contains("footswitch"));
    CHECK(!slots::setFootswitch(dir, "warm", 128));
    CHECK(!slots::setFootswitch(dir, "gone", 64));
    CHECK(slots::setFootswitch(dir, "warm", 64));
    CHECK(slots::list(dir)[0].footswitch == std::optional<int>(64));
    const auto doc = *slots::document(dir, "warm");
    CHECK(doc["footswitch"]["cc"] == 64);
    CHECK(doc["footswitch"]["plugins"] == json::array({"IN:amp"}));
    // Saved again from the chain, it keeps its pedal; let go, it has none.
    slots::save(dir, stage(dir, slot), slots::Installed{}, error);
    CHECK(slots::list(dir)[0].footswitch == std::optional<int>(64));
    CHECK(slots::setFootswitch(dir, "warm", std::nullopt));
    CHECK(!slots::list(dir)[0].footswitch);
    fs::remove_all(dir);
}

void runSlotsTests()
{
    RUN(theFootswitchIsTheMachinesAndRidesWithTheDocument);
    RUN(aSlotCarriesTheFilesItPlays);
    RUN(whatTheMachineLacksRefusesTheSlotAndKeepsTheOldOne);
    RUN(aSavedSlotIsListedWithoutItsDocument);
    RUN(theEnabledSlotIsRememberedAndGoesWithIt);
    RUN(aMarkNamingNoSlotIsNoMark);
    RUN(whatIsNoSlotIsRefusedAndNothingWritten);
    RUN(savingAgainReplacesTheSlot);
}
