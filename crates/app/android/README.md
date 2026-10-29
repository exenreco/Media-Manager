# Android native advertising

This integrates the **Native advanced** Android unit (not a banner unit):

- App ID: `ca-app-pub-9696662815161119~1695352827`
- Release native unit: `ca-app-pub-9696662815161119/4253855527`
- Debug native unit: `ca-app-pub-3940256099942544/2247696110` (Google test ads)

`Dioxus.toml` registers this custom `MainActivity.kt` and manifest and adds
Google Mobile Ads 25.5.0 and UMP 4.0.0 to the generated Gradle project.
Use **Dioxus CLI 0.7.10**, whose Android build supports these configuration
fields. The activity package must remain `dev.dioxus.main` for Dioxus JNI;
the BuildConfig alias uses the existing bundle ID `com.tvshowmanager.app`.

Google Ads 25.5.0 contains Kotlin 2.3 metadata, while Dioxus 0.7.10 generates
a Kotlin 2.0.20 Gradle plugin dependency. Before building, copy the supplied
`kotlin-toolchain.init.gradle` to
`$GRADLE_USER_HOME/init.d/media-manager-kotlin.gradle` (or
`~/.gradle/init.d/media-manager-kotlin.gradle` when GRADLE_USER_HOME is unset).
It selects Kotlin Gradle plugin 2.3.20 and migrates the generated JVM-target
configuration to compilerOptions without disabling metadata checks.
The release workflow applies this compiler configuration automatically.
Set `MEDIA_MANAGER_ANDROID_ICONS` to the absolute path of
`assets/branding/icons/android` before building. The same Gradle init script
registers these launcher resources; release CI sets the path automatically.

The Rust settings panel places the mobile native ad slot directly below the
TV Shows/Movies toggle, with 24 CSS pixels before Connect your library. An
origin-restricted AndroidX WebKit message listener accepts geometry and
privacy commands only from the app's main `https://dioxus.index.html` frame.
Google assets are rendered in Android `NativeAdView`, not HTML. The SDK owns
impressions and clicks. No custom click listeners are attached to ad assets.
The layout includes attribution, headline, supplied icon, advertiser, body,
MediaView and call-to-action; AdChoices is added by Google at top right.

The native view is visible only when the whole reserved area fits inside the
viewport and the scrolling settings panel and no app dialog is open. It hides
on pause. An ad is discarded after one hour and on activity destruction.
No automatic ad-request retry or refresh loop runs. Consent/ad failures collapse
the slot and show an error status and a user-operated Retry ad button without
disabling library actions. A valid UMP canRequestAds result may authorize ads
after a consent-update error, following Google's guidance. Unsupported WebView message APIs
disable the ad integration. UMP privacy options remain available when required.

## Build and validate

Install JDK 17, Android SDK/NDK, an Android Rust target and Dioxus CLI 0.7.10.
From `crates/app`, with the Android toolchain environment configured:

```sh
dx build --platform android --no-default-features --features mobile
```

Use a debug build on an emulator/device first. Confirm Google's test-ad label,
ad assets and video playback, AdChoices, consent forms and privacy-options
reopening. Verify scroll/clipping, landscape/rotation, font scaling, panel
resizing, dialogs, background/resume, no-fill and offline behavior. Verify
preview generation remains available when ads are unavailable. For production,
build/sign a release and confirm its merged manifest and ad-unit selection.
Do not click live ads for testing.

**Validation status:** GitHub Actions successfully compiled the Rust/Kotlin
app and packaged the ARM64 debug-signed APK for the v0.1.1 prerelease. This
Windows checkout still has no Android toolchain or device configured. Device
acceptance tests, production signing and store distribution remain pending.

## Required publisher setup before publishing

- Configure Privacy & messaging for this app in AdMob so UMP can display the
  applicable consent messages. Each activity requests fresh consent status
  before its first ad request and fails closed on a consent error.
- Supply/publish the app privacy policy and complete store advertising and
  data-safety disclosures, app linking/review and app-ads.txt setup as applicable.
- Review the intended audience and configure child/under-age request treatment
  if applicable before initializing the SDK. This code does not infer user ages.
- Review native-ad placement and text/media rendering on real devices before
  enabling production distribution; source implementation is not policy approval.

These Android IDs are not used on iOS or desktop. The separate
[iOS integration](../ios/README.md) has its own supplied IDs and Swift bridge;
desktop advertising still needs a suitable provider.

References: [SDK setup](https://developers.google.com/admob/android/quick-start),
[native loading](https://developers.google.com/admob/android/native),
[native display](https://developers.google.com/admob/android/native/advanced),
[UMP](https://developers.google.com/admob/android/privacy),
[native policies](https://support.google.com/admob/answer/6329638),
[AdMob policies](https://support.google.com/admob/answer/6128543).

## Local device libraries

MainActivity also hosts an origin-restricted MediaManagerStorage bridge. It uses
ACTION_OPEN_DOCUMENT_TREE with persisted read/write grants, queries document
identifiers through DocumentsContract, and renames through the provider. It
checks identities, sizes, timestamps and destination conflicts before mutations.
No broad storage permission is requested. Device/provider validation is pending.

The slot adapts between 380 and 440 CSS pixels. Eligible on-screen slots may
load before they fit completely, but the native overlay stays hidden until the
whole slot is visible. Compact layouts omit optional body/advertiser views and
retain attribution, headline, media, CTA and AdChoices. Bridge readiness is
retried briefly at mount. SDK failures are logged under MediaManagerAds.

Mobile ad slots follow the Details provider ID and the Artwork/Library search
controls. One native SDK ad view follows the visible placement; placeholders do
not duplicate the ad view or create extra requests when switching panels.


### Fullscreen ads

Interstitials are offered only immediately after a completed rename batch with no failures and at least one renamed file. The native layer requires two minutes in the current session and ten minutes between interstitial impressions. Unavailable ads are skipped; no delayed presentation is queued. Background apps, open dialogs, busy operations and playing videos block UI requests.

Settings offers an optional rewarded ad for 30 minutes without native or fullscreen ads on this device. Only the SDK earned-reward callback saves the expiration in native preferences; closing early gives no reward. The benefit survives app restarts. The SDK controls ad dismissal. Privacy choices gate all requests; changing them discards cached fullscreen ads. Cached ads expire after one hour. Debug builds use Google's test IDs; release builds use the configured production IDs.

### One-time ad removal

Android Settings also offers a non-consumable Google Play purchase and restore
action. Signed native receipts control ownership and suppress all ad formats;
pending payments do not unlock it. Purchases remain unavailable until the app's
licensing public key and active Play product are configured. See
[Android billing setup](../../../docs/ANDROID_BILLING.md) for product IDs, build
variables, offline/refund behavior and the required internal-track payment tests.
