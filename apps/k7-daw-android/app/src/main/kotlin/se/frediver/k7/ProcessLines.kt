// ProcessLines.kt — the threads that read a child program's output line by line: node's, the supervisor's.
// A stop closes the pipe under a blocked read, which Android throws as InterruptedIOException: that is the
// reader's end, never a crash of the app (docs/android.md).

package se.frediver.k7

import java.io.IOException
import java.io.InputStream

object ProcessLines {
    /** Reads `stream` on a thread named `name` until it ends or is closed, `onLine` per line; `log` gets the one line of a closed end. */
    fun pump(stream: InputStream, name: String, log: (String) -> Unit, onLine: (String) -> Unit): Thread =
        Thread({ drain(stream, name, log, onLine) }, name).apply { start() }

    fun drain(stream: InputStream, name: String, log: (String) -> Unit, onLine: (String) -> Unit) {
        try {
            stream.bufferedReader().forEachLine(onLine)
        } catch (e: IOException) {
            log("$name ended with its process: ${e.message}")
        }
    }

    /** Once the process has exited: `graceMs` for the reader to drain, then its stream is closed, so a pipe a grandchild still holds cannot keep it. */
    fun finish(reader: Thread, stream: InputStream, graceMs: Long) {
        reader.join(graceMs)
        if (reader.isAlive) runCatching { stream.close() }
    }
}
