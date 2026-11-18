// ChildKill.kt — a real SIGKILL for a program the app started. Android's Process.destroyForcibly only
// sends SIGTERM, which a wedged or stopped node never acts on, and its Process has no pid(): the child is
// found in /proc, by its parent and its command line (docs/android.md).

package se.frediver.k7

import android.system.Os
import android.system.OsConstants
import android.util.Log
import java.io.File

object ChildKill {
    /** SIGTERM and closed pipes through `p.destroy()`, then SIGKILL to every child of this app whose command line holds `marker`. */
    fun kill(p: Process, marker: String) {
        p.destroy()
        val me = Os.getpid()
        File("/proc").listFiles()?.forEach { dir ->
            val pid = dir.name.toIntOrNull() ?: return@forEach
            val stat = runCatching { File(dir, "stat").readText() }.getOrNull() ?: return@forEach
            if (parentOf(stat) != me) return@forEach
            val cmdline = runCatching { File(dir, "cmdline").readText() }.getOrDefault("")
            if (marker !in cmdline) return@forEach
            runCatching { Os.kill(pid, OsConstants.SIGKILL) }.onFailure { Log.w(TAG, "SIGKILL to $pid refused: ${it.message}") }
        }
    }

    /** The parent pid in a `/proc/<pid>/stat` line, whose second field, the name in parentheses, may hold spaces and parentheses. */
    fun parentOf(stat: String): Int? = stat.substringAfterLast(") ", "").split(' ').getOrNull(1)?.toIntOrNull()

    private const val TAG = "k7-kill"
}
