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

The Rust settings panel reserves an Android-only native ad slot above Build
rename preview, with 24 CSS pixels between the ad and that action. An
origin-restricted AndroidX WebKit message listener accepts geometry and
privacy commands only from the app's main `https://dioxus.index.html` frame.
Google assets are rendered in Android `NativeAdView`, not HTML. The SDK owns
impressions and clicks. No custom click listeners are attached to ad assets.
The layout includes attribution, headline, supplied icon, advertiser, body,
MediaView and call-to-action; AdChoices is added by Google at top right.

The native view is visible only when the whole reserved area fits inside the
viewport and the scrolling settings panel and no app dialog is open. It hides
on pause. An ad is discarded after one hour and on activity destruction.
No automatic retry or refresh loop runs. Consent/ad failures collapse the
slot without disabling library actions. Unsupported WebView message APIs
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

**Validation status:** Rust desktop checks do not compile Kotlin or validate
Android packaging. This checkout has no Android SDK, NDK, Java toolchain or
device configured; an APK build and device acceptance testing remain pending.

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
