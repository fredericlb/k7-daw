// EngineJni.cpp — the Android app's doors into the engine: its environment, serve on a port, and stop serving.
// nativeServe BLOCKS, and must be called on a thread that has PREPARED a Looper and loops nothing yet —
// not a HandlerThread, which already is inside loop() — because the first JUCE call on it makes it JUCE's
// message thread, and the host's run loops that Looper. Java.initialiseJUCE must have run first.
// See docs/android.md.

#include "../../engine/capi/k7_server.h"

#include <android/log.h>
#include <jni.h>
#include <unistd.h>

#include <cstdio>
#include <cstdlib>
#include <string>
#include <thread>

namespace {

// Everything the engine says goes to stderr, which Android discards: pipe it into logcat, a line per entry.
void stderrToLogcat() {
    static bool started = false;
    if (started) return;
    int fds[2];
    if (pipe(fds) != 0) return;
    started = true;
    setvbuf(stderr, nullptr, _IOLBF, 0);
    dup2(fds[1], STDERR_FILENO);
    close(fds[1]);
    std::thread([fd = fds[0]] {
        std::string line;
        char buf[512];
        for (ssize_t n; (n = read(fd, buf, sizeof buf)) > 0;) {
            for (ssize_t i = 0; i < n; ++i) {
                if (buf[i] != '\n') { line += buf[i]; continue; }
                __android_log_write(ANDROID_LOG_INFO, "k7-runtime", line.c_str());
                line.clear();
            }
        }
    }).detach();
}

}  // namespace

// Os.setenv writes the process's C library; under a native bridge this code reads a copy of its own, so the app sets each variable here too.
extern "C" JNIEXPORT void JNICALL
Java_se_frediver_k7_EngineBridge_nativeSetenv(JNIEnv* env, jclass, jstring key, jstring value, jboolean overwrite) {
    const char* k = env->GetStringUTFChars(key, nullptr);
    const char* v = env->GetStringUTFChars(value, nullptr);
    if (k && v) setenv(k, v, overwrite ? 1 : 0);
    if (v) env->ReleaseStringUTFChars(value, v);
    if (k) env->ReleaseStringUTFChars(key, k);
}

extern "C" JNIEXPORT jint JNICALL
Java_se_frediver_k7_EngineBridge_nativeServe(JNIEnv*, jclass, jint port) {
    stderrToLogcat();
    return k7_server_serve(static_cast<int>(port));
}

extern "C" JNIEXPORT void JNICALL
Java_se_frediver_k7_EngineBridge_nativeQuit(JNIEnv*, jclass) {
    k7_server_request_quit();
}
