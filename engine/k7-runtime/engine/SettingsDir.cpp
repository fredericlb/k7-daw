// SettingsDir.cpp — see SettingsDir.h.

#include "SettingsDir.h"

#include <cstdlib>
#include <filesystem>
#include <system_error>

namespace studio {

namespace {
std::string g_dir;
}

std::string pickSettingsDir(const std::string& flag, const std::string& env) { return flag.empty() ? env : flag; }

void initSettingsDir(int& argc, char** argv) {
    std::string flag;
    int out = 1;
    for (int i = 1; i < argc; ++i) {
        if (std::string(argv[i]) == "--settings-dir" && i + 1 < argc) {
            flag = argv[++i];
            continue;
        }
        argv[out++] = argv[i];
    }
    argc = out;
    const char* env = std::getenv("K7_SETTINGS_DIR");
    g_dir = pickSettingsDir(flag, env ? env : "");
    if (g_dir.empty()) return;
    std::error_code ec;
    std::filesystem::create_directories(g_dir, ec);
}

const std::string& settingsDir() { return g_dir; }

}  // namespace studio
