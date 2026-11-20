// EngineBridge.kt — the engine in this process: load it, initialise JUCE, and serve on a thread whose
// Looper becomes JUCE's message loop. The SUPERVISOR decides when, and with which identity, exactly as
// it spawns the engine on desktop. Once per PROCESS: JUCE stays bound to that Looper, and a quit Looper
// cannot loop again, so a stopped engine is restarted by a new process (docs/android.md).

package se.frediver.k7

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Looper
import android.system.Os
import android.util.Log
import com.rmsl.juce.Java
import java.io.File

object EngineBridge {
    private var thread: Thread? = null

    /** Whether the engine had RECORD_AUDIO when it opened its device; null before it started. Its inputs are fixed then. */
    @Volatile var startedWithMic: Boolean? = null
        private set

    @JvmStatic private external fun nativeServe(port: Int): Int
    @JvmStatic private external fun nativeQuit()

    /** Serve with the supervisor's identity, once; `onExit` gets serve()'s exit code, on the engine's thread. */
    @Synchronized
    fun start(context: Context, id: String, token: String, port: Int, host: String, origins: String, onExit: (Int) -> Unit) {
        if (thread != null) {
            Log.w(TAG, "engine already started in this process — a restart needs a new process")
            return
        }
        // What the supervisor's spawn passes on desktop, read by serve() when it starts.
        Os.setenv("STUDIO_HOME", stateDir(context).absolutePath, true)
        // The backend sends its own paths, outside `.k7`: without these the engine drops every clip and sample.
        Os.setenv("STUDIO_ENGINE_ROOTS", BackendHost.engineRoots(context).joinToString(":") { it.absolutePath }, true)
        Os.setenv("STUDIO_ENGINE_HOST", host, true)
        Os.setenv("STUDIO_ENGINE_ID", id, true)
        Os.setenv("STUDIO_ENGINE_TOKEN", token, true)
        Os.setenv("STUDIO_ENGINE_ALLOWED_ORIGINS", origins, true)
        // An app process has no HOME, and Surge XT's storage throws without one: it writes nothing there (docs/surgePlugin.md).
        Os.setenv("HOME", context.filesDir.absolutePath, false)
        val mic = context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED
        startedWithMic = mic
        // Without it the engine opens no inputs: asked for anyway, they fail the output with them (docs/android.md).
        Os.setenv("STUDIO_ENGINE_INPUTS", if (mic) "1" else "0", true)
        // Loads libk7runtime (Java's static block), whose JNI_OnLoad registered this very method.
        Java.initialiseJUCE(context.applicationContext)
        thread = Thread({
            Looper.prepare()
            onExit(nativeServe(port))
        }, "k7-runtime").apply { start() }
    }

    /** Whether the engine served in this process and stopped: only a new process starts it again (Relaunch.kt). */
    @Synchronized
    fun ended(): Boolean = thread?.isAlive == false

    /** Ask the engine to stop and wait for it, at most `timeoutMs`; true when it did. */
    fun stop(timeoutMs: Long): Boolean {
        val t = synchronized(this) { thread } ?: return true
        nativeQuit()
        t.join(timeoutMs)
        return !t.isAlive
    }

    /** `~/.k7` on a phone: the app's own files, shared by the engine and the supervisor. */
    fun stateDir(context: Context) = File(context.filesDir, ".k7")

    private const val TAG = "k7-runtime"
}
