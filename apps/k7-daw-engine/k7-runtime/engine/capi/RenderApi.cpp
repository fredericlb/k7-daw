// RenderApi.cpp — the render area of the C ABI (k7_engine_render.h) over renderInProcess(): the validated document
// read by DocReader, bounced on the caller's thread, the outcome handed to `done` once.
// What a reader gets wrong: nothing here checks the paths, which the Rust server held to the roots before this call.

#include "k7_engine_render.h"

#include "CApi.h"

#include "../DocReader.h"
#include "../EngineHost.h"

#include <nlohmann/json.hpp>

#include <exception>
#include <string>

namespace studio::capi {
namespace {

void say(void* ctx, K7RenderDone done, int32_t code, const std::string& manifest, const std::string& error) {
    if (done == nullptr) return;
    const K7RenderOutcome outcome{code, view(manifest), view(error)};
    done(ctx, &outcome);
}

void render(K7Str docJson, K7Str outDir, int32_t stems, void* ctx, K7RenderDone done) {
    ProjectDoc doc;
    try {
        doc = readDoc(nlohmann::json::parse(str(docJson)));
    } catch (const std::exception& e) {
        say(ctx, done, 2, {}, std::string("bad project: ") + e.what());
        return;
    }
    RenderOutcome r;
    try {
        r = renderInProcess(doc, str(outDir), stems != 0);
    } catch (const std::exception& e) {
        r = {2, {}, std::string("render failed: ") + e.what()};
    }
    say(ctx, done, r.code, r.manifest, r.error);
}

void cancel() { guardVoid("cancel", [] { cancelInProcessRenders(); }); }

const K7RenderApi kRender{render, cancel};

}  // namespace

const K7RenderApi* renderApi() { return &kRender; }

}  // namespace studio::capi
