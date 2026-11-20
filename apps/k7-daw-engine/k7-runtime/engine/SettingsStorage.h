// SettingsStorage.h — Tracktion's property storage, aimed at `settingsDir()` when one was chosen.
// The one thing a reader would get wrong: the default storage asks macOS for the home folder, so
// setting $HOME does not move Settings.xml; only this does. Internal: includes Tracktion.

#pragma once

#include "EngineTypes.h"
#include "SettingsDir.h"

#include <memory>

namespace studio {

struct SettingsStorage : te::PropertyStorage {
    using te::PropertyStorage::PropertyStorage;

    juce::File getAppPrefsFolder() override {
        if (settingsDir().empty()) return te::PropertyStorage::getAppPrefsFolder();
        juce::File f(settingsDir());
        f.createDirectory();
        return f;
    }
};

inline std::unique_ptr<te::PropertyStorage> makePropertyStorage() { return std::make_unique<SettingsStorage>("Studio"); }

}  // namespace studio
