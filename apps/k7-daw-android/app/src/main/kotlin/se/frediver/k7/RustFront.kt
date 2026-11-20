// RustFront.kt — the Rust front (apps/k7/backend-rs) inside this process: libk7_embed.so on the studio's own
// port, the one every view loads, with its seams' secret drawn per launch (apps/k7/backend-rs/docs/embed.md).

package se.frediver.k7

import android.util.Log
import org.json.JSONObject
import java.security.SecureRandom

object RustFront {
    private const val TAG = "k7.RustFront"

    /** The secret of the front's own seams, drawn per launch. */
    val secret: String by lazy { ByteArray(32).also { SecureRandom().nextBytes(it) }.joinToString("") { "%02x".format(it) } }

    /** The port it listens on, 0 while it does not run. */
    @Volatile var port = 0
        private set

    /** Starts it on `listenPort`, given its settings; once per process. */
    @Synchronized
    fun start(listenPort: Int, env: Map<String, String>) {
        if (port > 0) return
        // An Error, not an Exception: turned into one, so the start fails with a reason instead of the thread dying.
        try {
            System.loadLibrary("k7_embed")
        } catch (e: UnsatisfiedLinkError) {
            throw IllegalStateException("k7's front is missing from this build: ${e.message}")
        }
        val config = JSONObject()
            .put("listen", "127.0.0.1:$listenPort")
            .put("secret", secret)
            .put("env", JSONObject(env))
        val p = nativeStart(config.toString())
        check(p > 0) { "k7's front did not start on $listenPort (code $p)" }
        port = p
        Log.i(TAG, "Rust front on http://127.0.0.1:$p")
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
