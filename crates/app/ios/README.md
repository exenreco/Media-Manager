# iOS Native advanced advertising

Configured IDs:

- App: `ca-app-pub-9696662815161119~9310033241`
- Release native unit: `ca-app-pub-9696662815161119/9079018827`
- Debug unit: `ca-app-pub-3940256099942544/3986624511` (Google test ads)

The Rust settings panel places the native ad area directly below the TV Shows/Movies
toggle on Android and iOS, with 24 CSS pixels before Connect your library.
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

The current source compiled on macOS CI and passed
installation/launch in an iPhone Simulator after correcting platform metadata and
static SDK packaging. The captured test ad's SDK validator reported no implementation
issues. Full feature acceptance on physical devices and production/device signing
remain pending. See [mobile validation](../../../docs/MOBILE_VALIDATION.md).

Install Xcode, its iOS SDK, the required Rust iOS target and Dioxus CLI 0.7.10.
Use iOS 15 or later. From `crates/app`, build with:

```sh
dx build --platform ios --target aarch64-apple-ios-sim --no-default-features --features mobile
```

Dioxus builds the Swift package and installs `DioxusSwiftPlugins.framework`.
The Google SDK static archives are linked into that plugin; they must not be
embedded as executable frameworks. Their privacy/resources still need packaging.
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
profile first, targeting `aarch64-apple-ios`. The script selects matching XCFramework
slices, preserves static SDK privacy/resources as resource bundles, embeds any dynamic
SDK frameworks, configures the plugin's framework search path, corrects platform
metadata, merges advertising plist values, signs dynamic frameworks, re-signs the
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
- Confirm the signed bundle contains the SDK code in the Swift plugin and its
  privacy/resource bundles (or dynamically linked frameworks where applicable), the correct app ID/SKAdNetwork list, and the correct ad-unit mode.
  Source implementation is not Google or App Store approval.

Shared bridge tests: `node --test android/tests/native-ad.test.cjs`.
Slice-selection tests: `python3 -m unittest discover -s ios/tests`.

## One-time ad removal

Settings → Advertising includes **Remove ads** and **Restore purchases** on iOS.
The native StoreKit 2 integration uses a non-consumable product with the ID
`remove_ads`, configured by `MediaManagerRemoveAdsProductID` in the `[ios.plist]`
section of `Dioxus.toml`. If App Store Connect uses a different product ID, change
that value before building. Create the matching non-consumable under the
`com.tvshowmanager.app` app in App Store Connect, configure its localization and
price, and complete Apple's availability and review requirements. The app shows
StoreKit's localized price; it does not hard-code an amount or offer a subscription.

Only StoreKit-verified, unrevoked transactions for that product grant ad removal.
Ownership is read from StoreKit on startup and foregrounding, and transaction
updates handle purchases completed elsewhere and revocations. Native, rewarded
and interstitial ads are suppressed for owners. Pending payments do not grant
ownership. Web content cannot grant ownership, and no purchase receipts or tokens
are passed into JavaScript. Restore explicitly calls `AppStore.sync()` and may
show Apple's account UI. It is enabled only for a verified active purchase and
for ten minutes after its StoreKit purchase date. The native layer enforces this
window as well as the UI; automatic entitlement checks continue after it expires. Android purchases are separate
from App Store purchases.

A standalone simulator install without a configured StoreKit test session may
not return the product. The controls remain visible, with purchasing disabled and
an availability message. Use an App Store sandbox/TestFlight build with the
configured product to validate pricing, cancellation, pending approval, completion,
restoring on another device with the same Apple Account, offline relaunch, and
refund/revocation. Check that no ad placement or fullscreen ad is shown after a
verified purchase. Purchase-flow acceptance remains pending until these native
store scenarios are exercised; bridge tests do not substitute for them.

Purchase UI/bridge tests:
`node --test tests/billing.test.cjs android/tests/native-ad.test.cjs`.

### App Store Connect configuration (2026-11-02)

- Registered explicit bundle ID: `com.tvshowmanager.app`, with In-App Purchase enabled.
- App record: **Media Manager by Exenreco**, Apple ID `6829643613`, iOS,
  SKU `media-manager-ios`, primary language English (U.S.). The shorter
  **Media Manager** name was unavailable in App Store Connect.
- Product: **Remove ads**, ID `remove_ads`, non-consumable, Apple ID `6829644288`.
- Base price: **USD 5.99** in the United States, with Apple's automatically
  adjusted regional prices and availability in all 175 current storefronts.
- English display name: **Remove ads**. Description:
  **Permanently remove all ads. No subscription.** Review navigation notes are saved.
- Status: **Prepare for Submission**; no app or purchase has been submitted for review.
  The iOS Settings review screenshot is uploaded and saved. See the release
  preparation record below for the later signing, upload and listing work.
  Paid Applications agreement/banking/tax status and sandbox purchase acceptance
  still require verification.

[Purchase configuration](https://appstoreconnect.apple.com/apps/6829643613/distribution/iaps/6829644288).

References: [SDK setup](https://developers.google.com/admob/ios/quick-start),
[native loading](https://developers.google.com/admob/ios/native),
[native display](https://developers.google.com/admob/ios/native/advanced),
[UMP consent](https://developers.google.com/admob/ios/privacy),
[AdMob policies](https://support.google.com/admob/answer/6128543).

## Local device libraries

MediaManagerStorage.swift provides folder selection through Files, stores folder
bookmarks, and balances security-scoped access during operations. Scans and
renames use file coordination off the UI thread. Quick Look handles playback.
The Rust storage bridge attaches independently of advertising consent. The bridge compiled successfully in native iOS CI for v0.1.3. Device/provider
validation is still pending.

The ad slot adapts between 380 and 440 CSS pixels, with optional text hidden in
compact layouts. Loading eligibility is separate from full overlay visibility.
The bridge waits briefly for WKWebView readiness; errors show a status/code and
a manual Retry ad button. UMP must authorize every request, including retries.
SDK load failures are logged under MediaManagerAds.

A second mobile ad placement appears below the poster panel, above its navigation.
One native SDK ad view follows the visible placement; the two placeholders do not
duplicate the same ad view or create extra requests when switching panels.


### Fullscreen ads

Interstitials are offered only immediately after a completed rename batch with no failures and at least one renamed file. The native layer requires two minutes in the current session and ten minutes between interstitial impressions. Unavailable ads are skipped; no delayed presentation is queued. Background apps, open dialogs, busy operations and playing videos block UI requests.

Settings offers an optional rewarded ad for 30 minutes without native or fullscreen ads on this device. Only the SDK earned-reward callback saves the expiration in native preferences; closing early gives no reward. The benefit survives app restarts. The SDK controls ad dismissal. Privacy choices gate all requests; changing them discards cached fullscreen ads. Cached ads expire after one hour. Debug builds use Google's test IDs; release builds use the configured production IDs.

Rewarded-ad status retains consent/load errors and disables repeated taps while loading. A first tap may load an ad; tap again after the ready message to watch it. Run `node --test tests/fullscreen-ad.test.cjs` for loading, retry, reward expiry and UI blocking coverage.

On this Mac (2026-11-02), `fundingchoicesmessages.google.com` resolved to `0.0.0.0`/`::`, causing UMP connection-refused errors. Permit the consent service in the network filter or use an unfiltered connection to test ads. Do not bypass consent or grant rewards without the SDK callback.

### Local release preparation (2026-11-02)

Version 1.0.2 built for physical iPhones with Dioxus 0.7.10 and Xcode 27.2
(`aarch64-apple-ios`). Apple Distribution signing for team `PBA93F33RM` and the
**Media Manager App Store** provisioning profile are installed locally. Private
keys remain in the Mac Keychain and are not stored in this repository.

`SceneDelegate.swift` adopts the single Dioxus/Tao window into `UIWindowScene`
and forwards active/background transitions to Tao. `main` loads the Swift framework
before UIKit looks up the scene delegate. On bridge attachment, the window joins
the connected scene and the WebView uses flexible width/height without flexible
margins. The iPhone and iPad simulator launches use the actual SDK 27.2 metadata;
the old simulator-only SDK version workaround is no longer required.

Native iOS and Android always use the footer navigation and single-panel
workspace, including tablet and landscape widths. Their render tree omits the
desktop sidebar and minimize/maximize/close controls. Desktop and web keep their
responsive navigation. Short landscape mobile windows reserve space around
display cutouts even when the embedded webview reports zero safe-area insets.
This navigation change is local and is not included in the already submitted
1.0.2 (1.0.3) build.

The finishing script supplies APPL bundle metadata, an iOS 14+ dictionary launch
screen, the app name, the app's required-API privacy manifest, and matching iOS 15
minimum-version metadata for the Swift framework. It can repackage copied SDK
resources that were originally read-only. SDK privacy manifests remain bundled.

The signed archive passed Xcode's App Store Connect validation and version
**1.0.2 (1.0.3)** was uploaded successfully. A signed IPA and SHA-256 checksum are
saved locally in `dist/ios/`. The processed build was attached to version 1.0.2;
no App Review approval is implied.

App Store Connect now has saved English listing metadata and reviewer contact
information, content rights confirmed by the owner, and a 4+ age rating with
advertising declared (not Made for Kids). Privacy responses covering the seven
Google SDK data categories are published with owner approval. The public privacy
policy URL is configured. Free download pricing was configured; France was
subsequently excluded
with owner approval, leaving 174 storefronts. Standard third-party cryptography
was declared in Apple’s questionnaire.
The account holder completed banking and U.S. W-9 information. The W-9 is
**Active**; banking and the Paid Apps Agreement are **Processing** as of
2026-11-02. The non-consumable `remove_ads` remains USD 5.99 and was added
to the same iOS submission as version 1.0.2. Both items are **Waiting for Review**.
Actual simulator
screenshots are uploaded for 6.9-inch iPhone (1320×2868) and 13-inch iPad
(2064×2752), with smaller sizes inheriting assets where supported.

App Store sandbox purchase, restore and reward-completion acceptance still require
native store/ad testing. The local DNS filter still blocks Google's consent host.
Apple must finish processing the banking and agreement information before
monetization can be activated. Re-enabling France requires revisiting Apple’s encryption
documentation requirements.

The original build was blocked at Add for Review with a beta-toolchain message,
although Apple's public release listing identifies the installed Xcode 27.2
(27B52) as a release. The packaging script now records the actual selected Xcode,
SDK build and host OS build identifiers, which Dioxus's generated plist omitted.
A replacement archive uses bundle build 1.0.3 with marketing version 1.0.2 and
was uploaded successfully. Apple accepted this replacement for review on
2026-11-02 at 10:52 AM, together with `remove_ads`; submission ID
`cf389c37-3e0d-46c4-b374-221e718652fe`. Both items are **Waiting for Review**.
The replacement signed IPA and checksum are in
`dist/ios/Media-Manager-1.0.2-build-1.0.3.ipa` and `.ipa.sha256`.
This metadata correction does not change or lower the executable's SDK version.

### Release 1.0.3 (1.0.4)

The updated physical-device release was signed and uploaded successfully on
2026-11-02. It includes the interactive walkthrough and footer-only native mobile
navigation. The 1.0.3 draft uses current 6.9-inch iPhone and 13-inch iPad simulator
screenshots. The previous 1.0.2 submission was subsequently rejected under
Guideline 2.1 with a request for more review information and a physical-device
recording. Detailed review notes are saved, but physical-device QA/recording is
still required before resubmission. See
[Apple release 1.0.3](../../../docs/APPLE_RELEASE_1.0.3.md).
