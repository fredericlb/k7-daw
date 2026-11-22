// ForgeReturn.kt — a Forgejo sign-in coming back to k7: the browser follows the server to `rocks.k7.daw://callback`,
// which brings MainActivity to the front, and its query is forwarded to the front's own callback; the page's poll says the end.
// What a reader gets wrong: the browser never loads the loopback page here; see docs/android.md.

package se.frediver.k7

import android.content.Intent
import android.util.Log
import java.net.HttpURLConnection
import java.net.URI
import java.net.URL
import kotlin.concurrent.thread

object ForgeReturn {
    private const val TAG = "k7-forge"
    const val SCHEME = "rocks.k7.daw"
    /** The front's route the server's answer is forwarded to (apps/k7-daw-backend/docs/files/git.md). */
    const val CALLBACK_PATH = "/api/files/git/forge/callback"

    /** The front's callback for `data`; null for anything but `rocks.k7.daw://callback?…`. */
    fun forward(data: String?, origin: String): String? {
        val uri = data?.let { runCatching { URI(it) }.getOrNull() } ?: return null
        if (uri.scheme?.lowercase() != SCHEME || uri.host != "callback") return null
        val query = uri.rawQuery?.takeIf { it.isNotEmpty() } ?: return null
        return "$origin$CALLBACK_PATH?$query"
    }

    /** Whether `intent` was the sign-in coming back, forwarded then on a thread of its own. */
    fun take(intent: Intent?): Boolean {
        val target = forward(intent?.dataString, BackendHost.ORIGIN) ?: return false
        thread(name = "k7-forge-return") {
            try {
                val c = URL(target).openConnection() as HttpURLConnection
                c.connectTimeout = 5_000
                c.readTimeout = 30_000
                Log.i(TAG, "forwarded, front answered ${c.responseCode}")
                c.disconnect()
            } catch (e: java.io.IOException) {
                Log.w(TAG, "not forwarded: ${e.message}")
            }
        }
        return true
    }
}
