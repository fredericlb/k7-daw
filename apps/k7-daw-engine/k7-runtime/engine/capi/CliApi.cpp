// CliApi.cpp — the cli area of the C ABI (k7_engine_cli.h) over EngineHost's one-shot modes: the documents read by
// DocReader, everything else passed through as it was asked.
// What a reader gets wrong: a document is already validated by the Rust side, so a throw here is a bug of its writer.

#include "k7_engine_cli.h"

#include "CApi.h"

#include "../DocReader.h"
#include "../EngineHost.h"

#include <nlohmann/json.hpp>

#include <exception>
#include <iostream>
#include <string>
#include <vector>

namespace studio::capi {
namespace {

int32_t scanChild(K7Str format, K7Str id) {
    return guard<int32_t>("scan_plugin_child", 1, [&] { return runPluginScanChild(str(format), str(id)); });
}

int32_t scan(int32_t force) {
    return guard<int32_t>("scan_plugins", 1, [&] { return runPluginScanCli(force != 0); });
}

int32_t dumpParams(K7Str type) {
    return guard<int32_t>("dump_params", 1, [&] { return dumpPluginParams(str(type)); });
}

int32_t dumpParamsExt(K7Str format, K7Str id, K7Str stateFile) {
    return guard<int32_t>("dump_params_ext", 1,
                          [&] { return dumpPluginParamsExternal(str(format), str(id), str(stateFile)); });
}

int32_t dumpBuiltins() { return guard<int32_t>("dump_builtin_params", 1, [] { return dumpBuiltinParams(); }); }

int32_t dumpSurge() { return guard<int32_t>("dump_surge_layout", 1, [] { return dumpSurgeLayout(); }); }

int32_t listDevices() { return guard<int32_t>("list_audio_devices", 1, [] { return runAudioDevicesCli(); }); }

// A document DocReader refuses is said the way the CLI said a bad project, with code 2.
bool read(K7Str json, ProjectDoc& out) {
    try {
        out = readDoc(nlohmann::json::parse(str(json)));
        return true;
    } catch (const std::exception& e) {
        std::cerr << "bad project: " << e.what() << "\n";
        return false;
    }
}

int32_t multiEdit(const K7Str* docs, size_t count, double seconds, K7Str device, K7Str deviceType) {
    std::vector<ProjectDoc> parsed(count);
    for (size_t i = 0; i < count; ++i)
        if (!read(docs[i], parsed[i])) return 2;
    const std::vector<std::string> baseDirs(count);
    return guard<int32_t>("multi_edit_test", 1,
                          [&] { return runMultiEditTest(parsed, baseDirs, seconds, str(device), str(deviceType)); });
}

int32_t render(K7Str docJson, K7Str baseDir, K7Str outDir, int32_t stems) {
    ProjectDoc doc;
    if (!read(docJson, doc)) return 2;
    return guard<int32_t>("render_offline", 1,
                          [&] { return renderOffline(doc, str(baseDir), str(outDir), stems != 0); });
}

int32_t decode(K7Str path) { return guard<int32_t>("decode", 1, [&] { return decodeToStdout(str(path)); }); }

const K7CliApi kCli{
    scanChild, scan, dumpParams, dumpParamsExt, dumpBuiltins, dumpSurge, listDevices, multiEdit, render, decode,
};

}  // namespace

const K7CliApi* cliApi() { return &kCli; }

}  // namespace studio::capi
