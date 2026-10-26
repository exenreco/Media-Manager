package dev.dioxus.main

import android.graphics.Color
import android.graphics.Typeface
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.View
import android.webkit.WebView
import android.widget.Button
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import com.google.android.gms.ads.AdListener
import com.google.android.gms.ads.AdLoader
import com.google.android.gms.ads.AdRequest
import com.google.android.gms.ads.LoadAdError
import com.google.android.gms.ads.MobileAds
import com.google.android.gms.ads.nativead.MediaView
import com.google.android.gms.ads.nativead.NativeAd
import com.google.android.gms.ads.nativead.NativeAdOptions
import com.google.android.gms.ads.nativead.NativeAdView
import com.google.android.ump.ConsentInformation
import com.google.android.ump.ConsentRequestParameters
import com.google.android.ump.UserMessagingPlatform
import org.json.JSONObject
import kotlin.math.roundToInt

typealias BuildConfig = com.tvshowmanager.app.BuildConfig

/** Native ad assets never enter the WebView; the SDK owns clicks and impressions. */
class MainActivity : WryActivity() {
    private val storage by lazy { LocalStorage(this) }
    private var browser: WebView? = null
    private var adView: NativeAdView? = null
    private var nativeAd: NativeAd? = null
    private var consent: ConsentInformation? = null
    private var bounds: JSONObject? = null
    private var loading = false
    private var consentInFlight = false
    private var initializing = false
    private var loader: AdLoader? = null
    private var started = false
    private var foreground = false
    private var dead = false
    private var loadedAt = 0L
    private val timer = Handler(Looper.getMainLooper())
    private val expireAd = Runnable { discardAd(); collapse() }

    override fun onWebViewCreate(webView: WebView) {
        super.onWebViewCreate(webView)
        browser = webView
        if (!WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) return
        WebViewCompat.addWebMessageListener(webView, "MediaManagerStorage",
            setOf("https://dioxus.index.html")) { _, message, origin, mainFrame, _ ->
            if (mainFrame && origin.scheme == "https" && origin.host == "dioxus.index.html") {
                runCatching { JSONObject(message.data ?: "") }.getOrNull()?.let { storage.handle(it, webView) }
            }
        }
        WebViewCompat.addWebMessageListener(webView, "MediaManagerAds",
            setOf("https://dioxus.index.html")) { _, message, origin, mainFrame, _ ->
            if (!mainFrame || origin.scheme != "https" || origin.host != "dioxus.index.html")
                return@addWebMessageListener
            val data = runCatching { JSONObject(message.data ?: "") }.getOrNull()
                ?: return@addWebMessageListener
            when (data.optString("type")) {
                "bounds" -> {
                    updateSafeArea(webView)
                    bounds = data
                    positionAd()
                    if (!started && data.optBoolean("eligible") && foreground) {
                        started = true
                        requestConsent()
                    }
                }
                "retry" -> if (!loading && !consentInFlight && foreground && bounds?.optBoolean("eligible") == true) {
                    started = true; requestConsent()
                }
                "hide" -> { bounds = null; adView?.visibility = View.GONE }
                "privacy" -> if (consent?.privacyOptionsRequirementStatus ==
                    ConsentInformation.PrivacyOptionsRequirementStatus.REQUIRED) {
                    discardAd()
                    UserMessagingPlatform.showPrivacyOptionsForm(this) {
                        updatePrivacyButton()
                        if (consent?.canRequestAds() == true) initializeAds() else collapse()
                    }
                }
            }
        }
    }

    @Deprecated("Activity result API used by WryActivity")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: android.content.Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        storage.result(requestCode, resultCode, data)
    }

    private fun updateSafeArea(web: WebView) {
        val insets = web.rootWindowInsets ?: return
        val top = if (android.os.Build.VERSION.SDK_INT >= 30) {
            insets.getInsets(android.view.WindowInsets.Type.statusBars() or android.view.WindowInsets.Type.displayCutout()).top
        } else {
            @Suppress("DEPRECATION")
            val statusTop = insets.systemWindowInsetTop
            val cutoutTop = if (android.os.Build.VERSION.SDK_INT >= 28) insets.displayCutout?.safeInsetTop ?: 0 else 0
            maxOf(statusTop, cutoutTop)
        }
        val origin = IntArray(2)
        web.getLocationInWindow(origin)
        // Count only overlap: some Android windows already inset the WebView.
        val cssTop = maxOf(0, top - origin[1]) / resources.displayMetrics.density
        web.evaluateJavascript("document.documentElement.style.setProperty('--native-safe-top', '${cssTop}px')", null)
    }

    private fun requestConsent() {
        if (dead || consentInFlight) return
        consentInFlight = true
        report("Checking advertising privacy choices…")
        val info = UserMessagingPlatform.getConsentInformation(this)
        consent = info
        info.requestConsentInfoUpdate(this, ConsentRequestParameters.Builder().build(), {
            if (dead) return@requestConsentInfoUpdate
            UserMessagingPlatform.loadAndShowConsentFormIfRequired(this) { error ->
                if (dead) return@loadAndShowConsentFormIfRequired
                consentInFlight = false
                updatePrivacyButton()
                if (info.canRequestAds()) initializeAds() else collapse("Ads are unavailable until privacy choices can be checked.")
            }
        }, {
            consentInFlight = false
            updatePrivacyButton()
            // UMP may still authorize ads using a valid choice from an earlier session.
            if (info.canRequestAds()) initializeAds() else collapse("Could not check advertising privacy choices. Please retry.")
        })
    }

    private fun updatePrivacyButton() {
        val required = consent?.privacyOptionsRequirementStatus ==
            ConsentInformation.PrivacyOptionsRequirementStatus.REQUIRED
        browser?.evaluateJavascript(
            "document.querySelectorAll('.ad-privacy').forEach(button => button.toggleAttribute('hidden', ${!required}))", null)
    }

    private fun initializeAds() {
        if (dead || consent?.canRequestAds() != true) return
        if (sdkReady) { loadAd(); return }
        if (initializing) return
        initializing = true
        report("Starting advertising…")
        Thread {
            MobileAds.initialize(applicationContext) {
                sdkReady = true
                runOnUiThread { initializing = false; if (!dead) loadAd() }
            }
        }.start()
    }

    private fun loadAd() {
        if (dead || loading || nativeAd != null || consent?.canRequestAds() != true) return
        loading = true
        report("Loading advertisement…")
        val unit = if (BuildConfig.DEBUG) TEST_UNIT else RELEASE_UNIT
        loader = AdLoader.Builder(this, unit)
            .forNativeAd { ad ->
                loading = false
                if (dead || isFinishing || isDestroyed) { ad.destroy() }
                else {
                    discardAd()
                    nativeAd = ad
                    loadedAt = android.os.SystemClock.elapsedRealtime()
                    timer.postDelayed(expireAd, 3_600_000)
                    adView = makeAdView(ad)
                    findViewById<FrameLayout>(android.R.id.content).addView(adView)
                    browser?.evaluateJavascript(
                        "window.mediaManagerAdSetHidden?.(false)", null)
                    report("")
                    positionAd()
                }
            }
            .withNativeAdOptions(NativeAdOptions.Builder()
                .setAdChoicesPlacement(NativeAdOptions.ADCHOICES_TOP_RIGHT).build())
            .withAdListener(object : AdListener() {
                override fun onAdFailedToLoad(error: LoadAdError) {
                    loading = false
                    android.util.Log.w("MediaManagerAds", "Native ad failed: $error")
                    collapse("Ad unavailable (code ${error.code}). You can retry.")
                }
            })
            .build()
        loader?.loadAd(AdRequest.Builder().build())
    }

    private fun makeAdView(ad: NativeAd): NativeAdView {
        val view = NativeAdView(this)
        view.visibility = View.GONE
        view.setBackgroundColor(Color.rgb(30, 43, 59))
        val column = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(12), dp(12), dp(12), dp(12))
        }
        view.addView(column, FrameLayout.LayoutParams(-1, -1))
        fun text(value: String?, height: Int, bold: Boolean = false): TextView {
            return TextView(this).apply {
                text = value.orEmpty()
                setTextColor(Color.WHITE)
                textSize = if (bold) 16f else 12f
                maxLines = 2
                if (bold) setTypeface(null, Typeface.BOLD)
                column.addView(this, LinearLayout.LayoutParams(-1, dp(height)))
            }
        }
        text("Advertisement", 24).apply { textSize = 12f; setPadding(0, 0, dp(32), 0) }
        val heading = LinearLayout(this).apply { gravity = Gravity.CENTER_VERTICAL }
        column.addView(heading, LinearLayout.LayoutParams(-1, dp(52)))
        val icon = ImageView(this).apply { scaleType = ImageView.ScaleType.FIT_CENTER }
        heading.addView(icon, LinearLayout.LayoutParams(dp(44), dp(44)))
        if (ad.icon != null) icon.setImageDrawable(ad.icon?.drawable) else icon.visibility = View.GONE
        val headline = TextView(this).apply {
            text = ad.headline; textSize = 16f; setTextColor(Color.WHITE)
            setTypeface(null, Typeface.BOLD); maxLines = 2; setPadding(dp(8), 0, dp(8), 0)
        }
        heading.addView(headline, LinearLayout.LayoutParams(0, -1, 1f))
        val advertiser = text(ad.advertiser, 20)
        if (ad.advertiser == null) advertiser.visibility = View.GONE
        val body = text(ad.body, 36).apply { maxLines = 2 }
        if (ad.body == null) body.visibility = View.GONE
        val media = MediaView(this)
        media.minimumHeight = dp(120)
        column.addView(media, LinearLayout.LayoutParams(-1, 0, 1f))
        media.mediaContent = ad.mediaContent
        val action = Button(this).apply {
            text = ad.callToAction
            isAllCaps = false
            if (ad.callToAction == null) visibility = View.GONE
        }
        column.addView(action, LinearLayout.LayoutParams(-1, dp(48)))
        view.headlineView = headline
        view.iconView = icon
        view.advertiserView = advertiser
        view.bodyView = body
        view.mediaView = media
        view.callToActionView = action
        view.setNativeAd(ad)
        return view
    }

    private fun positionAd() {
        val view = adView ?: return
        val rect = bounds
        val web = browser ?: return
        val expired = android.os.SystemClock.elapsedRealtime() - loadedAt >= 3_600_000
        if (dead || !foreground || rect == null || !rect.optBoolean("visible") || expired) {
            view.visibility = View.GONE
            if (expired) { discardAd(); collapse() }
            return
        }
        val viewport = rect.optDouble("viewportWidth", 0.0)
        if (!viewport.isFinite() || viewport <= 0) { view.visibility = View.GONE; return }
        val scale = web.width.toDouble() / viewport
        val values = listOf("x", "y", "width", "height").map { rect.optDouble(it, Double.NaN) * scale }
        if (values.any { !it.isFinite() || it < 0 || it > 100_000 }) {
            view.visibility = View.GONE; return
        }
        val (x, y, width, height) = values.map { it.roundToInt() }
        if (width < dp(240) || height < dp(380) || x + width > web.width || y + height > web.height) {
            view.visibility = View.GONE; return
        }
        val compact = height < dp(440)
        view.advertiserView?.visibility = if (compact || nativeAd?.advertiser == null) View.GONE else View.VISIBLE
        view.bodyView?.visibility = if (compact || nativeAd?.body == null) View.GONE else View.VISIBLE
        val parent = findViewById<FrameLayout>(android.R.id.content)
        val webOrigin = IntArray(2); val parentOrigin = IntArray(2)
        web.getLocationInWindow(webOrigin); parent.getLocationInWindow(parentOrigin)
        view.layoutParams = FrameLayout.LayoutParams(width, height).apply {
            leftMargin = webOrigin[0] - parentOrigin[0] + x
            topMargin = webOrigin[1] - parentOrigin[1] + y
        }
        view.visibility = View.VISIBLE
    }

    private fun report(message: String, retry: Boolean = false) {
        browser?.evaluateJavascript("window.mediaManagerAdStatus?.(${JSONObject.quote(message)}, $retry)", null)
    }

    private fun collapse(message: String = "No advertisement is available. You can retry.") {
        report(message, true)
        adView?.visibility = View.GONE
        browser?.evaluateJavascript(
            "window.mediaManagerAdSetHidden?.(true)", null)
    }

    private fun discardAd() {
        timer.removeCallbacks(expireAd)
        adView?.let { (it.parent as? android.view.ViewGroup)?.removeView(it); it.destroy() }
        adView = null
        nativeAd?.destroy(); nativeAd = null
    }

    override fun onResume() {
        super.onResume(); foreground = true; positionAd()
        if (!started && bounds?.optBoolean("eligible") == true) {
            started = true; requestConsent()
        }
    }
    override fun onPause() {
        foreground = false; adView?.visibility = View.GONE; super.onPause()
    }
    override fun onDestroy() {
        dead = true; discardAd(); storage.close(); browser = null; super.onDestroy()
    }
    private fun dp(value: Int) = (value * resources.displayMetrics.density).roundToInt()

    companion object {
        @Volatile private var sdkReady = false
        private const val TEST_UNIT = "ca-app-pub-3940256099942544/2247696110"
        private const val RELEASE_UNIT = "ca-app-pub-9696662815161119/4253855527"
    }
}

/** Storage Access Framework access, restricted to folders explicitly granted by the user. */
private class LocalStorage(private val activity: MainActivity) {
    private val resolver get() = activity.contentResolver
    private val executor = java.util.concurrent.Executors.newSingleThreadExecutor()
    private var pending: Pair<JSONObject, WebView>? = null
    fun close() { pending = null; executor.shutdown() }
    private val requestCode = 7184
    private val projection = arrayOf("document_id", "_display_name", "mime_type", "_size", "last_modified", "flags")
    private val directory = android.provider.DocumentsContract.Document.MIME_TYPE_DIR
    private data class Doc(val uri: android.net.Uri, val id: String, val name: String, val dir: Boolean, val size: Long, val modified: Long, val flags: Long)

    private fun reply(web: WebView, command: JSONObject, value: JSONObject) {
        value.put("id", command.optString("id"))
        val script = "window.dispatchEvent(new CustomEvent('media-manager-storage',{detail:" + value.toString() + "}))"
        activity.runOnUiThread { web.evaluateJavascript(script, null) }
    }
    fun handle(command: JSONObject, web: WebView) {
        if (command.optString("op") == "pick" || command.optString("op") == "pickImage") {
            if (pending != null) { reply(web,command,JSONObject().put("error","A folder picker is already open")); return }
            pending = Pair(command, web)
            try {
                if (command.optString("op") == "pickImage") {
                    activity.startActivityForResult(android.content.Intent(android.content.Intent.ACTION_GET_CONTENT).apply { type="image/*"; addCategory(android.content.Intent.CATEGORY_OPENABLE) },requestCode)
                    return
                }
                activity.startActivityForResult(android.content.Intent(android.content.Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
                    addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION or android.content.Intent.FLAG_GRANT_WRITE_URI_PERMISSION or android.content.Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION or android.content.Intent.FLAG_GRANT_PREFIX_URI_PERMISSION)
                },requestCode)
            } catch (error: Exception) { pending=null; reply(web,command,JSONObject().put("error",error.message)) }
            return
        }
        executor.execute {
            try { reply(web,command,execute(command)) }
            catch (error: Exception) { reply(web,command,JSONObject().put("error",error.message ?: "Document access failed; select the folder again")) }
        }
    }
    fun result(code: Int, result: Int, data: android.content.Intent?) {
        if (code != requestCode) return
        val (command,web)=pending ?: return; pending=null
        try {
            val uri=data?.data
            require(result == android.app.Activity.RESULT_OK && uri != null) { "Folder selection cancelled" }
            if (command.optString("op") == "pickImage") {
                executor.execute {
                    try {
                        val bytes=java.io.ByteArrayOutputStream()
                        resolver.openInputStream(uri!!).use { stream -> require(stream!=null); val buffer=ByteArray(65536);while(true){val count=stream.read(buffer);if(count<0)break;require(bytes.size()+count<=10*1024*1024){"QR image exceeds 10 MB"};bytes.write(buffer,0,count)} }
                        reply(web,command,JSONObject().put("data",android.util.Base64.encodeToString(bytes.toByteArray(),android.util.Base64.NO_WRAP)))
                    } catch(error:Exception){reply(web,command,JSONObject().put("error",error.message))}
                }
                return
            }
            val flags=data!!.flags and (android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION or android.content.Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
            require(flags and android.content.Intent.FLAG_GRANT_WRITE_URI_PERMISSION != 0) { "This folder provider did not grant write access" }
            resolver.takePersistableUriPermission(uri,flags)
            reply(web,command,JSONObject().put("root",uri.toString()))
        } catch (error: Exception) { reply(web,command,JSONObject().put("error",error.message)) }
    }
    private fun root(command: JSONObject): android.net.Uri {
        val uri=android.net.Uri.parse(command.getString("root"))
        require(uri.scheme == "content" && android.provider.DocumentsContract.isTreeUri(uri)) { "Choose a document folder first" }
        require(resolver.persistedUriPermissions.any { it.uri == uri && it.isReadPermission && it.isWritePermission }) { "Folder permission expired; select the folder again" }
        return uri
    }
    private fun read(tree: android.net.Uri, id: String): Doc {
        val uri=android.provider.DocumentsContract.buildDocumentUriUsingTree(tree,id)
        resolver.query(uri,projection,null,null,null).use { cursor ->
            require(cursor != null && cursor.moveToFirst()) { "Document no longer exists" }
            return Doc(uri,cursor.getString(0),cursor.getString(1),cursor.getString(2)==directory,cursor.getLong(3),cursor.getLong(4),cursor.getLong(5))
        }
    }
    private fun children(tree: android.net.Uri, doc: Doc): List<Doc> {
        require(doc.dir) { "Expected a folder" }
        val result=mutableListOf<Doc>()
        resolver.query(android.provider.DocumentsContract.buildChildDocumentsUriUsingTree(tree,doc.id),projection,null,null,null).use { cursor ->
            require(cursor != null) { "Could not list folder" }
            while(cursor.moveToNext()) {
                val id=cursor.getString(0);val name=cursor.getString(1)
                require(name.isNotEmpty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\\')) { "Unsupported document name" }
                result.add(Doc(android.provider.DocumentsContract.buildDocumentUriUsingTree(tree,id),id,name,cursor.getString(2)==directory,cursor.getLong(3),cursor.getLong(4),cursor.getLong(5)))
            }
        }
        require(result.map { it.name }.distinct().size == result.size) { "Provider returned duplicate names" }
        return result
    }
    private fun parts(path: String): List<String> {
        if(path.isEmpty()) return emptyList()
        return path.split('/').also { list -> require(list.all { it.isNotEmpty() && it != "." && it != ".." && !it.contains('\\') }) { "Invalid document path" } }
    }
    private fun resolve(tree: android.net.Uri, path: String): Doc {
        var doc=read(tree,android.provider.DocumentsContract.getTreeDocumentId(tree))
        for(part in parts(path)) doc=children(tree,doc).singleOrNull { it.name == part } ?: error("Document no longer exists")
        return doc
    }
    private fun execute(command: JSONObject): JSONObject {
        val tree=root(command)
        when(command.getString("op")) {
            "list" -> {
                val entries=org.json.JSONArray(); val visited=mutableSetOf<String>()
                fun walk(doc: Doc, path: String, depth: Int) {
                    require(depth <= 32 && entries.length() < 20000 && visited.add(doc.id)) { "Folder exceeds scan limits or contains a cycle" }
                    for(child in children(tree,doc)) {
                        val relative=if(path.isEmpty()) child.name else "$path/${child.name}"
                        require(entries.length() < 20000) { "Folder exceeds 20,000 entries" }
                        entries.put(JSONObject().put("path",relative).put("dir",child.dir).put("size",child.size).put("modified",child.modified).put("identity",child.id))
                        if(child.dir) walk(child,relative,depth+1)
                    }
                }
                val doc=resolve(tree,"");walk(doc,"",0)
                return JSONObject().put("entries",entries).put("name",doc.name).put("cache",activity.cacheDir.absolutePath).put("state",activity.filesDir.absolutePath)
            }
            "hash" -> {
                val doc=resolve(tree,command.getString("path"));require(!doc.dir)
                val digest=java.security.MessageDigest.getInstance("SHA-256")
                resolver.openInputStream(doc.uri).use {stream -> require(stream!=null);val buffer=ByteArray(65536);while(true){val count=stream.read(buffer);if(count<0)break;digest.update(buffer,0,count)}}
                val after=read(tree,doc.id);require(after.size==doc.size && after.modified==doc.modified){"File changed while scanning"}
                return JSONObject().put("sha256",digest.digest().joinToString(""){"%02x".format(it)})
            }
            "rename" -> {
                val path=command.getString("path");val names=parts(path);require(names.isNotEmpty())
                val name=command.getString("name");require(parts(name).size==1 && !name.contains('/'))
                val doc=resolve(tree,path);val parent=resolve(tree,names.dropLast(1).joinToString("/"))
                require(doc.id==command.getString("identity") && doc.dir==command.getBoolean("dir")) { "Document changed; rebuild preview" }
                if(!doc.dir) require(doc.modified > 0 && doc.size==command.getLong("size") && doc.modified==command.getLong("modified")) { "File changed since preview" }
                require(doc.flags and android.provider.DocumentsContract.Document.FLAG_SUPPORTS_RENAME.toLong() != 0L) { "Provider does not support renaming this document" }
                require(children(tree,parent).none { it.name.equals(name,true) }) { "Destination exists; nothing was overwritten" }
                val updated=android.provider.DocumentsContract.renameDocument(resolver,doc.uri,name) ?: error("Provider refused rename")
                require(read(tree,android.provider.DocumentsContract.getDocumentId(updated)).name==name) { "Provider changed the requested name; rebuild preview" }
            }
            "write" -> {
                val path=parts(command.getString("path"));require(path.isNotEmpty())
                val parent=resolve(tree,path.dropLast(1).joinToString("/"));val name=path.last()
                if(children(tree,parent).any { it.name.equals(name,true) }) return JSONObject().put("kept",true)
                val bytes=android.util.Base64.decode(command.getString("data"),android.util.Base64.DEFAULT);require(bytes.size<=10*1024*1024)
                val mime=when(name.substringAfterLast('.')) { "png" -> "image/png"; "webp" -> "image/webp"; else -> "image/jpeg" }
                val uri=android.provider.DocumentsContract.createDocument(resolver,parent.uri,mime,name) ?: error("Could not create poster")
                if(read(tree,android.provider.DocumentsContract.getDocumentId(uri)).name != name) {
                    android.provider.DocumentsContract.deleteDocument(resolver,uri)
                    return JSONObject().put("kept",true)
                }
                try {
                    resolver.openOutputStream(uri,"w").use { stream -> require(stream!=null);stream.write(bytes) }
                } catch(error: Exception) { android.provider.DocumentsContract.deleteDocument(resolver,uri);throw error }
            }
            "play" -> {
                val doc=resolve(tree,command.getString("path"));require(!doc.dir)
                val mime=resolver.getType(doc.uri)?.takeUnless { it=="application/octet-stream" } ?: "video/*"
                val launch=java.util.concurrent.FutureTask<Unit> {
                    activity.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW).setDataAndType(doc.uri,mime).addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION))
                }
                activity.runOnUiThread(launch)
                launch.get()

            }
            else -> error("Unknown storage command")
        }
        return JSONObject()
    }
}
