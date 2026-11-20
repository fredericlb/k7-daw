// SettingsDir.h — where the engine keeps Settings.xml and the plugin-scan cache when a harness asks for
// its own folder: `--settings-dir <folder>`, else $K7_SETTINGS_DIR. JUCE-free, so tests/ has it in a FAST binary.
// The one thing a reader would get wrong: unset means JUCE's and ~/.k7's own places, so a test that
// forgets the option writes the user's real files. See docs/settingsDir.md.

#pragma once

#include <string>

namespace studio {

/** The folder to use: the flag if given, else the env value, else "" (the user's own places). */
std::string pickSettingsDir(const std::string& flag, const std::string& env);

/** Reads the flag out of argv (removing it, so positional parsing is untouched) and $K7_SETTINGS_DIR, once, at startup. */
void initSettingsDir(int& argc, char** argv);

/** The chosen folder, created; "" when none was chosen. */
const std::string& settingsDir();

}  // namespace studio
