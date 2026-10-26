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
    private var browser: WebView? = null
    private var adView: NativeAdView? = null
    private var nativeAd: NativeAd? = null
    private var consent: ConsentInformation? = null
    private var bounds: JSONObject? = null
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
        WebViewCompat.addWebMessageListener(webView, "MediaManagerAds",
            setOf("https://dioxus.index.html")) { _, message, origin, mainFrame, _ ->
            if (!mainFrame || origin.scheme != "https" || origin.host != "dioxus.index.html")
                return@addWebMessageListener
            val data = runCatching { JSONObject(message.data ?: "") }.getOrNull()
                ?: return@addWebMessageListener
            when (data.optString("type")) {
                "bounds" -> {
                    bounds = data
                    positionAd()
                    if (!started && data.optBoolean("visible") && foreground) {
                        started = true
                        requestConsent()
                    }
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

    private fun requestConsent() {
        val info = UserMessagingPlatform.getConsentInformation(this)
        consent = info
        info.requestConsentInfoUpdate(this, ConsentRequestParameters.Builder().build(), {
            if (dead) return@requestConsentInfoUpdate
            UserMessagingPlatform.loadAndShowConsentFormIfRequired(this) { error ->
                if (dead) return@loadAndShowConsentFormIfRequired
                updatePrivacyButton()
                if (error == null && info.canRequestAds()) initializeAds() else collapse()
            }
        }, {
            // Fail closed on an unavailable consent update; library controls still work.
            updatePrivacyButton()
            collapse()
        })
    }

    private fun updatePrivacyButton() {
        val required = consent?.privacyOptionsRequirementStatus ==
            ConsentInformation.PrivacyOptionsRequirementStatus.REQUIRED
        browser?.evaluateJavascript(
            "document.getElementById('ad-privacy')?.toggleAttribute('hidden', ${!required})", null)
    }

    private fun initializeAds() {
        if (dead || consent?.canRequestAds() != true) return
        if (sdkReady) { loadAd(); return }
        Thread {
            MobileAds.initialize(applicationContext) {
                sdkReady = true
                runOnUiThread { if (!dead) loadAd() }
            }
        }.start()
    }

    private fun loadAd() {
        if (dead || consent?.canRequestAds() != true) return
        val unit = if (BuildConfig.DEBUG) TEST_UNIT else RELEASE_UNIT
        AdLoader.Builder(this, unit)
            .forNativeAd { ad ->
                if (dead || isFinishing || isDestroyed) { ad.destroy() }
                else {
                    discardAd()
                    nativeAd = ad
                    loadedAt = android.os.SystemClock.elapsedRealtime()
                    timer.postDelayed(expireAd, 3_600_000)
                    adView = makeAdView(ad)
                    findViewById<FrameLayout>(android.R.id.content).addView(adView)
                    browser?.evaluateJavascript(
                        "document.getElementById('native-ad-slot')?.removeAttribute('hidden')", null)
                    positionAd()
                }
            }
            .withNativeAdOptions(NativeAdOptions.Builder()
                .setAdChoicesPlacement(NativeAdOptions.ADCHOICES_TOP_RIGHT).build())
            .withAdListener(object : AdListener() {
                override fun onAdFailedToLoad(error: LoadAdError) { collapse() }
            })
            .build().loadAd(AdRequest.Builder().build())
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
        val advertiser = text(ad.advertiser, 40)
        if (ad.advertiser == null) advertiser.visibility = View.GONE
        val body = text(ad.body, 80).apply { maxLines = 4 }
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
        if (width < dp(240) || height < dp(440) || x + width > web.width || y + height > web.height) {
            view.visibility = View.GONE; return
        }
        val parent = findViewById<FrameLayout>(android.R.id.content)
        val webOrigin = IntArray(2); val parentOrigin = IntArray(2)
        web.getLocationInWindow(webOrigin); parent.getLocationInWindow(parentOrigin)
        view.layoutParams = FrameLayout.LayoutParams(width, height).apply {
            leftMargin = webOrigin[0] - parentOrigin[0] + x
            topMargin = webOrigin[1] - parentOrigin[1] + y
        }
        view.visibility = View.VISIBLE
    }

    private fun collapse() {
        adView?.visibility = View.GONE
        browser?.evaluateJavascript(
            "document.getElementById('native-ad-slot')?.setAttribute('hidden', '')", null)
    }

    private fun discardAd() {
        timer.removeCallbacks(expireAd)
        adView?.let { (it.parent as? android.view.ViewGroup)?.removeView(it); it.destroy() }
        adView = null
        nativeAd?.destroy(); nativeAd = null
    }

    override fun onResume() {
        super.onResume(); foreground = true; positionAd()
        if (!started && bounds?.optBoolean("visible") == true) {
            started = true; requestConsent()
        }
    }
    override fun onPause() {
        foreground = false; adView?.visibility = View.GONE; super.onPause()
    }
    override fun onDestroy() {
        dead = true; discardAd(); browser = null; super.onDestroy()
    }
    private fun dp(value: Int) = (value * resources.displayMetrics.density).roundToInt()

    companion object {
        @Volatile private var sdkReady = false
        private const val TEST_UNIT = "ca-app-pub-3940256099942544/2247696110"
        private const val RELEASE_UNIT = "ca-app-pub-9696662815161119/4253855527"
    }
}
