// The settings folder's choice: the flag beats the env, and the flag leaves argv as if it was never there.
//
// What would be wrong SILENTLY: the flag staying in argv, where the port parse reads it as a number.

#include "SettingsDir.h"

#include "test_harness.h"

#include <cstdlib>
#include <filesystem>

using namespace studio;

namespace {

void theFlagBeatsTheEnv() {
    CHECK(pickSettingsDir("/a", "/b") == "/a");
    CHECK(pickSettingsDir("", "/b") == "/b");
    CHECK(pickSettingsDir("", "").empty());
}

void theFlagLeavesArgvAndTheFolderIsMade() {
    const auto dir = std::filesystem::temp_directory_path() / "k7-settingsdir-test" / "made";
    std::filesystem::remove_all(dir.parent_path());
    const std::string d = dir.string();
    char a0[] = "k7-runtime", a1[] = "--render", a2[] = "--settings-dir", a4[] = "doc.json";
    char* argv[] = {a0, a1, a2, const_cast<char*>(d.c_str()), a4, nullptr};
    int argc = 5;
    initSettingsDir(argc, argv);
    CHECK(argc == 3);
    CHECK(std::string(argv[1]) == "--render" && std::string(argv[2]) == "doc.json");
    CHECK(settingsDir() == d);
    CHECK(std::filesystem::is_directory(dir));
    std::filesystem::remove_all(dir.parent_path());
}

}  // namespace

int main() {
    RUN(theFlagBeatsTheEnv);
    RUN(theFlagLeavesArgvAndTheFolderIsMade);
    TEST_MAIN_END();
}
