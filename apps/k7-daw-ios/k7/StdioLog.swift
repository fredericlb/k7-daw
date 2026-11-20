// StdioLog.swift — the process's stdout and stderr into the system log. The engine, the supervisor and the front
// all write there, as programs of their own would, and an iOS app's streams reach no Console a device shows.
// Lines keep their own tags (`[host]`, `[engine]`…); each is echoed to the original stderr for Xcode's console.
// An `[engine]` line is also appended to the machine log, where the Logs panel reads what a child's lines are.
// See apps/k7-daw-engine/k7-runtime/docs/ios.md.

import Foundation
import os

enum StdioLog {
    private static let log = Logger(subsystem: "se.frediver.k7.daw", category: "stdio")

    /// Once, first thing in the process: what was written before goes to the original streams only.
    static func start() {
        // The machine log opens at its first line and stays there, so its folder is named before any line (a test hook's) is written.
        setenv("STUDIO_HOME", EngineBridge.stateDir.path, 1)
        let echo = dup(STDERR_FILENO)
        for fd in [STDOUT_FILENO, STDERR_FILENO] {
            var ends: [Int32] = [0, 0]
            guard pipe(&ends) == 0 else { continue }
            dup2(ends[1], fd)
            close(ends[1])
            let from = ends[0]
            let thread = Thread {
                readLines(from) { line in
                    log.info("\(line, privacy: .public)")
                    fileEngineLine(line)
                    _ = (line + "\n").withCString { write(echo, $0, strlen($0)) }
                }
            }
            thread.name = fd == STDOUT_FILENO ? "k7-stdout" : "k7-stderr"
            thread.start()
        }
        setvbuf(stdout, nil, _IOLBF, 0)
        setvbuf(stderr, nil, _IOLBF, 0)
    }
}

private let enginePrefix = "[engine] "

/// An engine line goes to the supervisor's log as a child's line does (Log.cpp), without its tag, which the log adds.
private func fileEngineLine(_ line: String) {
    guard line.hasPrefix(enginePrefix) else { return }
    let text = String(line.dropFirst(enginePrefix.count))
    k7_supervisor_log("engine", text)
    AudioSession.engineSaid(text)
}

/// One `[host]` line in the machine log and the system log: what the app itself did, as the supervisor's own lines read.
func hostLog(_ text: String) {
    Logger(subsystem: "se.frediver.k7.daw", category: "host").info("[host] \(text, privacy: .public)")
    k7_supervisor_log("host", text)
}

/// Every complete line read from `fd` until it ends, on the calling thread; the descriptor is closed after.
func readLines(_ fd: Int32, _ each: (String) -> Void) {
    var pending = Data()
    var buffer = [UInt8](repeating: 0, count: 4096)
    while true {
        let n = read(fd, &buffer, buffer.count)
        if n < 0 && errno == EINTR { continue }
        if n <= 0 { break }
        pending.append(buffer, count: n)
        while let nl = pending.firstIndex(of: 0x0A) {
            each(String(decoding: pending[pending.startIndex..<nl], as: UTF8.self))
            pending.removeSubrange(pending.startIndex...nl)
        }
    }
    close(fd)
}
