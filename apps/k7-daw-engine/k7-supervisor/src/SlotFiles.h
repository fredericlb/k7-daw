// SlotFiles.h — making a slot's document stand on its own: every file it names is copied into the slot's folder and
// its path rewritten there, so the slot plays on a machine that has none of the paths it was compiled with. What a
// reader would get wrong: an SFZ's samples sit BESIDE it, unnamed, so its whole folder is copied. See docs/slots.md.

#pragma once

#include <nlohmann/json.hpp>

#include <filesystem>
#include <functional>
#include <string>

namespace k7::slots {

/** What this machine has, asked as the document is walked. */
struct Installed {
    /** A third-party plugin by its format word and name, as the engine's scan lists it. */
    std::function<bool(const std::string& format, const std::string& name)> plugin;
    /** An instrument of `<state>/instruments`, by name. */
    std::function<bool(const std::string& name)> instrument;
    /** Whether a file the document names may be copied in; unset, any may — a server's job may name only its own copies. */
    std::function<bool(const std::filesystem::path& file)> readable;
};

/**
 * Copy what `doc` names into `stageDir/files` and rewrite each path as it will be under `finalDir/files`, the folder
 * the slot is renamed into once whole; false, and `error` saying what is missing, when the machine cannot play it.
 */
bool bundle(nlohmann::json& doc, const std::filesystem::path& stageDir, const std::filesystem::path& finalDir,
            const Installed& installed, std::string& error);

}  // namespace k7::slots
