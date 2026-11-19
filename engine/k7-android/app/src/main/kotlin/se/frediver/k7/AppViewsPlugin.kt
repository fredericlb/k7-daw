// AppViewsPlugin.kt — k7 in one full-screen WebView over the shell page (web/src/App.tsx, AppViews), under a
// status-bar strip painted the colour of k7's own header; who is signed in is k7's home page's. The page loads from
// this phone's own backend rather than from this APK: it is same-origin with its backend (`/api`, `/auth`, cookie
// sessions), so a bundled copy would be an app with no server. The view is made the first time it is shown and then
// only hidden, so k7 keeps its state, a track playing in the media library included, while the shell page is on screen.
// See docs/android.md.

package se.frediver.k7

import android.annotation.SuppressLint
import android.content.ActivityNotFoundException
import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.net.http.SslError
import android.os.Message
import android.view.View
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.PermissionRequest
import android.webkit.RenderProcessGoneDetail
import android.webkit.SslErrorHandler
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.core.graphics.ColorUtils
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import com.getcapacitor.JSObject
import com.getcapacitor.Plugin
import com.getcapacitor.PluginCall
import com.getcapacitor.PluginMethod
import com.getcapacitor.annotation.CapacitorPlugin

@CapacitorPlugin(name = "AppViews")
class AppViewsPlugin : Plugin() {
    private lateinit var frame: FrameLayout
    // Under the status bar, in the visible app's top colour.
    private lateinit var strip: View
    private lateinit var appBridge: StudioBridge
    private var color = Color.TRANSPARENT
    private var lightBarsBefore = false
    private lateinit var topColorJs: String
    private lateinit var accountJs: String
    private lateinit var mediaSessionJs: String
    private lateinit var backJs: String
    private lateinit var downloads: DownloadRoutes
    private val onBackend: (BackendHost.Status) -> Unit = { s -> bridge.executeOnMainThread { notifyListeners("backend", statusJs(s)) } }
    private var view: WebView? = null
    private var loaded = false
    private var visible = false
    private var topPx = 0
    // The navigation bar and the keyboard, measured up from the WINDOW's bottom, which the frame may already stop short of.
    private var bottomInsetPx = 0
    private var leftPx = 0
    private var rightPx = 0
    private lateinit var back: OnBackPressedCallback
    private lateinit var pickFiles: ActivityResultLauncher<Intent>
    private var chooser: ValueCallback<Array<Uri>>? = null
    private var localSignInAt = 0L
    // This plugin's number among those the process made, for the view lost before it (RendererLoss.kt).
    private var generation = 0

    override fun load() {
        generation = RendererLoss.nextGeneration()
        // The shell page's renderer is the view's: k7 is marked lost, and the activity made again.
        bridge.addWebViewListener(RendererLoss.shell(activity, bridge.webView) { why -> RendererLoss.lost(VIEW, generation, why) })
        // The shell page's ports too, from BuildConfig: its bundle's own are standalone.json's, and the e2e build has another (main.tsx).
        if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT))
            WebViewCompat.addDocumentStartJavaScript(bridge.webView, "window.$PORTS_GLOBAL='${BackendHost.PORTS_SPEC}';", setOf("https://localhost"))
        // Over the page's WebView, in the same layout, the whole screen: the shell page is only seen while k7 is not up.
        frame = FrameLayout(context).apply { visibility = View.GONE }
        (bridge.webView.parent as ViewGroup).addView(
            frame,
            ViewGroup.MarginLayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT),
        )
        strip = View(context)
        frame.addView(strip, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, 0))
        // The view starts below the status bar and the cutout, stop short of the navigation bar and the keyboard, and in landscape clear both sides.
        ViewCompat.setOnApplyWindowInsetsListener(frame) { _, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            topPx = bars.top
            leftPx = bars.left
            rightPx = bars.right
            bottomInsetPx = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.ime()).bottom
            place()
            insets
        }
        // A window resized for the keyboard moves the frame's bottom only at the next layout, so the padding is measured again then.
        frame.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> padBottom() }
        lightBarsBefore = barIcons().isAppearanceLightStatusBars
        topColorJs = context.assets.open("top-color.js").bufferedReader().use { it.readText() }
        accountJs = context.assets.open("account.js").bufferedReader().use { it.readText() }
        mediaSessionJs = context.assets.open("media-session.js").bufferedReader().use { it.readText() }
        downloads = DownloadRoutes(context.assets.open("download.js").bufferedReader().use { it.readText() }, ::isK7, ::openOutside)
        backJs = context.assets.open("back.js").bufferedReader().use { it.readText() }
        appBridge = StudioBridge(
            activity,
            ::isK7,
            onColor = {
                color = it
                if (visible) paint()
            },
            onAccount = { name -> activity.runOnUiThread { onAccount(name) } },
        )

        // Back is the page's first (back.js), then the view's history, then the system's: the task moves back on 12+, the activity finishes on 10 and 11.
        back = object : OnBackPressedCallback(false) {
            override fun handleOnBackPressed() {
                val shown = view?.takeIf { visible } ?: return passBack()
                shown.evaluateJavascript(backJs) { used ->
                    when {
                        used == "true" -> Unit
                        shown.canGoBack() -> shown.goBack()
                        else -> passBack()
                    }
                    syncBack()
                }
            }
        }
        activity.onBackPressedDispatcher.addCallback(activity, back)
        BackendHost.watch(onBackend)

        pickFiles = activity.activityResultRegistry.register("k7-app-files", ActivityResultContracts.StartActivityForResult()) {
            chooser?.onReceiveValue(WebChromeClient.FileChooserParams.parseResult(it.resultCode, it.data))
            chooser = null
        }
    }

    override fun handleOnPause() {
        // A session cookie set a moment ago must survive the process ending: this app ends it on every quit.
        CookieManager.getInstance().flush()
    }

    override fun handleOnDestroy() {
        BackendHost.unwatch(onBackend)
        appBridge.destroy()
        pickFiles.unregister()
        view?.destroy()
        view = null
    }

    @PluginMethod
    fun show(call: PluginCall) {
        // The phone's own backend is not up yet on a first run, which is still unpacking: wait rather than fail.
        BackendHost.whenReady(
            { bridge.executeOnMainThread { showNow(call) } },
            // Given up on (BackendHost.kt): the shell page's retry is what asks it to start again.
            { why -> bridge.executeOnMainThread { loadFailed("k7 could not start: $why"); call.resolve() } },
        )
    }

    /** Where the phone's own backend is: `starting` (with `step`, `progress`), `ready` or `failed` (with `message`); then `backend` events. */
    @PluginMethod
    fun backend(call: PluginCall) = call.resolve(statusJs(BackendHost.current()))

    private fun statusJs(s: BackendHost.Status): JSObject {
        val out = JSObject().put("state", s.state)
        s.step?.let { out.put("step", it) }
        s.progress?.let { out.put("progress", it) }
        s.message?.let { out.put("message", it) }
        return out
    }

    /** The shell page's Retry: BackendHost's user retry, the same a view asking again makes; the `backend` events say how it goes. */
    @PluginMethod
    fun restartBackend(call: PluginCall) {
        BackendHost.retry()
        call.resolve()
    }

    private fun showNow(call: PluginCall) {
        // Lost under the activity before this one: said once, so the page's Retry, not a reload, makes it again.
        RendererLoss.unseen(VIEW, generation)?.let { why ->
            loadFailed(why)
            return call.resolve()
        }
        val v = view ?: create().also { view = it }
        downloads.inject(v, Uri.parse(BackendHost.ORIGIN))
        // k7 opens as the local user, as the desktop window does (BackendHost.home).
        if (!loaded) v.loadUrl(BackendHost.home())
        loaded = true
        v.visibility = View.VISIBLE
        visible = true
        frame.visibility = View.VISIBLE
        // A frame that was gone may never have been given the insets.
        ViewCompat.requestApplyInsets(frame)
        place()
        paint()
        v.requestFocus()
        syncBack()
        call.resolve()
    }

    @PluginMethod
    fun hideAll(call: PluginCall) = bridge.executeOnMainThread {
        frame.visibility = View.GONE
        visible = false
        barIcons().isAppearanceLightStatusBars = lightBarsBefore
        bridge.webView.requestFocus()
        syncBack()
        call.resolve()
    }

    @PluginMethod
    fun reload(call: PluginCall) = bridge.executeOnMainThread {
        view?.reload()
        call.resolve()
    }

    /** Who k7 says is signed in; nobody means back to the local user, as on desktop. */
    private fun onAccount(name: String?) {
        if (name != null) return
        val now = System.currentTimeMillis()
        // Once in a while at most: a local sign-in that keeps failing must not reload forever.
        if (now - localSignInAt < LOCAL_SIGN_IN_GAP_MS) return
        localSignInAt = now
        view?.loadUrl(BackendHost.LOCAL_HOME)
    }

    private fun place() {
        strip.layoutParams = strip.layoutParams.apply { height = topPx }
        view?.let { v ->
            v.layoutParams = (v.layoutParams as FrameLayout.LayoutParams).apply {
                topMargin = topPx
                leftMargin = leftPx
                rightMargin = rightPx
            }
        }
        padBottom()
    }

    // Only the part of the bars and keyboard that covers the frame: a window the system already shrank for the keyboard must not lose it twice.
    private fun padBottom() {
        val at = IntArray(2)
        frame.getLocationInWindow(at)
        val belowFrame = frame.rootView.height - (at[1] + frame.height)
        val pad = maxOf(0, bottomInsetPx - belowFrame)
        if (frame.paddingBottom != pad) frame.setPadding(0, 0, 0, pad)
    }

    // Until the page has said, the shell page's own background shows through.
    private fun paint() {
        strip.setBackgroundColor(color)
        if (color != Color.TRANSPARENT) barIcons().isAppearanceLightStatusBars = ColorUtils.calculateLuminance(color) > 0.5
    }

    private fun barIcons() = WindowCompat.getInsetsController(activity.window, activity.window.decorView)

    // Taken whenever k7 is on screen, since only the page knows whether it has something open to close.
    private fun syncBack() {
        back.isEnabled = visible && view != null
    }

    /** The Back nobody here used, handed to the activity with this callback out of the way. */
    private fun passBack() {
        back.isEnabled = false
        activity.onBackPressedDispatcher.onBackPressed()
        syncBack()
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun create(): WebView = WebView(context).apply {
        // The splash's colour until the page paints, not the WebView's black between the shell and k7's splash.
        setBackgroundColor(ContextCompat.getColor(context, R.color.splash_background))
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        settings.mediaPlaybackRequiresUserGesture = false
        // target=_blank and window.open reach onCreateWindow, where the link goes outside.
        settings.setSupportMultipleWindows(true)
        settings.javaScriptCanOpenWindowsAutomatically = true
        // How the page can tell it is inside this app, should it ever need to.
        settings.userAgentString = "${settings.userAgentString} k7-android"
        webViewClient = AppClient()
        webChromeClient = AppChrome()
        setDownloadListener { url, _, disposition, mime, _ -> downloads.fromListener(this, Uri.parse(url), disposition, mime) }
        appBridge.attach(this)
        // The phone's own backend is a loopback one, whose pages derive each service's address from this table, as the desktop's preload gives it.
        if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            WebViewCompat.addDocumentStartJavaScript(this, "window.$PORTS_GLOBAL='${BackendHost.PORTS_SPEC}';", setOf(BackendHost.ORIGIN))
            // Before the page's own scripts, which may set a media session as they load; StudioBridge checks the origin.
            WebViewCompat.addDocumentStartJavaScript(this, mediaSessionJs, setOf("*"))
        }
        frame.addView(
            this,
            FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT).apply {
                topMargin = topPx
                leftMargin = leftPx
                rightMargin = rightPx
            },
        )
    }

    /** Whether `url` is k7's own origin. */
    private fun isK7(url: Uri): Boolean {
        val home = Uri.parse(BackendHost.ORIGIN)
        return home.scheme == url.scheme && home.host == url.host && home.port == url.port
    }

    /** Somewhere this app does not show: the browser, the mail app, the dialer — and only those, by scheme. */
    private fun openOutside(url: Uri) {
        // A page may name any scheme: file:, content: or intent: handed to another app would expose or launch what it should not.
        if (url.scheme?.lowercase() !in OUTSIDE_SCHEMES) return
        try {
            activity.startActivity(Intent(Intent.ACTION_VIEW, url).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        } catch (_: ActivityNotFoundException) {
            // Nothing handles it: the link just does nothing.
        } catch (e: RuntimeException) {
            // A refused URI (FileUriExposedException, SecurityException) must not take the app down with it.
            android.util.Log.w("k7-views", "not opened outside: ${e.message}")
        }
    }

    /** k7 did not load: the shell page takes over, with the tray and a retry, since no page is there to reach settings in. */
    private fun loadFailed(why: String) =
        notifyListeners("loadError", JSObject().put("message", why))

    /** The view's renderer is gone, never to be used again: made anew by the next show, after the shell page's Retry. */
    private fun drop(gone: WebView, why: String) {
        RendererLoss.lost(VIEW, generation, why)
        if (view !== gone) return
        view = null
        loaded = false
        downloads.forget()
        frame.removeView(gone)
        gone.destroy()
        syncBack()
        loadFailed(why)
    }

    private inner class AppClient : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
            val url = request.url
            if (url.scheme != "http" && url.scheme != "https") {
                openOutside(url)
                return true
            }
            return false
        }

        override fun doUpdateVisitedHistory(view: WebView, url: String?, isReload: Boolean) = syncBack()

        override fun onPageFinished(view: WebView, url: String?) {
            view.evaluateJavascript(topColorJs, null)
            view.evaluateJavascript(accountJs, null)
        }

        // Every view must claim its renderer's loss, or Android ends the process, engine and all (RendererLoss.kt).
        override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
            drop(view, RendererLoss.why(detail))
            return true
        }

        override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
            if (request.isForMainFrame) loadFailed(error.description.toString())
        }

        // Imported CAs: the WebView knows only Android's, so the ones the tray was given (TrustedCas)
        // are asked here — the same ones the supervisor trusts.
        @SuppressLint("WebViewClientOnReceivedSslError")
        override fun onReceivedSslError(view: WebView, handler: SslErrorHandler, error: SslError) {
            if (TrustedCas.vouch(context, error)) return handler.proceed()
            handler.cancel()
            // k7's own certificate, not some embed's: nothing of the app can load.
            if (isK7(Uri.parse(error.url))) loadFailed("k7's certificate is not trusted")
        }
    }

    private inner class AppChrome : WebChromeClient() {
        // The microphone and the camera, to k7's own page only, each asked on first need (PageCapture.kt).
        override fun onPermissionRequest(request: PermissionRequest) {
            val devices = PageCapture.devices(request.resources, isK7(request.origin))
            activity.runOnUiThread {
                val main = activity as? MainActivity
                if (devices == null || main == null) return@runOnUiThread request.deny()
                // Blocked for good: the page hears why its getUserMedia failed, beside the native dialog.
                PageCapture.answer(request, devices, main::access) { device ->
                    view?.evaluateJavascript("dispatchEvent(new CustomEvent('${device.event}'))", null)
                }
            }
        }

        override fun onShowFileChooser(
            view: WebView,
            callback: ValueCallback<Array<Uri>>,
            params: FileChooserParams,
        ): Boolean {
            chooser?.onReceiveValue(null)
            chooser = callback
            try {
                pickFiles.launch(params.createIntent())
            } catch (_: ActivityNotFoundException) {
                chooser = null
                return false
            }
            return true
        }

        // A new window is never opened: a throwaway view learns where it was headed, and that goes outside (WindowProbe.kt).
        override fun onCreateWindow(view: WebView, isDialog: Boolean, isUserGesture: Boolean, resultMsg: Message): Boolean {
            WindowProbe.answer(context, resultMsg) { openOutside(it) }
            return true
        }
    }

    companion object {
        // The one view's key where the helpers keep state per view (RendererLoss, Busy).
        private const val VIEW = "k7"
        private const val LOCAL_SIGN_IN_GAP_MS = 5_000L
        // web-protocol's PORTS_GLOBAL, the name every page reads the loopback ports from.
        private const val PORTS_GLOBAL = "__STUDIO_PORTS__"
        // What openOutside hands to another app; anything else is dropped.
        private val OUTSIDE_SCHEMES = setOf("http", "https", "mailto", "tel")
    }
}
