// ProjectApi.cpp — the project area of the C ABI (k7_engine_project.h) over EngineHost: a validated document
// read by DocReader, applied, its changes described. What a reader gets wrong: nothing here checks the
// document; a throw is the Rust side's bug and is answered through `error`, never left to cross the ABI.

#include "k7_engine_project.h"

#include "CApi.h"

#include "../DocReader.h"
#include "../EngineHost.h"
#include "../PathRoots.h"
#include "../ProjectDiff.h"
#include "../Warnings.h"

#include <nlohmann/json.hpp>

#include <exception>
#include <string>
#include <utility>

namespace studio::capi {
namespace {

EngineHost* host(K7Host* h) { return reinterpret_cast<EngineHost*>(h); }

int32_t pathAllowed(K7Str path, int32_t write) {
    return guard<int32_t>("path_allowed", 0, [&] {
        const std::string p = str(path);
        return (write != 0 ? writeAllowed(p) : readAllowed(p)) ? 1 : 0;
    });
}

void warnUnder(K7Str session, K7Str subject, K7Str message) {
    guardVoid("warn", [&] {
        WarningScope scope(str(session));
        warn(str(message), str(subject));
    });
}

void standing(K7Str session, void* ctx, K7WarningSink each) {
    guardVoid("standing_warnings", [&] {
        if (each == nullptr) return;
        for (const auto& w : standingWarnings(str(session))) {
            const K7Warning out{view(w.session), view(w.subject), view(w.message)};
            each(ctx, &out);
        }
    });
}

int32_t releaseSlot(K7Host* h) { return guard<int32_t>("release_slot", 0, [h] { return host(h)->releaseSlot() ? 1 : 0; }); }

// The failure of a call, said through `error` and answered 0.
int32_t failed(const char* what, void* ctx, K7StrEach error, const std::exception& e) {
    std::cerr << "[capi] " << what << " failed: " << e.what() << "\n";
    give(error, ctx, e.what());
    return 0;
}

int32_t apply(K7Host* h, K7Str session, K7Str docJson, void* ctx, K7StrEach change, K7StrEach error) {
    try {
        const auto j = nlohmann::json::parse(str(docJson));
        const std::string id = str(session);
        const ProjectDiff changes = host(h)->apply(id, readDoc(j), "");
        // The footswitch rides the slot's chain just applied, so it is bound only after a successful apply.
        if (id == EngineHost::kSlotSession) host(h)->setSlotFootswitch(readFootswitch(j));
        for (const auto& c : changes) give(change, ctx, describe(c));
        return 1;
    } catch (const std::exception& e) {
        return failed("apply", ctx, error, e);
    }
}

void trackIds(K7Host* h, K7Str session, void* ctx, K7StrEach each) {
    guardVoid("track_ids", [&] {
        for (const auto& t : host(h)->current(str(session)).tracks) give(each, ctx, t.id);
    });
}

int32_t applyTracks(K7Host* h, K7Str session, K7Str planJson, void* ctx, K7StrEach error) {
    try {
        const auto plan = nlohmann::json::parse(str(planJson));
        const std::string id = str(session);
        ProjectDoc doc = host(h)->current(id);
        for (const auto& step : plan) {
            const auto index = step.at("index").get<size_t>();
            if (index >= doc.tracks.size()) throw std::out_of_range("track index past the document");
            doc.tracks[index] = readTrack(step.at("track"));
        }
        host(h)->apply(id, std::move(doc), "");
        return 1;
    } catch (const std::exception& e) {
        return failed("apply_tracks", ctx, error, e);
    }
}

void reset(K7Host* h, K7Str session) { guardVoid("reset", [&] { host(h)->reset(str(session)); }); }

const K7ProjectApi kProject{
    pathAllowed, warnUnder, standing, releaseSlot, apply, trackIds, applyTracks, reset,
};

}  // namespace

const K7ProjectApi* projectApi() { return &kProject; }

}  // namespace studio::capi
