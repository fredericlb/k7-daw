// Slots.cpp — see Slots.h. A slot is `<dir>/<id>/slot.json`; the enabled one is named by `<dir>/enabled`.

#include "Slots.h"

#include "SlotFiles.h"

#include <algorithm>
#include <fstream>
#include <sstream>
#include <system_error>

namespace k7::slots {

namespace fs = std::filesystem;
using nlohmann::json;

namespace {

constexpr const char* kSlotFile = "slot.json";
constexpr const char* kEnabledFile = "enabled";

std::optional<std::string> readText(const fs::path& p)
{
    std::ifstream in(p, std::ios::binary);
    if (!in) return std::nullopt;
    std::ostringstream s;
    s << in.rdbuf();
    return s.str();
}

/** Written beside, then renamed over: a crash mid-write leaves the last whole file. */
bool writeText(const fs::path& p, const std::string& text)
{
    std::error_code ec;
    fs::create_directories(p.parent_path(), ec);
    const auto tmp = p.parent_path() / (p.filename().string() + ".tmp");
    {
        std::ofstream out(tmp, std::ios::binary | std::ios::trunc);
        if (!out) return false;
        out << text;
        if (!out) {
            out.close();
            fs::remove(tmp, ec);
            return false;
        }
    }
    fs::rename(tmp, p, ec);
    if (ec) fs::remove(tmp, ec);
    return !ec;
}

std::optional<json> readSlot(const fs::path& dir, const std::string& id)
{
    if (!validId(id)) return std::nullopt;
    const auto text = readText(dir / id / kSlotFile);
    if (!text) return std::nullopt;
    auto j = json::parse(*text, nullptr, false);
    if (!j.is_object() || !j.contains("doc") || !j["doc"].is_object()) return std::nullopt;
    return j;
}

std::string str(const json& j, const char* key) { return j.contains(key) && j[key].is_string() ? j[key].get<std::string>() : ""; }

std::optional<int> footswitchOf(const json& j)
{
    if (!j.contains("footswitch") || !j["footswitch"].is_number_integer()) return std::nullopt;
    const int cc = j["footswitch"].get<int>();
    if (cc < 0 || cc > 127) return std::nullopt;
    return cc;
}

}  // namespace

bool validId(const std::string& id)
{
    if (id.empty() || id.size() > 64) return false;
    for (const char c : id)
        if (!((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '-')) return false;
    return true;
}

std::vector<Slot> list(const fs::path& dir)
{
    std::vector<Slot> out;
    std::error_code ec;
    for (const auto& e : fs::directory_iterator(dir, ec)) {
        if (!e.is_directory(ec)) continue;
        const auto id = e.path().filename().string();
        const auto j = readSlot(dir, id);
        if (!j) continue;
        out.push_back({id, str(*j, "name"), str(*j, "chain"), str(*j, "savedAt"),
                       (*j).value("protocol", 0), footswitchOf(*j)});
    }
    std::sort(out.begin(), out.end(), [](const Slot& a, const Slot& b) { return a.id < b.id; });
    return out;
}

std::optional<std::string> enabled(const fs::path& dir)
{
    auto text = readText(dir / kEnabledFile);
    if (!text) return std::nullopt;
    while (!text->empty() && (text->back() == '\n' || text->back() == '\r' || text->back() == ' ')) text->pop_back();
    // A mark naming a slot that is gone is no mark: the machine boots into nothing rather than an error.
    if (!readSlot(dir, *text)) return std::nullopt;
    return *text;
}

void setEnabled(const fs::path& dir, const std::optional<std::string>& id)
{
    std::error_code ec;
    if (!id) {
        fs::remove(dir / kEnabledFile, ec);
        return;
    }
    writeText(dir / kEnabledFile, *id + "\n");
}

std::optional<std::string> save(const fs::path& dir, const fs::path& staged, const Installed& installed, std::string& error)
{
    const auto text = readText(staged);
    if (!text) {
        error = "no slot to save at " + staged.string();
        return std::nullopt;
    }
    auto j = json::parse(*text, nullptr, false);
    if (!j.is_object() || !j.contains("doc") || !j["doc"].is_object()) {
        error = "a slot is {id, name, chain, savedAt, protocol, doc}, doc an object";
        return std::nullopt;
    }
    const auto id = str(j, "id");
    if (!validId(id)) {
        error = "a slot's id is lowercase letters, digits and '-', 64 at most";
        return std::nullopt;
    }
    // The pedal is the machine's, not the chain's: saving the chain again keeps what it was bound to.
    if (const auto old = readSlot(dir, id); old && footswitchOf(*old)) j["footswitch"] = *footswitchOf(*old);
    else j.erase("footswitch");
    // Built whole beside, then swapped in: a refused or failed save leaves the slot it replaces as it was.
    std::error_code ec;
    const auto stage = dir / (id + ".new");
    fs::remove_all(stage, ec);
    if (!bundle(j["doc"], stage, dir / id, installed, error) || !writeText(stage / kSlotFile, j.dump(-1, ' ', false, nlohmann::json::error_handler_t::replace))) {
        if (error.empty()) error = "the slot could not be written under " + dir.string();
        fs::remove_all(stage, ec);
        return std::nullopt;
    }
    fs::remove_all(dir / id, ec);
    fs::rename(stage, dir / id, ec);
    if (ec) {
        error = "the slot could not be put in place: " + ec.message();
        fs::remove_all(stage, ec);
        return std::nullopt;
    }
    fs::remove(staged, ec);
    return id;
}

bool remove(const fs::path& dir, const std::string& id)
{
    if (!validId(id)) return false;
    std::error_code ec;
    if (!fs::exists(dir / id, ec)) return false;
    if (enabled(dir) == id) setEnabled(dir, std::nullopt);
    fs::remove_all(dir / id, ec);
    return !ec;
}

bool setFootswitch(const fs::path& dir, const std::string& id, const std::optional<int>& cc)
{
    auto j = readSlot(dir, id);
    if (!j || (cc && (*cc < 0 || *cc > 127))) return false;
    if (cc) (*j)["footswitch"] = *cc;
    else j->erase("footswitch");
    return writeText(dir / id / kSlotFile, j->dump());
}

std::optional<json> document(const fs::path& dir, const std::string& id)
{
    const auto j = readSlot(dir, id);
    if (!j) return std::nullopt;
    auto doc = (*j)["doc"];
    // What a press bypasses is the chain's own plugins, which the compiler named (apps/k7-daw-engine/k7-runtime/docs/sessions.md).
    const auto cc = footswitchOf(*j);
    if (cc && j->contains("chainPlugins") && (*j)["chainPlugins"].is_array())
        doc["footswitch"] = {{"cc", *cc}, {"plugins", (*j)["chainPlugins"]}};
    return doc;
}

}  // namespace k7::slots
