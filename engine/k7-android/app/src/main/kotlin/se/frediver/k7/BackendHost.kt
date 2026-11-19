// BackendHost.kt — k7's backend on the phone: the Rust front (apps/k7/backend-rs) in this process, answering the
// studio's own port by itself. Port, environment and folders are the desktop's, from
// apps/k7/backend-rs/standalone.json. The page, the library it seeds and the front's code root ride in one zip the
// app unpacks. One front per PROCESS, like the supervisor: a start that failed is the one thing a retry redoes. See docs/android.md.

package se.frediver.k7

import android.content.Context
import android.util.Log
import org.json.JSONObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.util.zip.ZipInputStream

/** One per PROCESS, like the supervisor: the front outlives a recreated activity. */
object BackendHost {
    /** standalone.json's port, the desktop app's. */
    const val PORT = BuildConfig.STANDALONE_PORT
    const val ORIGIN = "http://127.0.0.1:$PORT"
    /** What every process that derives a loopback address is told — the backend, the supervisor, the pages. */
    const val PORTS_SPEC = "k7=$PORT"
    /** k7's first page: the page asks `/auth/me` for its token itself (apps/k7/backend-rs/docs/sessions.md). */
    const val HOME = "$ORIGIN/"

    private var app: Context? = null
    private var dirs: Dirs? = null
    private var started = false
    private var stopping = false
    private var ready = false
    // Why the front is given up on, until a view asks again; null while it runs or is being started.
    private var failure: String? = null
    private val waiting = mutableListOf<Waiter>()
    private var status = Status("starting")
    private val watchers = mutableListOf<(Status) -> Unit>()

    private class Waiter(val then: () -> Unit, val failed: (String) -> Unit)

    /** What the shell page shows: `starting` with a step (`unpacking` with the fraction done, `launching`), `ready`, or `failed` with why. */
    data class Status(val state: String, val step: String? = null, val progress: Double? = null, val message: String? = null)

    fun current(): Status = synchronized(this) { status }

    /** Every status change from now on, called on whichever thread made it. */
    fun watch(watcher: (Status) -> Unit) {
        synchronized(this) { watchers.add(watcher) }
    }

    fun unwatch(watcher: (Status) -> Unit) {
        synchronized(this) { watchers.remove(watcher) }
    }

    private fun report(next: Status) {
        val to = synchronized(this) {
            status = next
            watchers.toList()
        }
        to.forEach { it(next) }
    }

    /** Unpack, lay out and start, once per process: a first run unpacks for a few seconds. */
    @Synchronized
    fun start(context: Context) {
        if (started) return
        started = true
        app = context.applicationContext
        attempt()
    }

    /** `then` once the front has answered — at once if it has — or `failed` once it is given up on; either on any thread. */
    fun whenReady(then: () -> Unit, failed: (String) -> Unit) {
        val now = synchronized(this) { ready.also { if (!it) waiting.add(Waiter(then, failed)) } }
        // A view asking after the start was given up on is the user's retry.
        if (now) then() else retry()
    }

    /** The user's retry — a view asking again, or the shell page's Retry: a start given up on is made again. */
    fun retry() {
        val again = synchronized(this) {
            (failure != null && started).also {
                if (it) failure = null
            }
        }
        if (again) attempt()
    }

    /** The front stopped, which stops the supervisor it runs; nothing when it does not run. */
    fun stop(timeoutMs: Long) {
        synchronized(this) { stopping = true }
        val done = java.util.concurrent.CountDownLatch(1)
        Thread({ RustFront.stop(); done.countDown() }, "k7-front-stop").start()
        done.await(timeoutMs, java.util.concurrent.TimeUnit.MILLISECONDS)
    }

    /** Start the front unless it runs, then wait for its answer. */
    private fun attempt() {
        val context = synchronized(this) { app } ?: return
        Thread({
            try {
                report(Status("starting"))
                val d = synchronized(this) { dirs } ?: prepare(context).also { synchronized(this) { dirs = it } }
                report(Status("starting", "launching"))
                if (synchronized(this) { stopping }) return@Thread
                RustFront.start(PORT, sharedEnv(context, d))
                waitUp()
                Log.i(TAG, "front up on $ORIGIN")
                report(Status("ready"))
                val run = synchronized(this) {
                    ready = true
                    waiting.toList().also { waiting.clear() }
                }
                run.forEach { it.then() }
            } catch (e: Exception) {
                Log.e(TAG, "front did not start: ${e.message}", e)
                fail(e.message ?: "k7's front did not start")
            }
        }, "k7-backend-start").start()
    }

    /** Whether `path` on `origin` answers 200 within `timeoutMs`. */
    private fun answers(path: String, timeoutMs: Int, origin: String = ORIGIN): Boolean = try {
        val c = URL("$origin$path").openConnection() as HttpURLConnection
        c.connectTimeout = timeoutMs
        c.readTimeout = timeoutMs
        try {
            c.responseCode == 200
        } finally {
            c.disconnect()
        }
    } catch (_: java.io.IOException) {
        false
    }

    /** Given up on: every view waiting is told why, and the next one to ask starts it again. */
    private fun fail(why: String) {
        val told = synchronized(this) {
            if (stopping) return
            failure = why
            ready = false
            waiting.toList().also { waiting.clear() }
        }
        report(Status("failed", message = why))
        told.forEach { it.failed(why) }
    }

    /** Where the backend keeps everything it writes, beside the engine's `.k7`. */
    private fun rootDir(context: Context) = File(context.filesDir, "standalone")

    /** The backend's workspaces and médiathèque, which the engine must be allowed to open (engine/k7-runtime/docs/android.md). */
    fun engineRoots(context: Context): List<File> =
        BuildConfig.ENGINE_ROOT_DIRS.split(';').filter { it.isNotEmpty() }.map { File(rootDir(context), it) }

    private class Dirs(val root: File, val bundle: File, val table: JSONObject)

    private fun prepare(context: Context): Dirs {
        val root = rootDir(context)
        val bundle = unpack(context, File(root, "backend"))
        val table = JSONObject(File(bundle, TABLE).readText())
        return Dirs(root, bundle, table)
    }

    /** The bundle, out of the APK's assets and onto disk, where the front reads it; per install, writing only the files that changed. */
    private fun unpack(context: Context, dir: File): File {
        val marker = File(dir, ".installed")
        val stamp = context.packageManager.getPackageInfo(context.packageName, 0).lastUpdateTime.toString()
        if (marker.isFile && marker.readText() == stamp) return dir
        // First, so an unpack cut short by a kill is redone on the next start rather than trusted.
        marker.delete()
        dir.mkdirs()
        val manifest = File(dir, ENTRIES)
        val before = runCatching { manifest.readLines() }.getOrDefault(emptyList()).mapNotNull { line ->
            line.split('\t').takeIf { it.size == 2 }?.let { it[0] to it[1] }
        }.toMap()
        val after = mutableMapOf<String, String>()
        val names = mutableSetOf<String>()
        var count = 0
        // Stored uncompressed in the APK (noCompress), so its length is known and the bytes read are the progress.
        val total = context.assets.openFd(BUNDLE_ASSET).use { it.length }.coerceAtLeast(1)
        ZipInputStream(Counting(context.assets.open(BUNDLE_ASSET), total).buffered()).use { zip ->
            generateSequence { zip.nextEntry }.forEach { entry ->
                val out = File(dir, entry.name)
                require(out.canonicalPath.startsWith(dir.canonicalPath + File.separator)) { "bad entry ${entry.name}" }
                names.add(entry.name.trimEnd('/'))
                if (entry.isDirectory) out.mkdirs()
                else {
                    // The local header's CRC and size, which Gradle's Zip writes; an entry without them is always written.
                    val key = if (entry.crc != -1L && entry.size >= 0) "${entry.crc}:${entry.size}" else null
                    key?.let { after[entry.name] = it }
                    if (key == null || before[entry.name] != key || out.length() != entry.size) {
                        out.parentFile?.mkdirs()
                        out.outputStream().use(zip::copyTo)
                        count++
                    }
                }
            }
        }
        // What the old bundle had and this one does not.
        dir.walkBottomUp().filter { it != dir }.forEach { f ->
            val name = f.relativeTo(dir).invariantSeparatorsPath
            if (name !in names && name !in OWN_FILES && (f.isFile || f.list()?.isEmpty() == true)) f.delete()
        }
        manifest.writeText(after.entries.joinToString("") { (name, key) -> "$name\t$key\n" })
        marker.writeText(stamp)
        Log.i(TAG, "unpacked the bundle: $count files written of ${after.size}")
        return dir
    }

    /** The asset as it is read, reporting each whole percent of it to the watchers. */
    private class Counting(inner: java.io.InputStream, private val total: Long) : java.io.FilterInputStream(inner) {
        private var done = 0L
        private var shown = -1

        private fun add(n: Int) {
            if (n <= 0) return
            done += n
            val pct = (done * 100 / total).toInt()
            if (pct == shown) return
            shown = pct
            report(Status("starting", "unpacking", pct / 100.0))
        }

        override fun read(): Int = super.read().also { if (it >= 0) add(1) }

        override fun read(b: ByteArray, off: Int, len: Int): Int = super.read(b, off, len).also(::add)
    }

    /** What the front is told: the table's environment, its folders and the switches. */
    private fun sharedEnv(context: Context, dirs: Dirs): Map<String, String> {
        val out = mutableMapOf<String, String>()
        val env = dirs.table.getJSONObject("env")
        env.keys().forEach { out[it] = env.getString(it) }
        val folders = dirs.table.getJSONObject("dirs")
        folders.keys().forEach { out[it] = File(dirs.root, folders.getString(it)).absolutePath }
        // The registry, the relay and the tunnel hub in the front; the supervisor stays SupervisorHost's (docs/android.md).
        out["K7_ENGINE_CORE"] = "front"
        // The compiled modules in the app's own files (docs/android.md).
        out["K7_FRONT_CACHE"] = File(dirs.root, "front-cache").absolutePath
        // The file hub too: every account's file service is the front's own (docs/android.md).
        out["K7_FILES"] = "front"
        // The hub copies `k7` from the bundle the first time it has no folder for it.
        out["STUDIO_LIBRARY_SEED"] = "k7-core=" + File(dirs.bundle, dirs.table.getString("library")).absolutePath
        // The front's code root, unpacked from backend.zip: what its actor reads (apps/k7/backend-rs/docs/service.md).
        out["K7_CODE_ROOT"] = File(dirs.bundle, "code").absolutePath
        // The page, built into the bundle at its repo path (apps/k7/backend-rs/scripts/stageBundle.mjs).
        out["K7_FRONTEND_DIST"] = File(dirs.bundle, "apps/k7/frontend/dist").absolutePath
        // The user's own CAs for the front's outbound calls (a paired server, the hunt) (docs/android.md).
        TrustedCas.file(context).takeIf { it.isFile }?.let { out["K7_EXTRA_CA_CERTS"] = it.absolutePath }
        return out
    }

    /** Until the front answers its health on the studio's port; the library gate is the front's own (docs/android.md). */
    private fun waitUp() {
        val end = System.currentTimeMillis() + READY_MS
        while (System.currentTimeMillis() < end) {
            if (answers("/api/health", 500)) return
            Thread.sleep(RETRY_MS)
        }
        throw IllegalStateException("no answer on $ORIGIN within $READY_MS ms")
    }

    private const val TAG = "k7-backend"
    private const val RETRY_MS = 200L
    private const val BUNDLE_ASSET = "backend.zip"
    private const val TABLE = "standalone.json"
    // Each unpacked file's CRC and size, what the next update compares against.
    private const val ENTRIES = ".entries"
    // Written beside the zip's files, so never stale.
    private val OWN_FILES = setOf(".installed", ENTRIES)
    private const val READY_MS = 60_000L
}
