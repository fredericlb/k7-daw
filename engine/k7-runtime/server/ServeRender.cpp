// ServeRender.cpp — `render`: a ProjectDoc bounced to wav INSIDE this engine, for a machine that may start no program,
// answered with the manifest `--render` prints. One at a time, the machine's own clients only, every path held to the
// roots before anything is read (docs/protocol.md, docs/cliModes.md).
// The one thing a reader would get wrong: the render runs on a detached worker, since it needs the message loop running.

#include "ServeContext.h"
#include "DocJson.h"

#include "../engine/PathRoots.h"

#include <nlohmann/json.hpp>

#include <atomic>
#include <filesystem>
#include <iostream>
#include <mutex>
#include <string>

#include <EngineHost.h>

using json = nlohmann::json;

namespace studio {

namespace {

// The doc with every relative clip source joined to `base`, so the roots test what the render will open.
json withBase(json doc, const std::string& base) {
    if (base.empty() || !doc.contains("tracks") || !doc.at("tracks").is_array()) return doc;
    for (auto& t : doc.at("tracks")) {
        if (!t.is_object() || !t.contains("clips") || !t.at("clips").is_array()) continue;
        for (auto& c : t.at("clips")) {
            if (!c.is_object() || !c.contains("source") || !c.at("source").is_string()) continue;
            const auto source = std::filesystem::u8path(c.at("source").get<std::string>());
            if (source.empty() || !source.is_relative()) continue;
            const auto joined = (std::filesystem::u8path(base) / source).lexically_normal().u8string();
            c["source"] = std::string(joined.begin(), joined.end());
        }
    }
    return doc;
}

// Clears the one-render flag on every way out of the worker.
struct RenderSlot {
    std::atomic<bool>& rendering;
    ~RenderSlot() { rendering = false; }
};

}  // namespace

void addRenderHandlers(HandlerTable& t) {
    t.add({"render"}, [](ServeContext& ctx, ix::WebSocket& ws, const json& j, const std::string&, const std::string&) {
        const ix::WebSocket* asker = &ws;
        const json id = j.contains("id") ? j.at("id") : json();
        const auto refuse = [&](const std::string& why) {
            ctx.sendTo(asker, json{{"type", "rendered"}, {"id", id}, {"ok", false}, {"error", why}});
        };
        // It writes files and takes a core for seconds: a LAN device has no business asking (docs/pairing.md).
        {
            std::lock_guard<std::mutex> lk(ctx.trustMx);
            if (ctx.local.count(asker) == 0) return refuse("only the machine running the engine can ask a render");
        }
        const std::string outDir = j.value("outDir", std::string{});
        const std::string base = j.value("base", std::string{});
        const bool stems = j.value("stems", false);
        if (!j.contains("doc") || !j.at("doc").is_object() || outDir.empty())
            return refuse("render: a doc and an outDir are required");
        if (!writeAllowed(outDir)) return refuse("render: '" + outDir + "' is outside the folders this engine writes");
        if (!base.empty() && !readAllowed(base)) return refuse("render: '" + base + "' is outside the folders this engine reads");

        // Refused WHOLE when one path is outside the roots: a render missing a clip is a wrong file, not a warning.
        ProjectDoc doc;
        {
            PathRefusals refused;
            try {
                doc = docFromJson(withBase(j.at("doc"), base));
            } catch (const std::exception& e) {
                return refuse(std::string("bad project: ") + e.what());
            }
            if (!refused.paths.empty())
                return refuse("render: '" + refused.paths.front() + "' is outside the folders this engine may open");
        }

        if (ctx.rendering.exchange(true)) {
            ctx.sendTo(asker, json{{"type", "rendered"}, {"id", id}, {"ok", false}, {"busy", true},
                                   {"error", "a render is already running on this engine"}});
            return;
        }
        std::cerr << "[render] started" << (stems ? " with stems" : "") << "\n";
        ctx.work.spawn([&ctx, asker, id, doc = std::move(doc), outDir, stems] {
            const RenderSlot slot{ctx.rendering};
            RenderOutcome r;
            try {
                r = renderInProcess(doc, outDir, stems);
            } catch (const std::exception& e) {
                r = {2, {}, std::string("render failed: ") + e.what()};
            }
            if (r.code != 0) {
                std::cerr << "[render] " << r.error << "\n";
                ctx.sendTo(asker, json{{"type", "rendered"}, {"id", id}, {"ok", false}, {"error", r.error}});
                return;
            }
            std::cerr << "[render] done\n";
            ctx.sendTo(asker, json{{"type", "rendered"}, {"id", id}, {"ok", true},
                                   {"manifest", json::parse(r.manifest, nullptr, false)}});
        });
    });
}

}  // namespace studio
