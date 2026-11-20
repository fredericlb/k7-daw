// ExportFolders.kt — where the page's exports go on this phone: a folder the person picks with the system's
// document-tree picker (ACTION_OPEN_DOCUMENT_TREE), kept as a persisted grant, then each file created in it through
// DocumentsContract. The page sends the bytes in chunks over StudioBridge; the front never sees the folder.
// The thing a reader would get wrong: the folder is a content URI, not a path, so only this app can write it.
// See docs/android.md and apps/k7/frontend/docs/export.md.

package se.frediver.k7

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.util.Base64
import android.webkit.MimeTypeMap
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import org.json.JSONObject
import java.io.OutputStream
import java.util.UUID
import java.util.concurrent.Executors

class ExportFolders(private val activity: AppCompatActivity) {
    private class Open(val uri: Uri, val out: OutputStream)

    // One thread, so the chunks of a file are written in the order they came.
    private val io = Executors.newSingleThreadExecutor { Thread(it, "k7-export") }
    private val open = mutableMapOf<String, Open>()
    private val prefs = activity.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    private var picked: ((Uri?) -> Unit)? = null
    private val picker: ActivityResultLauncher<Intent> =
        activity.activityResultRegistry.register("k7-export-folder", ActivityResultContracts.StartActivityForResult()) {
            val uri = if (it.resultCode == Activity.RESULT_OK) it.data?.data else null
            if (uri != null) {
                val both = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                activity.contentResolver.takePersistableUriPermission(uri, both)
                prefs.edit().putString(LAST, uri.toString()).apply()
            }
            picked?.invoke(uri)
            picked = null
        }

    fun destroy() {
        picker.unregister()
        io.execute { open.keys.toList().forEach { abort(it) } }
        io.shutdown()
    }

    /** The picker, opened on the last folder; `{id, name}` or `{}` when cancelled. */
    fun pick(answer: (Result<JSONObject>) -> Unit) {
        picked = { uri ->
            answer(Result.success(if (uri == null) JSONObject() else JSONObject().put("id", uri.toString()).put("name", folderName(uri))))
        }
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).addFlags(
            Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION,
        )
        prefs.getString(LAST, null)?.let { intent.putExtra(DocumentsContract.EXTRA_INITIAL_URI, Uri.parse(it)) }
        activity.runOnUiThread { picker.launch(intent) }
    }

    /** A file created in the folder; the answer carries the token the chunks name. */
    fun begin(folder: String, name: String, answer: (Result<JSONObject>) -> Unit) = run(answer) {
        val tree = Uri.parse(folder)
        // The grant must still be held: a revoked folder is picked again, not written blind.
        check(activity.contentResolver.persistedUriPermissions.any { it.uri == tree && it.isWritePermission }) { "the folder is no longer granted: choose it again" }
        val parent = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        val made = DocumentsContract.createDocument(activity.contentResolver, parent, mimeOf(name), plain(name)) ?: error("cannot create $name")
        val out = activity.contentResolver.openOutputStream(made) ?: error("cannot write $name")
        val token = UUID.randomUUID().toString()
        open[token] = Open(made, out.buffered())
        JSONObject().put("token", token)
    }

    fun chunk(token: String, data: String, answer: (Result<JSONObject>) -> Unit) = run(answer) {
        (open[token] ?: error("no export $token")).out.write(Base64.decode(data, Base64.DEFAULT))
        JSONObject()
    }

    /** Closes the file, or removes it when `cancel`; answers the name it really has (a provider numbers a clash) and its URI. */
    fun end(token: String, cancel: Boolean, answer: (Result<JSONObject>) -> Unit) = run(answer) {
        if (cancel) {
            abort(token)
            return@run JSONObject()
        }
        val file = open.remove(token) ?: error("no export $token")
        file.out.close()
        JSONObject().put("name", displayName(file.uri) ?: "file").put("where", file.uri.toString())
    }

    /** The system share sheet, offered the saved file. */
    fun share(where: String, name: String, answer: (Result<JSONObject>) -> Unit) = run(answer) {
        val uri = Uri.parse(where)
        val send = Intent(Intent.ACTION_SEND).setType(mimeOf(name)).putExtra(Intent.EXTRA_STREAM, uri).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        send.clipData = android.content.ClipData.newRawUri(name, uri)
        activity.runOnUiThread { activity.startActivity(Intent.createChooser(send, name).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)) }
        JSONObject()
    }

    private fun run(answer: (Result<JSONObject>) -> Unit, work: () -> JSONObject) {
        if (io.isShutdown) return
        io.execute { answer(runCatching(work)) }
    }

    private fun abort(token: String) {
        val file = open.remove(token) ?: return
        runCatching { file.out.close() }
        runCatching { DocumentsContract.deleteDocument(activity.contentResolver, file.uri) }
    }

    private fun displayName(uri: Uri): String? =
        activity.contentResolver.query(uri, arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0) else null
        }

    private fun folderName(tree: Uri): String =
        displayName(DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))) ?: "folder"

    companion object {
        private const val PREFS = "k7-export"
        private const val LAST = "last-folder"

        /** The page's name, made a plain file name. */
        fun plain(given: String): String =
            given.substringAfterLast('/').substringAfterLast('\\').filter { it >= ' ' && it !in "<>:\"|?*" }.trim().trimStart('.').ifEmpty { "export" }.take(200)

        fun mimeOf(name: String): String =
            MimeTypeMap.getSingleton().getMimeTypeFromExtension(name.substringAfterLast('.', "").lowercase()) ?: "application/octet-stream"
    }
}
