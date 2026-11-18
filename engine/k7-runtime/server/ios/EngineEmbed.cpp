// EngineEmbed.cpp — see EngineEmbed.h. The engine's iostreams are tagged `[engine]` here, since the process's stderr
// is shared with the front and carries no other mark; the app files each tagged line in the machine log (docs/ios.md).

#include "EngineEmbed.h"

#include "../Serve.h"

#include <unistd.h>

#include <iostream>
#include <mutex>
#include <streambuf>
#include <string>

namespace {

// Whole lines as ONE write(2) each, so threads never interleave inside a line.
class TaggedBuf : public std::streambuf {
public:
    explicit TaggedBuf(int fd) : fd_(fd) {}

protected:
    int_type overflow(int_type ch) override {
        if (ch == traits_type::eof()) return traits_type::not_eof(ch);
        std::lock_guard lock(mutex_);
        line_.push_back(static_cast<char>(ch));
        if (ch == '\n') flushLine();
        return ch;
    }

    std::streamsize xsputn(const char* s, std::streamsize n) override {
        std::lock_guard lock(mutex_);
        for (std::streamsize i = 0; i < n; ++i) {
            line_.push_back(s[i]);
            if (s[i] == '\n') flushLine();
        }
        return n;
    }

private:
    void flushLine() {
        const std::string out = "[engine] " + line_;
        line_.clear();
        const char* p = out.data();
        size_t left = out.size();
        while (left > 0) {
            const ssize_t n = ::write(fd_, p, left);
            if (n <= 0) break;
            p += n;
            left -= static_cast<size_t>(n);
        }
    }

    int fd_;
    std::mutex mutex_;
    std::string line_;
};

void tagStreams() {
    static TaggedBuf err(STDERR_FILENO);
    static TaggedBuf out(STDOUT_FILENO);
    std::cerr.rdbuf(&err);
    std::cout.rdbuf(&out);
}

}  // namespace

extern "C" int k7_engine_serve(int port) {
    tagStreams();
    return studio::serve(port);
}

extern "C" void k7_engine_quit(void) {
    studio::requestQuit();
}

extern "C" void k7_engine_suspend_audio(int suspended) {
    studio::suspendAudio(suspended != 0);
}
