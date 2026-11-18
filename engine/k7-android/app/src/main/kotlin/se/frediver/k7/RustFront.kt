// RustFront.kt — the Rust front (apps/k7/backend-rs) inside this process: libk7_embed.so on the studio's own
// port, the one every view loads, in front of the bundled backend on a loopback port of its own. The two
// share a secret drawn per launch (apps/k7/backend-rs/docs/embed.md).

package se.frediver.k7

import android.util.Log
import org.json.JSONObject
import java.security.SecureRandom

object RustFront {
    private const val TAG = "k7.RustFront"

    /** What the backend is told as K7_FRONT_SECRET, and the front sends with every request. */
    val secret: String by lazy { ByteArray(32).also { SecureRandom().nextBytes(it) }.joinToString("") { "%02x".format(it) } }

    /** The port it listens on, 0 while it does not run. */
    @Volatile var port = 0
        private set

    /** Starts it on `listenPort` in front of the backend on `upstreamPort`, given the backend's own settings; once per process. */
    @Synchronized
    fun start(listenPort: Int, upstreamPort: Int, env: Map<String, String>) {
        if (port > 0) return
        // An Error, not an Exception: turned into one, so the backend's start fails with a reason instead of the thread dying.
        try {
            System.loadLibrary("k7_embed")
        } catch (e: UnsatisfiedLinkError) {
            throw IllegalStateException("k7's front is missing from this build: ${e.message}")
        }
        val config = JSONObject()
            .put("listen", "127.0.0.1:$listenPort")
            .put("upstream", "127.0.0.1:$upstreamPort")
            .put("secret", secret)
            .put("env", JSONObject(env))
        val p = nativeStart(config.toString())
        check(p > 0) { "k7's front did not start on $listenPort (code $p)" }
        port = p
        Log.i(TAG, "Rust front on http://127.0.0.1:$p, in front of 127.0.0.1:$upstreamPort")
    }

    /** Stops it and waits for it; nothing when it does not run. */
    @Synchronized
    fun stop() {
        if (port == 0) return
        nativeStop()
        port = 0
    }

    @JvmStatic private external fun nativeStart(config: String): Int

    @JvmStatic private external fun nativeStop(): Int
}
