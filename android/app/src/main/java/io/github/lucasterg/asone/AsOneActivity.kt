package io.github.lucasterg.asone

import android.content.ClipboardManager
import android.content.res.Configuration
import android.graphics.Color
import android.content.ContentUris
import android.content.ContentValues
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.DocumentsContract
import android.provider.MediaStore
import android.provider.OpenableColumns
import android.text.InputType
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.widget.EditText
import android.widget.FrameLayout
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AlertDialog
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import com.google.androidgamesdk.GameActivity
import java.io.File

/**
 * Powłoka AsOne na Androida. Interfejs rysuje Rust (egui + wgpu w libasone_app.so);
 * Kotlin dostarcza to, czego natywna aktywność nie ma: marginesy pasków systemowych,
 * okno wyboru zdjęć, schowek, klawiaturę (okno edycji tekstu) i zapis do Galerii.
 *
 * Metody publiczne wywołuje Rust przez JNI (`apps/asone/src/android.rs`) – z dowolnego wątku.
 */
class AsOneActivity : GameActivity() {

    private var lastInsets = intArrayOf(-1, -1, -1, -1)

    private val openImages =
        registerForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
            // Odczyt poza wątkiem UI – zdjęcia mogą mieć po kilkanaście MB.
            Thread { readPicked(uris) }.start()
        }

    private val openFolder =
        registerForActivityResult(ActivityResultContracts.OpenDocumentTree()) { tree ->
            // Odczyt poza wątkiem UI – w folderze może być wiele dużych zdjęć.
            if (tree != null) Thread { readFolder(tree) }.start()
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        // Rysujemy na całym ekranie; Rust odsuwa interfejs o marginesy z nativeOnInsets.
        WindowCompat.setDecorFitsSystemWindows(window, false)
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.VANILLA_ICE_CREAM) {
            // Przed Androidem 15 paski mają własne tło – przezroczyste, pod spodem kolor belki z Rusta.
            @Suppress("DEPRECATION")
            window.statusBarColor = Color.TRANSPARENT
            @Suppress("DEPRECATION")
            window.navigationBarColor = Color.TRANSPARENT
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            val attrs = window.attributes
            attrs.layoutInDisplayCutoutMode = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
            } else {
                WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            }
            window.attributes = attrs
        }
        super.onCreate(savedInstanceState)
        installInsetsListener()
    }

    private fun installInsetsListener() {
        val host: View = window.decorView.findViewById<ViewGroup>(android.R.id.content) ?: window.decorView
        ViewCompat.setOnApplyWindowInsetsListener(host) { view, insets ->
            sendInsets(insets)
            ViewCompat.onApplyWindowInsets(view, insets)
        }
        ViewCompat.requestApplyInsets(host)
        // Na wypadek, gdyby pierwsze rozesłanie marginesów minęło nasz widok – po ułożeniu okna.
        host.post { pushRootInsets() }
    }

    // Marginesy zmieniają się przy obrocie, trybie wielu okien, zmianie wyświetlacza – odświeżamy.
    override fun onResume() {
        super.onResume()
        window.decorView.post { pushRootInsets() }
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        window.decorView.post { pushRootInsets() }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) window.decorView.post { pushRootInsets() }
    }

    override fun onMultiWindowModeChanged(isInMultiWindowMode: Boolean, newConfig: Configuration) {
        super.onMultiWindowModeChanged(isInMultiWindowMode, newConfig)
        window.decorView.post { pushRootInsets() }
    }

    private fun pushRootInsets() {
        ViewCompat.getRootWindowInsets(window.decorView)?.let { sendInsets(it) }
    }

    /** Paski systemowe + wycięcie aparatu → Rust (tylko gdy się zmieniły). */
    private fun sendInsets(insets: WindowInsetsCompat) {
        val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
        val now = intArrayOf(bars.left, bars.top, bars.right, bars.bottom)
        if (!now.contentEquals(lastInsets)) {
            lastInsets = now
            nativeOnInsets(bars.left, bars.top, bars.right, bars.bottom)
        }
    }

    // ---- Wywoływane z Rusta ----------------------------------------------------------------

    /** Systemowe okno wyboru zdjęć (Galeria, Pliki, Dysk…), kilka naraz. */
    fun pickImages() {
        runOnUiThread { openImages.launch(arrayOf("image/*")) }
    }

    /** Systemowe okno wyboru folderu – wczytuje z niego wszystkie obrazy (bez podfolderów). */
    fun pickFolder() {
        runOnUiThread { openFolder.launch(null) }
    }

    /** Tekst ze schowka – odpowiedź przez [nativeOnClipboardText]. */
    fun requestClipboardText() {
        runOnUiThread {
            val clipboard = getSystemService(ClipboardManager::class.java)
            val clip = clipboard?.primaryClip
            val text = if (clip != null && clip.itemCount > 0) {
                clip.getItemAt(0).coerceToText(this)?.toString() ?: ""
            } else {
                ""
            }
            nativeOnClipboardText(text)
        }
    }

    /**
     * Okno edycji tekstu z klawiaturą systemową – odpowiedź przez [nativeOnTextEntered].
     * `secret`: klucz API – bez podpowiedzi i autokorekty (klawiatura nie zmienia znaków).
     */
    fun editText(title: String, current: String, secret: Boolean) {
        runOnUiThread {
            val input = EditText(this).apply {
                setText(current)
                setSelection(current.length)
                inputType = if (secret) {
                    InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD or
                        InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
                } else {
                    InputType.TYPE_CLASS_TEXT
                }
                isSingleLine = true
            }
            val pad = (20 * resources.displayMetrics.density).toInt()
            val box = FrameLayout(this).apply {
                setPadding(pad, pad / 2, pad, 0)
                addView(input)
            }
            val dialog = AlertDialog.Builder(this)
                .setTitle(title)
                .setView(box)
                .setPositiveButton(android.R.string.ok) { _, _ -> nativeOnTextEntered(input.text.toString()) }
                .setNegativeButton(android.R.string.cancel, null)
                .create()
            dialog.window?.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_VISIBLE)
            dialog.show()
            input.requestFocus()
        }
    }

    /** Ciemne ikony paska stanu na jasnym motywie (`light`), jasne na ciemnym. */
    fun setLightSystemBars(light: Boolean) {
        runOnUiThread {
            WindowCompat.getInsetsController(window, window.decorView).apply {
                isAppearanceLightStatusBars = light
                isAppearanceLightNavigationBars = light
            }
        }
    }

    /**
     * Zapis JPG do Galerii: Pictures/AsOne (MediaStore, bez uprawnień). Plik AsOne o tej samej
     * nazwie jest nadpisywany. Zwraca miejsce zapisu albo `null` przy błędzie. Wołane z wątku eksportu.
     */
    fun saveImage(name: String, bytes: ByteArray): String? = try {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val dir = Environment.DIRECTORY_PICTURES + "/AsOne/"
            val collection = MediaStore.Images.Media.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)
            val existing = contentResolver.query(
                collection,
                arrayOf(MediaStore.Images.Media._ID),
                "${MediaStore.Images.Media.DISPLAY_NAME}=? AND ${MediaStore.Images.Media.RELATIVE_PATH}=?",
                arrayOf(name, dir),
                null,
            )?.use { c -> if (c.moveToFirst()) ContentUris.withAppendedId(collection, c.getLong(0)) else null }
            val uri = existing ?: contentResolver.insert(collection, ContentValues().apply {
                put(MediaStore.Images.Media.DISPLAY_NAME, name)
                put(MediaStore.Images.Media.MIME_TYPE, "image/jpeg")
                put(MediaStore.Images.Media.RELATIVE_PATH, dir)
                put(MediaStore.Images.Media.IS_PENDING, 1)
            })
            if (uri == null) {
                null
            } else {
                contentResolver.openOutputStream(uri, "wt")?.use { it.write(bytes) } ?: error("brak strumienia")
                if (existing == null) {
                    contentResolver.update(uri, ContentValues().apply { put(MediaStore.Images.Media.IS_PENDING, 0) }, null, null)
                }
                "Pictures/AsOne/$name"
            }
        } else {
            // Android 8–9: katalog aplikacji (bez uprawnień do całej pamięci).
            val dir = File(getExternalFilesDir(Environment.DIRECTORY_PICTURES), "AsOne").apply { mkdirs() }
            File(dir, name).writeBytes(bytes)
            "${dir.path}/$name"
        }
    } catch (e: Exception) {
        null
    }

    // ---- Pomocnicze ------------------------------------------------------------------------

    private fun readPicked(uris: List<Uri>) {
        var ok = 0
        var failed = 0
        for (uri in uris) {
            try {
                val name = displayName(uri) ?: uri.lastPathSegment ?: "zdjecie.jpg"
                val bytes = contentResolver.openInputStream(uri)?.use { it.readBytes() } ?: error("brak danych")
                nativeOnImagePicked(name, bytes)
                ok++
            } catch (e: Exception) {
                failed++
            }
        }
        nativeOnPickFinished(ok, failed)
    }

    private fun readFolder(tree: Uri) {
        var ok = 0
        var failed = 0
        val items = mutableListOf<Pair<String, Uri>>()
        try {
            val parent = DocumentsContract.getTreeDocumentId(tree)
            val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parent)
            val columns = arrayOf(
                DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                DocumentsContract.Document.COLUMN_DISPLAY_NAME,
                DocumentsContract.Document.COLUMN_MIME_TYPE,
            )
            contentResolver.query(children, columns, null, null, null)?.use { c ->
                while (c.moveToNext()) {
                    val mime = c.getString(2) ?: continue
                    if (!mime.startsWith("image/")) continue
                    items += (c.getString(1) ?: "zdjecie.jpg") to DocumentsContract.buildDocumentUriUsingTree(tree, c.getString(0))
                }
            }
        } catch (e: Exception) {
            failed++
        }
        items.sortBy { it.first.lowercase() }
        for ((name, uri) in items) {
            try {
                val bytes = contentResolver.openInputStream(uri)?.use { it.readBytes() } ?: error("brak danych")
                nativeOnImagePicked(name, bytes)
                ok++
            } catch (e: Exception) {
                failed++
            }
        }
        nativeOnPickFinished(ok, failed)
    }

    private fun displayName(uri: Uri): String? =
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0) else null
        }

    // ---- Rust (apps/asone/src/android.rs) ----------------------------------------------------

    private external fun nativeOnInsets(left: Int, top: Int, right: Int, bottom: Int)
    private external fun nativeOnImagePicked(name: String, bytes: ByteArray)
    private external fun nativeOnPickFinished(ok: Int, failed: Int)
    private external fun nativeOnClipboardText(text: String)
    private external fun nativeOnTextEntered(text: String)
}
