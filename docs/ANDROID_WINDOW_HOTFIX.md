# Android preview and window hotfix — build 1000012

Marketing version **1.0.3**, Android version code **1000012**, source
`8ae3febd371b0aba7826f5ed8bd06384483f42b9` (November 3, 2026).
The tagged GitHub v1.0.3 Android downloads predate this hotfix. Use the artifacts
from the production run below; do not mix APK/AAB files and native symbols from
intermediate runs with the same development build number.

## Changes

- Preview creation failures are handled inside the dispatched UI callback.
  Activity destruction and closed-player callbacks are guarded; controls stop
  accessing disposed players. Teardown detaches listeners and views safely and
  leaves blocking decoder shutdown off the UI thread. Initialization failure
  reports an error instead of escaping the UI callback.
- System-bar, cutout and keyboard insets update all four native safe-edge CSS
  variables on layout/inset changes and page loading. The toolbar, footer, sheets
  and content use these edges without counting space already excluded by the
  native window. Landscape spacing follows actual insets.
- Android 15+ uses target-SDK-36 enforced edge-to-edge. The AndroidX 1.17 helper
  is called only below API 35. The compiled bytecode's API >=35 branch skips the
  legacy status/navigation color calls. Fullscreen native previews use a
  nonfloating theme and apply system/cutout padding to their controls.
- The app and QR scanner are explicitly resizable. Portrait locking and the
  unused library scanner activity are removed. Camera framing adapts to size.
- Android Gradle plugin 8.13.2 supports the current AndroidX API and compile SDK
  36. Dioxus's unused direct Material dependency is removed, eliminating the
  DatePicker implementation reported by Play.

## Validation

[Final phone/tablet validation](https://github.com/exenreco/Media-Manager/actions/runs/41724220299)
passed ARM64 compilation, merged-manifest verification, native receipt-signature
unit tests, Android 15 phone and Android 16 Pixel Tablet installation/launch,
portrait and both landscape safe-area geometry, H.264 inline playback and three
popup open/close cycles through playback completion. The phone also exercised
10-bit H.264 inline playback. No crash/ANR was detected by these synthetic tests.
[Desktop regressions](https://github.com/exenreco/Media-Manager/actions/runs/41724220381)
passed on Windows, Linux and macOS. All 53 JavaScript bridge tests passed.

[Final production build](https://github.com/exenreco/Media-Manager/actions/runs/41724252753)
passed production APK/AAB compilation, native purchase-signature unit tests,
ARM64 structure and 16 KB ELF alignment checks. Download artifact
`android-play-bundle-unsigned`. Locally verified SHA-256 checksums, version code,
package, target SDK 36/min SDK 24, matching APK/AAB native payloads, absence of QA
fixtures and unused Material DatePicker, and the final merged activity manifest.
R8 mapping remains embedded in the AAB. Use the matching native symbols ZIP.

The original device/file crash has not been reproduced. These tests use private
synthetic local media, not the owner's original file or physical device. Retest
that file before broad rollout; device/version details and a crash stack would
be needed for any remaining device-specific failure.

## Remaining Play warning and distribution

The production bytecode still contains Google SDK short-edge cutout handling and
AndroidX compatibility code for older Android versions. Google Ads/Play SDK
compatibility calls cannot be removed by changing this app's window code. The
app retains Google Mobile Ads 25.5.0 and Billing 9.1.0; these are the latest
versions listed in the checked official release notes. Do not claim that every
Play deprecation warning has disappeared until the signed build is rescanned.
See [Android edge-to-edge guidance](https://developer.android.com/develop/ui/views/layout/edge-to-edge),
[Google Ads release notes](https://developers.google.com/admob/android/rel-notes),
and [Billing release notes](https://developer.android.com/google/play/billing/release-notes).

The APK/AAB are **unsigned**, not uploaded to Play, and cannot update an installed
production app yet. Sign privately using the existing Windows upload identity
and the repository's `sign_android_bundle.ps1` / `sign_android_apk.ps1` scripts.
Preserve the existing keystore and DPAPI password file; do not initialize a new
identity for an existing listing. APK signing requires the Android SDK's
`apksigner.jar`. Play App Signing uses its own distribution identity, so a locally
upload-key-signed APK may not replace a Play-installed copy. Use a Play testing
track for update acceptance with the Play distribution identity.
