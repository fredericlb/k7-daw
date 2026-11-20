// Slots.h — the machine's SLOTS: frozen chains compiled once and kept here, one of which may be ENABLED, the one the
// engine plays in its `slot` session with nobody attached. The store is files under the state folder, written whole and
// renamed, so a crash leaves the last good one. What a reader would get wrong: enabling here is only the memory of
// it; the supervisor sends the document, and the engine drops it when a project opens. See docs/slots.md.

#pragma once

#include "SlotFiles.h"

#include <nlohmann/json.hpp>

#include <filesystem>
#include <optional>
#include <string>
#include <vector>

namespace k7::slots {

/** The engine session a slot plays in, the engine's `EngineHost::kSlotSession` (apps/k7-daw-engine/k7-runtime/docs/sessions.md). */
constexpr const char* kSession = "slot";

/** One slot as the app lists it: never its document, which is the engine's to read. */
struct Slot {
    std::string id;
    std::string name;
    /** The frozen chain it was compiled from, as the library names it (`@account/warm`). */
    std::string chain;
    /** When it was saved, ISO 8601 as k7 wrote it. */
    std::string savedAt;
    /** The wire protocol of the compiler that made it: an engine of another one may not read it. */
    int protocol = 0;
    /** The MIDI CC whose press bypasses its chain, on any channel; none when unset. */
    std::optional<int> footswitch;
};

/** Whether `id` may name a slot: a folder name on every platform, lowercase letters, digits and `-`, 64 at most. */
bool validId(const std::string& id);

/** Every slot under `dir`, by id; one whose file does not read is left out. */
std::vector<Slot> list(const std::filesystem::path& dir);

/** The enabled slot's id, if one is enabled and still there. */
std::optional<std::string> enabled(const std::filesystem::path& dir);

/** Remember `id` as the enabled one, or none. */
void setEnabled(const std::filesystem::path& dir, const std::optional<std::string>& id);

/** Store the slot `staged` holds (`{id, name, chain, savedAt, protocol, doc}`), its files copied in (SlotFiles.h), replacing one of that id and keeping its footswitch; its id, or nothing and `error`. */
std::optional<std::string> save(const std::filesystem::path& dir, const std::filesystem::path& staged, const Installed& installed,
                                std::string& error);

/** Forget slot `id`, and the enabled mark when it named it; false when there was none. */
bool remove(const std::filesystem::path& dir, const std::string& id);

/** Bind slot `id`'s footswitch to CC `cc` (0..127), or none; false when there is no such slot or no such CC. */
bool setFootswitch(const std::filesystem::path& dir, const std::string& id, const std::optional<int>& cc);

/** Slot `id`'s compiled document, as the engine's `project` message carries it, its footswitch added when it has one. */
std::optional<nlohmann::json> document(const std::filesystem::path& dir, const std::string& id);

}  // namespace k7::slots
