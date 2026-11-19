// StudioBridge.kt — what k7's page in AppViewsPlugin's view may ask of this app, over one WebMessageListener
// (`window.k7Android`). The page reports the colour at the top of it (assets/top-color.js), which paints the status
// bar above it, who is signed in (assets/account.js), what it plays (assets/media-session.js, to MediaPlayback), what
// keeps it busy (`busy`, to Busy), the files it saves (assets/download.js, to Downloads), and calls the engine and
// full screen: its settings panel is this phone's tray (apps/k7/frontend/src/settings/androidEngine.ts).
// The object is offered to every origin, so each message is checked for k7's own instead. See docs/android.md.

package se.frediver.k7

import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.util.Log
import android.webkit.WebView
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.webkit.JavaScriptReplyProxy
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import org.json.JSONObject

class StudioBridge(
    private val activity: AppCompatActivity,
    /** Whether an origin is k7's own. */
    private val isK7: (Uri) -> Boolean,
    private val onColor: (color: Int) -> Unit,
    /** Who k7 says is signed in, null for nobody (assets/account.js). */
    private val onAccount: (name: String?) -> Unit,
) : SupervisorHost.Listener {
    // The k7 page listening now: a reload hands over a new one with its `listen`.
    private var k7: JavaScriptReplyProxy? = null
    private var pickedCa: ((Uri?) -> Unit)? = null
    private val downloads = Downloads(activity)
    private val pickCa: ActivityResultLauncher<Intent> =
        activity.activityResultRegistry.register("k7-bridge-ca", ActivityResultContracts.StartActivityForResult()) {
            val uri = if (it.resultCode == android.app.Activity.RESULT_OK) it.data?.data else null
            pickedCa?.invoke(uri)
            pickedCa = null
        }

    init {
        SupervisorHost.listen(this)
    }

    fun destroy() {
        SupervisorHost.unlisten(this)
        pickCa.unregister()
        downloads.destroy()
    }

    fun attach(view: WebView) {
        if (!WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) return
        WebViewCompat.addWebMessageListener(view, NAME, setOf("*")) { _, message, origin, isMainFrame, reply ->
            // k7's own origin and nothing else: a page the view wandered off to gets no answer.
            if (!isMainFrame || !isK7(origin)) return@addWebMessageListener
            val msg = runCatching { JSONObject(message.data ?: "") }.getOrNull() ?: return@addWebMessageListener
            when (msg.optString("t")) {
                "color" -> runCatching { Color.parseColor(msg.getString("value")) }.onSuccess { onColor(it) }
                "account" -> onAccount(if (msg.isNull("name")) null else msg.optString("name"))
                "media" -> MediaPlayback.update(activity, msg, reply)
                "busy" -> Busy.report("k7", msg)
                "dl-start", "dl-chunk", "dl-end", "dl-fail" -> downloads.onMessage(msg, reply)
                "listen" -> {
                    k7 = reply
                    push()
                }
                "call" -> call(reply, msg.optInt("id"), msg.optString("method"), msg.optJSONObject("arg") ?: JSONObject())
            }
        }
    }

    override fun onState(frame: JSONObject) = push()

    override fun onEngineExit(code: Int) = push()

    private fun push() {
        val (state, exit) = SupervisorHost.known()
        val snapshot = JSONObject().put("t", "snapshot")
        state?.let { snapshot.put("state", it) }
        exit?.let { snapshot.put("engineExit", it) }
        activity.runOnUiThread { k7?.postMessage(snapshot.toString()) }
    }

    private fun call(reply: JavaScriptReplyProxy, id: Int, method: String, arg: JSONObject) {
        val answer = { done: Result<JSONObject> ->
            val out = JSONObject().put("t", "reply").put("id", id).put("ok", done.isSuccess)
            done.fold({ out.put("value", it) }, { out.put("error", it.message ?: "failed") })
            activity.runOnUiThread { reply.postMessage(out.toString()) }
        }
        val ok = { value: JSONObject -> answer(Result.success(value)) }
        when (method) {
            "command" -> command(arg.optString("cmd"), arg.optString("arg"), answer)
            "lanAddress" -> ok(JSONObject().apply { EngineOps.lanAddress(activity)?.let { put("address", it) } })
            "casInfo" -> ok(JSONObject().put("count", TrustedCas.count(activity)))
            "importCa" -> {
                pickedCa = { uri ->
                    if (uri == null) ok(JSONObject().put("said", "").put("count", TrustedCas.count(activity)))
                    else EngineOps.importCa(activity, uri, answer)
                }
                pickCa.launch(Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("*/*"))
            }
            "forgetCas" -> EngineOps.forgetCas(activity, answer)
            // The Engine page's Full screen switch, a setting of this device (Immersive.kt).
            "immersive" -> ok(JSONObject().put("on", Immersive.isOn(activity)))
            "setImmersive" -> {
                Immersive.set(activity, arg.optString("on") == "1")
                ok(JSONObject().put("on", Immersive.isOn(activity)))
            }
            "quit" -> {
                ok(JSONObject())
                activity.runOnUiThread { activity.finish() }
            }
            else -> answer(Result.failure(IllegalArgumentException("no method $method")))
        }
    }

    private fun command(cmd: String, arg: String, answer: (Result<JSONObject>) -> Unit) {
        val send = {
            SupervisorHost.command(cmd, arg)
            answer(Result.success(JSONObject()))
        }
        when (cmd) {
            in ALLOWED -> send()
            else -> {
                Log.w(TAG, "refused command from k7's page: $cmd")
                answer(Result.failure(SecurityException("command $cmd is not allowed")))
            }
        }
    }

    companion object {
        const val NAME = "k7Android"
        private const val TAG = "k7-bridge"

        /** The commands the tray's pages may send too (apps/k7/backend-rs/crates/k7-front/src/routes/local/routes.rs). */
        private val ALLOWED = setOf(
            "engine.start", "engine.stop", "engine.restart", "audio.priority", "lan.open",
            "plugins.rescan", "instruments.download", "instruments.remove",
        )
    }
}
