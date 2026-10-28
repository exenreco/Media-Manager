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
    private val mediaPreviews by lazy { MediaPreviews(this) }
    private val storage by lazy { LocalStorage(this,mediaPreviews) }
    private val fullscreen: FullscreenAds by lazy { FullscreenAds(this,
        { !dead && foreground && sdkReady && consent?.canRequestAds() == true },
        { state -> state.put("privacyRequired", consent?.privacyOptionsRequirementStatus == ConsentInformation.PrivacyOptionsRequirementStatus.REQUIRED); browser?.evaluateJavascript("window.mediaManagerFullscreenState?.(${state})", null) },
        { positionAd(); if (!fullscreenSuppressed()) loadAd() }) }
    private fun fullscreenSuppressed(): Boolean = fullscreen.suppressed
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
        mediaPreviews.attach(webView)
        // Disposable private fixture, used only by the native playback acceptance harness.
        if(BuildConfig.DEBUG && intent.getBooleanExtra("media_manager_preview_fixture",false)) {
            timer.postDelayed({
                val fixture=java.io.File(filesDir,"compatibility-fixture.mp4")
                if(fixture.isFile) {
                    val prepared=mediaPreviews.local(android.net.Uri.fromFile(fixture),fixture.length(),"video/mp4")
                    mediaPreviews.play("/native-media-preview/"+prepared.getString("token"))
                }
            },1500)
        }
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
                "fullscreen-state" -> { fullscreen.state(); fullscreen.load(); if (!started) { started = true; requestConsent() } }
                "interstitial" -> fullscreen.show(false)
                "rewarded" -> { fullscreen.show(true); if (consent?.canRequestAds() != true) requestConsent() }
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
                    discardAd(); fullscreen.reset()
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
        fullscreen.state()
        val required = consent?.privacyOptionsRequirementStatus ==
            ConsentInformation.PrivacyOptionsRequirementStatus.REQUIRED
        browser?.evaluateJavascript(
            "document.querySelectorAll('.ad-privacy').forEach(button => button.toggleAttribute('hidden', ${!required}))", null)
    }

    private fun initializeAds() {
        if (dead || consent?.canRequestAds() != true) return
        if (sdkReady) { loadAd(); fullscreen.load(); return }
        if (initializing) return
        initializing = true
        report("Starting advertising…")
        Thread {
            MobileAds.initialize(applicationContext) {
                sdkReady = true
                runOnUiThread { initializing = false; if (!dead) { loadAd(); fullscreen.load() } }
            }
        }.start()
    }

    private fun loadAd() {
        if (dead || fullscreenSuppressed() || loading || nativeAd != null || consent?.canRequestAds() != true) return
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
        if (dead || fullscreenSuppressed() || !foreground || rect == null || !rect.optBoolean("visible") || expired) {
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
        super.onResume(); foreground = true; positionAd(); fullscreen.state(); fullscreen.load()
        if (!started && bounds?.optBoolean("eligible") == true) {
            started = true; requestConsent()
        }
    }
    override fun onPause() {
        foreground = false; mediaPreviews.pause(); adView?.visibility = View.GONE; super.onPause()
    }
    override fun onDestroy() {
        dead = true; fullscreen.close(); discardAd(); mediaPreviews.clear(); storage.close(); browser = null; super.onDestroy()
    }
    private fun dp(value: Int) = (value * resources.displayMetrics.density).roundToInt()

    companion object {
        @Volatile private var sdkReady = false
        private const val TEST_UNIT = "ca-app-pub-3940256099942544/2247696110"
        private const val RELEASE_UNIT = "ca-app-pub-9696662815161119/4253855527"
    }
}

/** Storage Access Framework access, restricted to folders explicitly granted by the user. */
private class LocalStorage(private val activity: MainActivity, private val mediaPreviews: MediaPreviews) {
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
        if (command.optString("op") == "appDirectories") {
            reply(web,command,JSONObject().put("state",activity.filesDir.absolutePath));return
        }
        if (command.optString("op") == "scanPairing") {
            if (pending != null) { reply(web, command, JSONObject().put("error", "Close the current scanner or picker first")); return }
            pending = Pair(command, web)
            try {
                com.google.zxing.integration.android.IntentIntegrator(activity)
                    .setCaptureActivity(PairingCaptureActivity::class.java)
                    .setDesiredBarcodeFormats(com.google.zxing.integration.android.IntentIntegrator.QR_CODE)
                    .setPrompt("Align the desktop Media Manager QR code inside the guide")
                    .setTimeout(240000L)
                    .setBeepEnabled(false)
                    .setOrientationLocked(false)
                    .initiateScan()
            } catch (error: Exception) { pending = null; reply(web, command, JSONObject().put("error", error.message ?: "Camera unavailable")) }
            return
        }
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
        if (code == com.google.zxing.integration.android.IntentIntegrator.REQUEST_CODE) {
            val current = pending ?: return
            if (current.first.optString("op") != "scanPairing") return
            pending = null
            val scanned = com.google.zxing.integration.android.IntentIntegrator.parseActivityResult(code, result, data)
            val text = scanned?.contents
            if (text == null) reply(current.second, current.first, JSONObject().put("error", "Scan cancelled or camera permission denied"))
            else if (text.length > 8192) reply(current.second, current.first, JSONObject().put("error", "Pairing QR is too large"))
            else reply(current.second, current.first, JSONObject().put("text", text))
            return
        }
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
        if(command.getString("op")=="playCompatiblePreview") {mediaPreviews.play(command.getString("source"));return JSONObject()}
        if(command.getString("op")=="prepareRemotePreview") return mediaPreviews.remote(command)
        if(command.getString("op")=="stopMediaPreview") {
            if(!(BuildConfig.DEBUG && activity.intent.getBooleanExtra("media_manager_preview_fixture",false))) mediaPreviews.clear()
            return JSONObject()
        }
        val tree=root(command)
        when(command.getString("op")) {
            "checkFolder" -> {
                require(resolve(tree, "").dir) { "Choose a media folder first" }
                return JSONObject().put("connected",true)
            }
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
            "prepareMediaPreview" -> {
                val doc=resolve(tree,command.getString("path"));require(!doc.dir)
                val mime=command.getString("mime");require(mime.startsWith("video/") || mime == "application/vnd.rn-realmedia")
                return mediaPreviews.local(doc.uri,doc.size,mime)
            }
            "readMedia" -> {
                val doc=resolve(tree,command.getString("path"));require(!doc.dir)
                val offset=command.getLong("offset");val count=command.getInt("count")
                require(offset>=0 && count in 0..1048576 && offset<=doc.size && count.toLong()<=doc.size-offset) { "Invalid media range" }
                val data=ByteArray(count)
                if(count>0) resolver.openFileDescriptor(doc.uri,"r").use { descriptor ->
                    require(descriptor!=null) { "Media unavailable" }
                    java.io.FileInputStream(descriptor.fileDescriptor).use { input ->
                        input.channel.position(offset)
                        var read=0
                        while(read<count) { val n=input.read(data,read,count-read);require(n>0){"Incomplete media read"};read+=n }
                    }
                }
                return JSONObject().put("size",doc.size).put("data",android.util.Base64.encodeToString(data,android.util.Base64.NO_WRAP))
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
            "savePoster" -> {
                val path=parts(command.getString("path"));require(path.isNotEmpty())
                val parent=resolve(tree,path.dropLast(1).joinToString("/"));val name=path.last()
                require(name.substringAfterLast('.').lowercase() in listOf("jpg","jpeg","png","webp"))
                val existing=children(tree,parent).firstOrNull {it.name.equals(name,true)}
                if(existing!=null) {
                    require(command.optBoolean("replace")){"Confirm Replace existing poster first"}
                    require(!existing.dir && existing.size<=10*1024*1024)
                    val digest=java.security.MessageDigest.getInstance("SHA-256")
                    resolver.openInputStream(existing.uri).use {stream -> require(stream!=null);val buffer=ByteArray(65536);while(true){val n=stream.read(buffer);if(n<0)break;digest.update(buffer,0,n)}}
                    require(digest.digest().joinToString(""){"%02x".format(it)}==command.optString("expected")){"Existing poster changed; select it again"}
                }
                val bytes=android.util.Base64.decode(command.getString("data"),android.util.Base64.DEFAULT);require(bytes.size<=10*1024*1024)
                val mime=when(name.substringAfterLast('.').lowercase()) {"png"->"image/png";"webp"->"image/webp";else->"image/jpeg"}
                val temporary=".media-manager-${java.util.UUID.randomUUID()}.${name.substringAfterLast('.')}"
                val temp=android.provider.DocumentsContract.createDocument(resolver,parent.uri,mime,temporary) ?: error("Could not create poster")
                var backup:android.net.Uri?=null
                try {
                    resolver.openOutputStream(temp,"w").use {stream -> require(stream!=null);stream.write(bytes)}
                    if(existing!=null) {backup=android.provider.DocumentsContract.renameDocument(resolver,existing.uri,".media-manager-backup-${java.util.UUID.randomUUID()}.${name.substringAfterLast('.')}") ?: error("Could not preserve existing poster")}
                    val saved=android.provider.DocumentsContract.renameDocument(resolver,temp,name) ?: error("Could not save poster")
                    require(read(tree,android.provider.DocumentsContract.getDocumentId(saved)).name==name){"Provider changed the poster name"}
                    backup?.let {android.provider.DocumentsContract.deleteDocument(resolver,it)}
                } catch(error:Exception) {
                    if(backup!=null && children(tree,parent).none {it.name.equals(name,true)}) {android.provider.DocumentsContract.renameDocument(resolver,backup!!,name)}
                    try {android.provider.DocumentsContract.deleteDocument(resolver,temp)} catch(_:Exception) {}
                    throw error
                }
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


/** Dedicated portrait QR scanner; the guide matches the decoder's square crop. */
class PairingCaptureActivity : com.journeyapps.barcodescanner.CaptureActivity() {
    override fun initializeContent(): com.journeyapps.barcodescanner.DecoratedBarcodeView {
        val root = FrameLayout(this).apply { setBackgroundColor(Color.BLACK) }
        val scanner = com.journeyapps.barcodescanner.DecoratedBarcodeView(this)
        scanner.viewFinder.setLaserVisibility(false)
        root.addView(scanner, FrameLayout.LayoutParams(-1, -1))
        val guide = object : View(this) {
            private val line = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG).apply {
                color = Color.rgb(157, 232, 208); style = android.graphics.Paint.Style.STROKE
                strokeWidth = 3 * resources.displayMetrics.density
            }
            override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
                super.onSizeChanged(w, h, oldw, oldh)
                val side = (minOf(w, h) * 0.72).roundToInt()
                scanner.barcodeView.framingRectSize = com.journeyapps.barcodescanner.Size(side, side)
            }
            override fun onDraw(canvas: android.graphics.Canvas) {
                super.onDraw(canvas)
                val side = minOf(width, height) * 0.72f
                val left = (width - side) / 2; val top = (height - side) / 2
                canvas.drawRoundRect(android.graphics.RectF(left, top, left + side, top + side), 16f, 16f, line)
            }
        }
        guide.isClickable = false
        guide.importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        root.addView(guide, FrameLayout.LayoutParams(-1, -1))
        val cancel = Button(this).apply {
            text = "Cancel"; contentDescription = "Cancel pairing scan"
            setOnClickListener { setResult(android.app.Activity.RESULT_CANCELED); finish() }
        }
        val density = resources.displayMetrics.density
        root.addView(cancel, FrameLayout.LayoutParams(-2, -2, Gravity.TOP or Gravity.START).apply {
            leftMargin = (20 * density).roundToInt(); topMargin = (48 * density).roundToInt()
        })
        setContentView(root)
        return scanner
    }
}

/** Fullscreen objects are single use. Entitlements never come from WebView messages. */
private class FullscreenAds(
    private val activity: MainActivity,
    private val allowed: () -> Boolean,
    private val publish: (JSONObject) -> Unit,
    private val changed: () -> Unit
) {
    private val prefs = activity.getSharedPreferences("advertising", android.content.Context.MODE_PRIVATE)
    private val handler = Handler(Looper.getMainLooper())
    private val sessionStart = android.os.SystemClock.elapsedRealtime()
    private var interstitial: com.google.android.gms.ads.interstitial.InterstitialAd? = null
    private var rewarded: com.google.android.gms.ads.rewarded.RewardedAd? = null
    private var interstitialAt = 0L
    private var rewardedAt = 0L
    private var loadingInterstitial = false
    private var loadingRewarded = false
    private var generation = 0
    private var closed = false
    var presenting = false; private set
    private var earned = false
    val adFree: Boolean get() = prefs.getLong("adFreeUntil", 0L) > System.currentTimeMillis()
    val suppressed: Boolean get() = adFree || presenting
    private val expiry = Runnable { state(); changed(); load() }

    fun state(message: String = "") {
        if (closed) return
        publish(JSONObject().put("adFreeUntil", prefs.getLong("adFreeUntil", 0L))
            .put("presenting", presenting).put("rewardedReady", rewarded != null && fresh(rewardedAt)).put("message", message))
        handler.removeCallbacks(expiry)
        if (adFree) handler.postDelayed(expiry, prefs.getLong("adFreeUntil", 0L) - System.currentTimeMillis() + 50)
    }
    private fun fresh(time: Long) = android.os.SystemClock.elapsedRealtime() - time < 3_600_000
    fun reset() { generation++; interstitial = null; rewarded = null; loadingInterstitial = false; loadingRewarded = false; state() }
    fun close() { closed = true; reset(); handler.removeCallbacksAndMessages(null) }
    fun load() {
        if (closed || !allowed() || suppressed) return
        if (!fresh(interstitialAt)) interstitial = null
        if (!fresh(rewardedAt)) rewarded = null
        val epoch = generation
        if (interstitial == null && !loadingInterstitial) {
            loadingInterstitial = true
            val unit = if (BuildConfig.DEBUG) "ca-app-pub-3940256099942544/1033173712" else "ca-app-pub-9696662815161119/3326470014"
            com.google.android.gms.ads.interstitial.InterstitialAd.load(activity, unit, AdRequest.Builder().build(),
                object : com.google.android.gms.ads.interstitial.InterstitialAdLoadCallback() {
                    override fun onAdLoaded(ad: com.google.android.gms.ads.interstitial.InterstitialAd) {
                        if (closed || epoch != generation) return
                        loadingInterstitial = false
                        if (allowed() && !adFree) { interstitial = ad; interstitialAt = android.os.SystemClock.elapsedRealtime() }
                        state()
                    }
                    override fun onAdFailedToLoad(error: LoadAdError) { if (!closed && epoch == generation) { loadingInterstitial = false; state() } }
                })
        }
        if (rewarded == null && !loadingRewarded) {
            loadingRewarded = true
            val unit = if (BuildConfig.DEBUG) "ca-app-pub-3940256099942544/5224354917" else "ca-app-pub-9696662815161119/8879460859"
            com.google.android.gms.ads.rewarded.RewardedAd.load(activity, unit, AdRequest.Builder().build(),
                object : com.google.android.gms.ads.rewarded.RewardedAdLoadCallback() {
                    override fun onAdLoaded(ad: com.google.android.gms.ads.rewarded.RewardedAd) {
                        if (closed || epoch != generation) return
                        loadingRewarded = false
                        if (allowed() && !adFree) { rewarded = ad; rewardedAt = android.os.SystemClock.elapsedRealtime() }
                        state()
                    }
                    override fun onAdFailedToLoad(error: LoadAdError) {
                        if (!closed && epoch == generation) { loadingRewarded = false; state("No rewarded ad is available. Please try again later.") }
                    }
                })
        }
    }
    fun show(reward: Boolean) {
        if (closed || !allowed() || suppressed || activity.isFinishing || activity.isDestroyed || !activity.hasWindowFocus()) return
        if (!reward && (android.os.SystemClock.elapsedRealtime() - sessionStart < 120_000 ||
            System.currentTimeMillis() - prefs.getLong("lastInterstitial", 0L) < 600_000)) return
        if (reward && (rewarded == null || !fresh(rewardedAt))) {
            rewarded = null; load(); state("Rewarded ad not ready. Please try again shortly."); return
        }
        if (!reward && (interstitial == null || !fresh(interstitialAt))) { interstitial = null; load(); return }
        val full = interstitial; val offer = rewarded
        if (reward) rewarded = null else interstitial = null
        presenting = true; earned = false; changed(); state()
        val callback = object : com.google.android.gms.ads.FullScreenContentCallback() {
            override fun onAdShowedFullScreenContent() {
                if (!reward) prefs.edit().putLong("lastInterstitial", System.currentTimeMillis()).apply()
            }
            override fun onAdDismissedFullScreenContent() { finish(if (reward && !earned) "Ad closed. No reward was earned." else "") }
            override fun onAdFailedToShowFullScreenContent(error: com.google.android.gms.ads.AdError) { finish("Ad could not be shown. Please try again later.") }
        }
        if (reward) {
            offer!!.fullScreenContentCallback = callback
            offer.show(activity) {
                if (!closed && !earned) {
                    earned = true
                    prefs.edit().putLong("adFreeUntil", System.currentTimeMillis() + 1_800_000).apply()
                    interstitial = null; state(); changed()
                }
            }
        } else { full!!.fullScreenContentCallback = callback; full.show(activity) }
    }
    private fun finish(message: String) { presenting = false; state(message); changed(); load() }
}

/** Streams media without copying video bytes through JavaScript or Rust's Android request bridge. */
private class MediaPreviews(private val activity: MainActivity) {
    private data class Source(val uri: android.net.Uri?, val url: String?, val tls: javax.net.ssl.SSLSocketFactory?, val size: Long, val mime: String)
    private val sources = java.util.concurrent.ConcurrentHashMap<String, Source>()
    private val streams = java.util.Collections.synchronizedSet(mutableSetOf<java.io.InputStream>())
    private var supported = false
    private var compatible: CompatiblePreview? = null
    fun attach(web: WebView) {
        if (android.os.Build.VERSION.SDK_INT < 26) return
        val original = web.webViewClient
        web.webViewClient = object : android.webkit.WebViewClient() {
            override fun shouldInterceptRequest(view: WebView, request: android.webkit.WebResourceRequest): android.webkit.WebResourceResponse? {
                if (request.url.scheme == "https" && request.url.host == "dioxus.index.html" && request.url.path?.startsWith("/native-media-preview/") == true) return response(request)
                return original.shouldInterceptRequest(view, request)
            }
            override fun shouldOverrideUrlLoading(view: WebView, request: android.webkit.WebResourceRequest) = original.shouldOverrideUrlLoading(view,request)
            override fun onPageStarted(view: WebView, url: String, icon: android.graphics.Bitmap?) = original.onPageStarted(view,url,icon)
            override fun onPageFinished(view: WebView, url: String) = original.onPageFinished(view,url)
            override fun onPageCommitVisible(view: WebView, url: String) = original.onPageCommitVisible(view,url)
            override fun onReceivedError(view: WebView, request: android.webkit.WebResourceRequest, error: android.webkit.WebResourceError) = original.onReceivedError(view,request,error)
            override fun onReceivedHttpError(view: WebView, request: android.webkit.WebResourceRequest, error: android.webkit.WebResourceResponse) = original.onReceivedHttpError(view,request,error)
            override fun onReceivedSslError(view: WebView, handler: android.webkit.SslErrorHandler, error: android.net.http.SslError) = original.onReceivedSslError(view,handler,error)
            override fun onRenderProcessGone(view: WebView, detail: android.webkit.RenderProcessGoneDetail) = original.onRenderProcessGone(view,detail)
            override fun doUpdateVisitedHistory(view: WebView, url: String, reload: Boolean) = original.doUpdateVisitedHistory(view,url,reload)
        }
        supported = true
    }
    fun play(source: String) {
        val token = source.removePrefix("/native-media-preview/")
        require(source == "/native-media-preview/$token" && sources.containsKey(token)) { "Preview expired; select the file again" }
        activity.runOnUiThread {
            compatible?.close()
            compatible = CompatiblePreview(activity, token, ::response).also { it.show() }
        }
    }
    fun pause(){activity.runOnUiThread {compatible?.pause()}}
    fun clear() {
        activity.runOnUiThread { compatible?.close(); compatible = null }
        sources.clear()
        synchronized(streams) { streams.toList().forEach { runCatching { it.close() } }; streams.clear() }
    }
    fun local(uri: android.net.Uri, size: Long, mime: String): JSONObject = register(Source(uri,null,null,size,mime))
    fun remote(command: JSONObject): JSONObject {
        val url = java.net.URI(command.getString("url"))
        require(url.scheme == "https" || (url.scheme == "http" && url.host in listOf("localhost","127.0.0.1","[::1]","::1"))) { "Remote playback requires HTTPS" }
        require(url.rawUserInfo == null && url.rawQuery == null && url.rawFragment == null && Regex("/v1/media/[0-9a-fA-F-]{36}").matches(url.path)) { "Invalid playback URL" }
        val certificate = command.optString("certificate")
        val tls = if (certificate.isEmpty()) null else {
            val bytes = android.util.Base64.decode(certificate,android.util.Base64.DEFAULT)
            require(bytes.size <= 16384)
            val cert = java.security.cert.CertificateFactory.getInstance("X.509").generateCertificate(bytes.inputStream())
            val store = java.security.KeyStore.getInstance(java.security.KeyStore.getDefaultType()).apply { load(null); setCertificateEntry("desktop",cert) }
            val trust = javax.net.ssl.TrustManagerFactory.getInstance(javax.net.ssl.TrustManagerFactory.getDefaultAlgorithm()).apply { init(store) }
            javax.net.ssl.SSLContext.getInstance("TLS").apply { init(null,trust.trustManagers,null) }.socketFactory
        }
        return register(Source(null,url.toString(),tls,command.getLong("size"),command.getString("mime")))
    }
    private fun register(source: Source): JSONObject {
        require(supported) { "Inline preview requires Android 8 or newer. Open the file with your device video app." }
        require(source.size > 0) { "Empty media file" }
        clear()
        val token = java.util.UUID.randomUUID().toString(); sources[token] = source
        return JSONObject().put("token",token).put("size",source.size)
    }
    private fun response(request: android.webkit.WebResourceRequest): android.webkit.WebResourceResponse {
        fun empty(code: Int, reason: String, extra: Map<String,String> = emptyMap()) = android.webkit.WebResourceResponse("text/plain",null,code,reason,extra,java.io.ByteArrayInputStream(byteArrayOf()))
        val token = request.url.path?.removePrefix("/native-media-preview/") ?: return empty(404,"Not Found")
        val source = sources[token] ?: return empty(404,"Not Found")
        if (request.method !in listOf("GET","HEAD")) return empty(405,"Method Not Allowed")
        val raw = request.requestHeaders.entries.firstOrNull { it.key.equals("range",true) }?.value
        val range = try { previewRange(raw,source.size) } catch (_: Exception) { return empty(416,"Range Not Satisfiable",mapOf("Content-Range" to "bytes */${source.size}")) }
        val start=range.first; val count=range.second-start+1
        val headers=mutableMapOf("Accept-Ranges" to "bytes","Content-Length" to count.toString(),"Cache-Control" to "no-store","Referrer-Policy" to "no-referrer")
        if(raw!=null) headers["Content-Range"]="bytes $start-${range.second}/${source.size}"
        if(request.method=="HEAD") return android.webkit.WebResourceResponse(source.mime,null,if(raw==null)200 else 206,if(raw==null)"OK" else "Partial Content",headers,java.io.ByteArrayInputStream(byteArrayOf()))
        return try {
            val input: java.io.InputStream
            var connection: java.net.HttpURLConnection? = null
            if(source.uri!=null) {
                val descriptor=activity.contentResolver.openFileDescriptor(source.uri,"r") ?: error("Media unavailable")
                val file=android.os.ParcelFileDescriptor.AutoCloseInputStream(descriptor)
                try { file.channel.position(start) } catch(error:Exception) {file.close();throw error}
                input=file
            } else {
                connection=java.net.URL(source.url!!).openConnection() as java.net.HttpURLConnection
                connection.instanceFollowRedirects=false; connection.connectTimeout=10000; connection.readTimeout=10000
                if(connection is javax.net.ssl.HttpsURLConnection && source.tls!=null) connection.sslSocketFactory=source.tls
                connection.setRequestProperty("Range","bytes=$start-${range.second}")
                try {
                    require(connection.responseCode==206 && connection.getHeaderField("Content-Range")=="bytes $start-${range.second}/${source.size}") { "Invalid server media range" }
                    input=connection.inputStream
                } catch(error:Exception) {connection.disconnect();throw error}
            }
            val remote=connection
            val bounded=object : java.io.FilterInputStream(input) {
                private var remaining=count
                override fun read(): Int {if(remaining<=0)return -1;val n=super.read();if(n>=0)remaining--;return n}
                override fun read(data:ByteArray,offset:Int,length:Int):Int {if(length==0)return 0;if(remaining<=0)return -1;val n=`in`.read(data,offset,minOf(length.toLong(),remaining).toInt());if(n>0)remaining-=n;return n}
                override fun skip(count:Long):Long {val n=`in`.skip(minOf(count,remaining));remaining-=n;return n}
                override fun close(){try{super.close()}finally{remote?.disconnect();streams.remove(this)}}
            }
            streams.add(bounded)
            android.webkit.WebResourceResponse(source.mime,null,if(raw==null)200 else 206,if(raw==null)"OK" else "Partial Content",headers,bounded)
        } catch(error:Exception) {
            android.util.Log.w("MediaManagerPlayback","Could not open preview stream",error)
            empty(503,"Media Unavailable")
        }
    }
}

/** Full requested HTTP byte ranges; response bodies stream without buffering the whole range. */
internal fun previewRange(value:String?,size:Long):Pair<Long,Long> {
    require(size>0)
    if(value==null)return 0L to size-1
    require(value.startsWith("bytes=") && !value.contains(','))
    val parts=value.removePrefix("bytes=").split('-',limit=2);require(parts.size==2)
    if(parts[0].isEmpty()){val count=parts[1].toLong().coerceAtMost(size);require(count>0);return size-count to size-1}
    val start=parts[0].toLong();val end=if(parts[1].isEmpty())size-1 else parts[1].toLong().coerceAtMost(size-1)
    require(start>=0 && start<size && end>=start);return start to end
}
/** libVLC decoding stays out of the WebView. The loopback gateway preserves pinned TLS upstream. */
private class CompatiblePreview(
    private val activity: MainActivity,
    private val token: String,
    private val response: (android.webkit.WebResourceRequest) -> android.webkit.WebResourceResponse
) {
    private val dialog = android.app.Dialog(activity)
    private val server = java.net.ServerSocket(0, 4, java.net.InetAddress.getByName("127.0.0.1"))
    private val workers = java.util.concurrent.ThreadPoolExecutor(2,2,0L,java.util.concurrent.TimeUnit.SECONDS,java.util.concurrent.ArrayBlockingQueue<Runnable>(4))
    private val sockets = java.util.Collections.synchronizedSet(mutableSetOf<java.net.Socket>())
    private val handler = Handler(Looper.getMainLooper())
    private var lib: org.videolan.libvlc.LibVLC? = null
    private var player: org.videolan.libvlc.MediaPlayer? = null
    @Volatile private var closed = false
    fun show() {
        try {
            val engine=org.videolan.libvlc.LibVLC(activity,arrayListOf("--no-video-title-show","--network-caching=1000"));lib=engine
            val mediaPlayer=org.videolan.libvlc.MediaPlayer(engine);player=mediaPlayer
            val column=LinearLayout(activity).apply {orientation=LinearLayout.VERTICAL;setBackgroundColor(Color.BLACK)}
            val status=TextView(activity).apply {text="Preparing compatibility player…";setTextColor(Color.WHITE);setPadding(16,16,16,16)}
            val video=org.videolan.libvlc.util.VLCVideoLayout(activity)
            column.addView(video,LinearLayout.LayoutParams(-1,0,1f));column.addView(status)
            val seek=android.widget.SeekBar(activity).apply {max=1000;contentDescription="Playback position"}
            column.addView(seek)
            val controls=LinearLayout(activity).apply {gravity=Gravity.CENTER}
            val pause=Button(activity).apply {text="Pause";setOnClickListener {if(mediaPlayer.isPlaying){mediaPlayer.pause();text="Play"}else{mediaPlayer.play();text="Pause"}}}
            controls.addView(pause)
            controls.addView(Button(activity).apply {text="Close";setOnClickListener {close()}})
            column.addView(controls)
            dialog.setContentView(column);dialog.setOnDismissListener {close()}
            dialog.show();dialog.window?.setLayout(-1,-1)
            dialog.window?.addFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            mediaPlayer.attachViews(video,null,true,false)
            mediaPlayer.setEventListener {event ->
                if(BuildConfig.DEBUG && activity.intent.getBooleanExtra("media_manager_preview_fixture",false)) {
                    val state=when(event.type){
                        org.videolan.libvlc.MediaPlayer.Event.Playing -> "playing"
                        org.videolan.libvlc.MediaPlayer.Event.EndReached -> "ended"
                        org.videolan.libvlc.MediaPlayer.Event.EncounteredError -> "error"
                        org.videolan.libvlc.MediaPlayer.Event.Vout -> if(event.voutCount>0)"video-output" else ""
                        else -> ""
                    }
                    if(state.isNotEmpty())java.io.File(activity.filesDir,"compatibility-fixture-events.txt").appendText("$state\n")
                }
                when(event.type) {
                    org.videolan.libvlc.MediaPlayer.Event.Playing -> {status.text="Compatibility player";pause.text="Pause"}
                    org.videolan.libvlc.MediaPlayer.Event.EncounteredError -> status.text="Playback failed. The file may be damaged, protected, or the connection interrupted."
                    org.videolan.libvlc.MediaPlayer.Event.EndReached -> {status.text="Playback finished";pause.text="Play"}
                    org.videolan.libvlc.MediaPlayer.Event.Buffering -> status.text="Buffering ${event.buffering.toInt()}%"
                }
            }
            seek.setOnSeekBarChangeListener(object:android.widget.SeekBar.OnSeekBarChangeListener {
                override fun onProgressChanged(bar:android.widget.SeekBar,progress:Int,user:Boolean){if(user && mediaPlayer.isSeekable)mediaPlayer.position=progress/1000f}
                override fun onStartTrackingTouch(bar:android.widget.SeekBar){}
                override fun onStopTrackingTouch(bar:android.widget.SeekBar){}
            })
            val tick=object:Runnable {override fun run(){if(!closed){seek.progress=(mediaPlayer.position*1000).toInt().coerceIn(0,1000);handler.postDelayed(this,500)}}};handler.post(tick)
            Thread({while(!closed){try{val socket=server.accept();sockets.add(socket);try{workers.execute {serve(socket)}}catch(_:java.util.concurrent.RejectedExecutionException){sockets.remove(socket);socket.close()}}catch(_:java.io.IOException){break}}},"media-preview-gateway").start()
            val media=org.videolan.libvlc.Media(engine,android.net.Uri.parse("http://127.0.0.1:${server.localPort}/$token"))
            media.setHWDecoderEnabled(true,false);mediaPlayer.media=media;media.release();mediaPlayer.play()
        }catch(error:Exception){android.util.Log.w("MediaManagerPlayback","Compatibility player initialization failed",error);close();android.widget.Toast.makeText(activity,"Could not start compatibility player",android.widget.Toast.LENGTH_LONG).show()}
    }
    fun pause(){player?.pause()}
    fun close(){
        if(closed)return;closed=true
        handler.removeCallbacksAndMessages(null)
        runCatching {server.close()};synchronized(sockets){sockets.toList().forEach{runCatching{it.close()}};sockets.clear()}
        workers.shutdownNow()
        val oldPlayer=player;val oldEngine=lib;player=null;lib=null
        oldPlayer?.setEventListener(null);oldPlayer?.detachViews()
        // Native stop may wait for decoder/network teardown; never wait on the UI thread.
        Thread({runCatching {oldPlayer?.stop()};runCatching {oldPlayer?.release()};runCatching {oldEngine?.release()}},"media-preview-release").start()
        dialog.dismiss()
    }
    private fun serve(socket:java.net.Socket){
        try{socket.use {
            socket.soTimeout=10000
            val input=socket.getInputStream();var budget=8192
            fun line():String {val data=java.io.ByteArrayOutputStream();while(true){require(--budget>=0);val c=input.read();require(c>=0);if(c==10)break;if(c!=13)data.write(c)};return data.toString("US-ASCII")}
            val first=line().split(' ');require(first.size==3 && first[0] in listOf("GET","HEAD") && first[1]=="/$token")
            val headers=mutableMapOf<String,String>();while(true){val h=line();if(h.isEmpty())break;val colon=h.indexOf(':');require(colon>0);headers[h.substring(0,colon).trim()]=h.substring(colon+1).trim()}
            val request=object:android.webkit.WebResourceRequest {
                override fun getUrl()=android.net.Uri.parse("https://dioxus.index.html/native-media-preview/$token")
                override fun isForMainFrame()=false
                override fun isRedirect()=false
                override fun hasGesture()=false
                override fun getMethod()=first[0]
                override fun getRequestHeaders()=headers
            }
            val result=response(request)
            result.data.use {body ->
                val out=socket.getOutputStream()
                val head=StringBuilder("HTTP/1.1 ${result.statusCode} ${result.reasonPhrase}\r\nConnection: close\r\nContent-Type: ${result.mimeType}\r\n")
                result.responseHeaders?.forEach{(k,v)->head.append("$k: $v\r\n")};head.append("\r\n")
                out.write(head.toString().toByteArray(Charsets.US_ASCII))
                if(first[0]=="GET")body?.copyTo(out,64*1024)
                out.flush()
            }
        }}catch(_:Exception){runCatching{socket.close()}}finally{sockets.remove(socket)}
    }
}
