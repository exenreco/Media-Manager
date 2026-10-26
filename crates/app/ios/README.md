# iOS Native advanced advertising

Configured IDs:

- App: `ca-app-pub-9696662815161119~9310033241`
- Release native unit: `ca-app-pub-9696662815161119/9079018827`
- Debug unit: `ca-app-pub-3940256099942544/3986624511` (Google test ads)

The Rust settings panel shows the same 440-point reserved ad area above Build
rename preview on Android and iOS, separated from the button by 24 CSS pixels.
On iOS, `manganis::ffi` registers the `MediaManagerAds` Swift package with
Dioxus CLI 0.7.10. The plugin attaches a main-frame-only WebKit message handler
to the app's existing WKWebView. It accepts geometry/privacy commands only
from the app's `dioxus://index.html` document. It does not replace Dioxus's
navigation delegate or pass Google ad assets through JavaScript.

Swift renders an SDK NativeAdView containing attribution, supplied icon,
headline, advertiser, body, MediaView and call-to-action. Google manages
AdChoices, clicks and impressions. The overlay hides when the reserved area
is clipped, a dialog is open, a native controller is presented, or the app is
inactive. Ads are discarded after an hour and when the component detaches.
Consent/no-fill failures collapse the ad area without blocking rename preview.

The package pins Google Mobile Ads **13.11.0** and UMP **3.1.0**. Dioxus config
injects the app ID and 50 SKAdNetwork identifiers from Google's iOS setup guide
(retrieved 2026-10-25). `Info.plist` is the matching advertising-only fragment,
not a replacement for the app's complete generated plist.

## Build on a Mac

This checkout is on Windows. GitHub Actions successfully compiled Rust/Swift,
embedded the Google SDKs, ad-hoc signed the simulator bundle and verified its
signatures for the v0.1.1 prerelease. **Simulator UI/device acceptance testing
and production/device signing remain pending.** Rust desktop checks alone
cannot validate the native mobile integration.

Install Xcode, its iOS SDK, the required Rust iOS target and Dioxus CLI 0.7.10.
Use iOS 15 or later. From `crates/app`, build with:

```sh
dx build --platform ios --no-default-features --features mobile
```

Dioxus builds the Swift package and installs `DioxusSwiftPlugins.framework`.
Its packaging code does not explicitly copy SwiftPM binary SDK dependencies.
After building, run the supplied finishing script with the actual output
paths from that build. The SwiftPM artifacts directory is normally beneath
`target/swift-build/swift-plugins/mediamanagerads/.build/artifacts`:

```sh
python3 ios/embed_ad_frameworks.py \
  --app /absolute/path/to/MediaManager.app \
  --artifacts /absolute/path/to/mediamanagerads/.build/artifacts \
  --sdk iphonesimulator --arch arm64 --identity -
```

For a physical device, use `--sdk iphoneos --arch arm64` and replace `-` with
your valid Apple signing identity. Build with the correct team/provisioning
profile first. The script selects the matching XCFramework slices, embeds
GoogleMobileAds and UserMessagingPlatform, configures the plugin's framework
search path, merges advertising plist values, signs frameworks, re-signs the
app while preserving entitlements, and verifies signatures. It fails if the
Swift plugin or either SDK is missing. Run it **before** installing/exporting
the finished bundle. Do not use an earlier IPA produced before this step.

Release builds use the publisher unit; debug builds use Google's test unit.
Do not click production ads while testing.

## Publisher setup and acceptance checks

- Configure this iOS app's Privacy & messaging in AdMob. UMP updates consent
  before SDK initialization and shows privacy options when required.
- Publish a privacy policy and complete App Store privacy/advertising
  disclosures, app linking/readiness review and app-ads.txt setup as applicable.
- This integration does not request App Tracking Transparency permission or
  implement cross-app tracking. If tracking is added, implement the applicable
  ATT flow and usage description before requesting tracking authorization.
- Configure child/under-age request treatment before SDK initialization if
  applicable; the app does not infer age.
- On simulator and device, check test-ad identification, native assets and
  video, AdChoices, consent and reopened privacy options, offline/no-fill,
  panel scrolling/resizing, dialogs, rotation, background/resume and font sizes.
- Confirm the signed bundle contains the SDK frameworks and their privacy
  manifests, the correct app ID/SKAdNetwork list, and the correct ad-unit mode.
  Source implementation is not Google or App Store approval.

Shared bridge tests: `node --test android/tests/native-ad.test.cjs`.
Slice-selection tests: `python3 -m unittest discover -s ios/tests`.

References: [SDK setup](https://developers.google.com/admob/ios/quick-start),
[native loading](https://developers.google.com/admob/ios/native),
[native display](https://developers.google.com/admob/ios/native/advanced),
[UMP consent](https://developers.google.com/admob/ios/privacy),
[AdMob policies](https://support.google.com/admob/answer/6128543).

## Local device libraries

MediaManagerStorage.swift provides folder selection through Files, stores folder
bookmarks, and balances security-scoped access during operations. Scans and
renames use file coordination off the UI thread. Quick Look handles playback.
The Rust storage bridge attaches independently of advertising consent. Native
build and device validation of this new bridge is still pending.

The ad slot adapts between 380 and 440 CSS pixels, with optional text hidden in
compact layouts. Loading eligibility is separate from full overlay visibility.
The bridge waits briefly for WKWebView readiness; errors show a status/code and
a manual Retry ad button. UMP must authorize every request, including retries.
SDK load failures are logged under MediaManagerAds.
