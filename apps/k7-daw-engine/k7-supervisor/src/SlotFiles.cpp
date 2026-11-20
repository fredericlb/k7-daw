// SlotFiles.cpp — see SlotFiles.h. The fields walked are the document's file fields (engine/k7-runtime/engine/ProjectDoc.h).

#include "SlotFiles.h"

#include "Paths.h"

#include <cctype>
#include <map>
#include <optional>
#include <set>
#include <system_error>
#include <vector>

namespace k7::slots {

namespace fs = std::filesystem;
using nlohmann::json;

namespace {

/** The formats named by the plugin's own scan entry rather than a file: the engine matches them by name (EditBuilder.cpp). */
const std::set<std::string> kThirdParty = {"vst3", "vst", "vst2", "au", "audiounit", "clap", "lv2", "ladspa", "aax"};

/** The plugin fields that each name one file the engine reads. */
const char* const kFileFields[] = {"path", "namFile", "irFile", "surgePatch", "deckFile"};

constexpr const char* kInstruments = "@instruments/";

std::string lower(std::string s)
{
    for (auto& c : s) c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
    return s;
}

class Bundler {
public:
    Bundler(const fs::path& stage, const fs::path& final, const Installed& installed)
        : stage_(stage / "files"), final_(final / "files"), installed_(installed)
    {
    }

    void doc(json& d)
    {
        if (d.contains("masterPlugins") && d["masterPlugins"].is_array())
            for (auto& p : d["masterPlugins"]) plugin(p);
        if (d.contains("tracks") && d["tracks"].is_array())
            for (auto& t : d["tracks"]) track(t);
        // Takes go into the slot's own folder: the compiling machine's record folder is not on this one.
        d["recordDir"] = paths::utf8(final_.parent_path() / "takes");
    }

    /** Why the machine cannot play it, joined; empty when it can. */
    std::string why() const
    {
        std::string out;
        for (const auto& m : missing_) out += (out.empty() ? "" : "; ") + m;
        return out;
    }

private:
    void track(json& t)
    {
        if (t.contains("plugins") && t["plugins"].is_array())
            for (auto& p : t["plugins"]) plugin(p);
        if (t.contains("clips") && t["clips"].is_array())
            for (auto& c : t["clips"]) file(c, "source");
    }

    void plugin(json& p)
    {
        const std::string format = lower(p.value("type", ""));
        const std::string name = p.value("name", "");
        if (kThirdParty.count(format) && !(installed_.plugin && installed_.plugin(format, name)))
            missing_.push_back("the plugin '" + name + "' (" + format + ") is not in this machine's plugin scan");
        // A slot never writes back: what it was saved with is what it plays.
        if (p.contains("presetSave")) p["presetSave"] = "";
        for (const char* f : kFileFields) file(p, f);
        sfz(p);
        if (p.contains("rack") && p["rack"].is_array())
            for (auto& r : p["rack"]) file(r, "clip");
        if (p.contains("patternBanks") && p["patternBanks"].is_array() && !p["patternBanks"].empty())
            missing_.push_back("a pattern lane's sample banks are folders a slot does not carry");
    }

    /** An installed instrument is the machine's own; a plain .sfz travels with the folder its samples are in. */
    void sfz(json& p)
    {
        if (!p.contains("sfzFile") || !p["sfzFile"].is_string()) return;
        const std::string v = p["sfzFile"].get<std::string>();
        if (v.empty()) return;
        if (v.rfind(kInstruments, 0) == 0) {
            const std::string rest = v.substr(std::string(kInstruments).size());
            const std::string name = rest.substr(0, rest.find('/'));
            if (!(installed_.instrument && installed_.instrument(name)))
                missing_.push_back("the instrument '" + name + "' is not installed on this machine");
            return;
        }
        const fs::path src = paths::fromUtf8(v);
        const auto dir = folderCopy(src.parent_path());
        if (!dir) return;
        p["sfzFile"] = paths::utf8(*dir / src.filename());
    }

    void file(json& o, const char* field)
    {
        if (!o.contains(field) || !o[field].is_string()) return;
        const std::string v = o[field].get<std::string>();
        if (v.empty()) return;
        o[field] = copied(v);
    }

    /** `v` copied once into the slot, by name under a number of its own; the path it will have there. */
    std::string copied(const std::string& v)
    {
        if (auto it = done_.find(v); it != done_.end()) return it->second;
        const fs::path src = paths::fromUtf8(v);
        std::error_code ec;
        if (installed_.readable && !installed_.readable(src)) {
            missing_.push_back("it names a file outside this machine's copies of the server's folders: " + v);
            return v;
        }
        if (!fs::is_regular_file(src, ec)) {
            missing_.push_back("it names a file this machine does not have: " + v);
            return v;
        }
        const std::string name = std::to_string(next_++) + "-" + paths::utf8(src.filename());
        fs::create_directories(stage_, ec);
        fs::copy_file(src, stage_ / paths::fromUtf8(name), fs::copy_options::overwrite_existing, ec);
        if (ec) {
            missing_.push_back("it could not copy " + v + ": " + ec.message());
            return v;
        }
        const std::string out = paths::utf8(final_ / paths::fromUtf8(name));
        done_[v] = out;
        return out;
    }

    /** A whole folder copied once, for a file whose neighbours it reads unnamed; the folder it will have there. */
    std::optional<fs::path> folderCopy(const fs::path& dir)
    {
        const std::string key = paths::utf8(dir) + "/";
        if (auto it = done_.find(key); it != done_.end()) return paths::fromUtf8(it->second);
        std::error_code ec;
        if (installed_.readable && !installed_.readable(dir)) {
            missing_.push_back("it names a folder outside this machine's copies of the server's folders: " + paths::utf8(dir));
            return std::nullopt;
        }
        if (!fs::is_directory(dir, ec)) {
            missing_.push_back("it names a folder this machine does not have: " + paths::utf8(dir));
            return std::nullopt;
        }
        const std::string name = std::to_string(next_++) + "-" + paths::utf8(dir.filename());
        fs::create_directories(stage_ / paths::fromUtf8(name), ec);
        fs::copy(dir, stage_ / paths::fromUtf8(name), fs::copy_options::recursive | fs::copy_options::overwrite_existing, ec);
        if (ec) {
            missing_.push_back("it could not copy " + paths::utf8(dir) + ": " + ec.message());
            return std::nullopt;
        }
        const fs::path out = final_ / paths::fromUtf8(name);
        done_[key] = paths::utf8(out);
        return out;
    }

    fs::path stage_;
    fs::path final_;
    const Installed& installed_;
    std::map<std::string, std::string> done_;
    std::vector<std::string> missing_;
    int next_ = 1;
};

}  // namespace

bool bundle(json& doc, const fs::path& stageDir, const fs::path& finalDir, const Installed& installed, std::string& error)
{
    Bundler b(stageDir, finalDir, installed);
    b.doc(doc);
    error = b.why();
    return error.empty();
}

}  // namespace k7::slots
