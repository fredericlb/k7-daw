// StateApi.cpp — the state area of the C ABI (k7_engine_state.h) over EngineHost's state timer and its sinks.
// What a reader gets wrong: a frame is flattened into arrays that live on this function's stack for the one call
// of the sink, which is why the server copies what it keeps.

#include "k7_engine_state.h"

#include "CApi.h"

#include "../EngineHost.h"

#include <string>
#include <utility>
#include <vector>

namespace studio::capi {
namespace {

EngineHost* host(K7Host* h) { return reinterpret_cast<EngineHost*>(h); }

K7Envelope envelope(const WaveformEnvelope& e) { return K7Envelope{e.min, e.max}; }

std::vector<K7Envelope> envelopes(const std::vector<WaveformEnvelope>& in) {
    std::vector<K7Envelope> out;
    out.reserve(in.size());
    for (const auto& e : in) out.push_back(envelope(e));
    return out;
}

// Hands one session's snapshot to the sink: every array below outlives the call and nothing else.
void deliver(const K7StateSinks& sinks, const EngineHost::EngineState& s) {
    std::vector<K7Str> armed;
    for (const auto& id : s.armedTracks) armed.push_back(view(id));

    std::vector<std::vector<K7Envelope>> waveformPoints;
    waveformPoints.reserve(s.waveformEnvelopes.size());
    std::vector<K7WaveformEntry> waveforms;
    for (const auto& [key, points] : s.waveformEnvelopes) {
        waveformPoints.push_back(envelopes(points));
        waveforms.push_back(K7WaveformEntry{view(key), waveformPoints.back().data(), waveformPoints.back().size()});
    }

    std::vector<K7PlatterEntry> platters;
    for (const auto& [id, seconds] : s.platterSeconds) platters.push_back(K7PlatterEntry{view(id), seconds});

    std::vector<std::vector<K7TakeInfo>> takeLists;
    std::vector<std::vector<K7Envelope>> pictures;
    takeLists.reserve(s.recorders.size());
    pictures.reserve(s.recorders.size());
    std::vector<K7RecorderEntry> recorders;
    for (const auto& [id, r] : s.recorders) {
        std::vector<K7TakeInfo> takes;
        for (const auto& t : r.takes) takes.push_back(K7TakeInfo{view(t.file), t.seconds, t.damaged ? 1 : 0});
        takeLists.push_back(std::move(takes));
        pictures.push_back(envelopes(r.envelope));
        recorders.push_back(K7RecorderEntry{view(id), r.recording ? 1 : 0, r.seconds, takeLists.back().data(),
                                            takeLists.back().size(), pictures.back().data(), pictures.back().size(),
                                            static_cast<uint64_t>(r.envelopeFrom)});
    }

    std::vector<K7LoadEntry> loads;
    for (const auto& [id, l] : s.pluginLoads) loads.push_back(K7LoadEntry{view(id), l.cpu, l.bytes});

    const K7StateFrame frame{view(s.session),
                             s.playing ? 1 : 0,
                             s.playheadSeconds,
                             s.loop.on ? 1 : 0,
                             s.loop.startSeconds,
                             s.loop.endSeconds,
                             s.trackLevelsDb.data(),
                             s.trackLevelsDb.size(),
                             s.trackInputLevelsDb.data(),
                             s.trackInputLevelsDb.size(),
                             armed.data(),
                             armed.size(),
                             s.recordEnabled ? 1 : 0,
                             s.masterLevelDb,
                             s.masterOverload ? 1 : 0,
                             waveforms.data(),
                             waveforms.size(),
                             platters.data(),
                             platters.size(),
                             recorders.data(),
                             recorders.size(),
                             s.hasLoad ? 1 : 0,
                             loads.data(),
                             loads.size(),
                             s.processBytes};
    sinks.state(sinks.ctx, &frame);
}

void start(K7Host* h, const K7StateSinks* in, int32_t intervalMs) {
    guardVoid("state.start", [h, in, intervalMs] {
        const K7StateSinks sinks = *in;
        auto* eh = host(h);
        if (sinks.recorded != nullptr)
            eh->onRecorded([sinks](const std::string& session, const std::string& track, const std::string& file,
                                   double startSeconds, double lengthSeconds) {
                sinks.recorded(sinks.ctx, view(session), view(track), view(file), startSeconds, lengthSeconds);
            });
        if (sinks.params != nullptr)
            eh->onPluginParams([sinks](const std::string& session, const std::string& id, const std::string& json) {
                sinks.params(sinks.ctx, view(session), view(id), view(json));
            });
        if (sinks.params_live != nullptr)
            eh->onPluginParamsLive([sinks](const std::string& session, const std::string& id,
                                           const std::vector<std::pair<std::string, double>>& params) {
                std::vector<K7ParamValue> values;
                values.reserve(params.size());
                for (const auto& [name, value] : params) values.push_back(K7ParamValue{view(name), value});
                sinks.params_live(sinks.ctx, view(session), view(id), values.data(), values.size());
            });
        if (sinks.state != nullptr)
            eh->onState([sinks](const EngineHost::EngineState& s) { deliver(sinks, s); }, intervalMs);
    });
}

void rewind(K7Host* h, K7Str session) {
    guardVoid("rewind_recorder_envelopes", [h, session] { host(h)->rewindRecorderEnvelopes(str(session)); });
}

void watch(K7Host* h, K7Str session, K7Str id, int32_t on) {
    guardVoid("watch_plugin_params",
              [h, session, id, on] { host(h)->watchPluginParams(str(session), str(id), on != 0); });
}

void infos(K7Host* h, K7Str session, K7Str id, void* ctx, K7ParamInfoEach each) {
    guardVoid("plugin_param_infos", [h, session, id, ctx, each] {
        if (each == nullptr) return;
        for (const auto& p : host(h)->pluginParamInfos(str(session), str(id))) {
            std::vector<K7ParamState> states;
            for (const auto& [label, value] : p.states) states.push_back(K7ParamState{view(label), value});
            const K7ParamInfo info{view(p.name), p.value, p.min, p.max, p.hasDefault ? 1 : 0, p.def,
                                   states.data(),  states.size()};
            each(ctx, &info);
        }
    });
}

const K7StateApi kState{start, rewind, watch, infos};

}  // namespace

const K7StateApi* stateApi() { return &kState; }

}  // namespace studio::capi
