# Android and iOS validation

## Current evidence — 2026-10-26

Physical-device acceptance is **not run**: no Android phone or iPhone/Mac test
hardware is available. This Windows workspace now has a local Android SDK and Android 15 Pixel 7 emulator
with WHPX acceleration. Media Manager installed, launched, and remained running;
a startup screenshot was reviewed. The local Android ad panel reported a privacy-check
error. Xcode and iOS Simulator require a Mac.
Both remote runtime launch checks passed in [GitHub Actions](https://github.com/exenreco/Media-Manager/actions/runs/40404535263)
on validation commit `a620d4539e270fff959ab49f488126d2b5c42704` and branch
`codex/mobile-validation-20261026-211815`. Android and iOS apps installed, launched,
and remained running through the startup observation. Screenshots/reports are in
the run artifacts and locally under `.tools/mobile-validation/`.

The iOS screenshot also shows a test ad and Google's native-ad validator reporting
no implementation issues for that displayed ad. The initial successful Android
launch showed a Google SDK internal ad error (code 0); ad delivery is not marked
passed on Android. This is launch evidence, not full feature acceptance.

Completed locally:

- 14 JavaScript bridge tests: ad visibility/lifecycle and native storage response handling.
- 9 iOS packaging tests: framework slice selection, writable signing copies,
  simulator/device metadata, matching Mach-O build platforms, and static SDK resource packaging.
- 4 validation-helper tests: disposable fixtures, no overwrite on repeat runs,
  absent tooling, unauthorized devices, and macOS prerequisites.
- Workflow YAML parses; embedded simulator setup Python compiles.
- A disposable 13-file acceptance library and preflight reports were generated under
  `.tools/mobile-validation/` (ignored by Git).

The v0.1.4 release includes the new library features. Use that version or a
fresh build of the current source for acceptance. Simulator platform metadata is now corrected during framework
packaging and checked against the executable before signing. Static Google SDK
archives are linked into the Swift plugin, not embedded as executable frameworks;
their privacy/resource bundles are retained. This fixes the simulator installation
failure discovered during the first runtime attempt.

## Automated emulator/simulator launch checks

The isolated validation branch was pushed and its emulator/simulator run completed.
Use **Mobile validation
(emulator and simulator)** in GitHub Actions and choose Android, iOS, or both.
Pushing a `codex/mobile-validation-*` branch also triggers it. It builds the selected
workflow revision, not an older release tag, and does not publish a release.

Android uses an x86_64 Android 35 Google APIs emulator. iOS uses an ARM64 iPhone
Simulator on a macOS runner, packages the native ad SDK resources and dynamic frameworks, and ad-hoc signs
that simulator build. This does not create an installable iPhone IPA.

Artifacts include the test app, startup screenshot, launch output, Android app log,
and `result.json`. A passing result means install/launch/process survival for ten
seconds only. It does **not** certify local folder permissions, Rename/Undo,
cloud providers, ad fill/consent, notifications, background work, or physical phones.
Review the screenshot and complete the matrix below on each target device.

To run the helper against an already booted Android emulator or authorized phone:

```sh
python scripts/mobile_smoke.py --platform android --app path/to/current-preview.apk --device DEVICE_SERIAL --evidence target/mobile-validation/android
```

The helper locates adb in PATH. It discovers the package with Android build-tools
`aapt`, or accepts `--package com.tvshowmanager.app`. It uses `adb install -r`, does
not uninstall or clear app data, and stops if the device is absent or unauthorized.
Enable USB debugging and approve the phone's computer authorization yourself.

On a Mac with a booted iOS Simulator:

```sh
python scripts/mobile_smoke.py --platform ios --app path/to/MediaManager.app --device SIMULATOR_UDID --evidence target/mobile-validation/ios
```

Physical iPhones require a device build, Apple signing/provisioning, and device
setup in Xcode. The simulator helper deliberately refuses a device executable.
Review diagnostics before sharing them; a real-device screenshot/log can contain
your library names or paths. The local helper uploads nothing.

## Disposable acceptance library

```sh
python scripts/mobile_fixtures.py path/to/new-test-library
```

The directory must be new or empty. Repeat runs refuse to overwrite anything.
Copy this test folder to Android storage or iOS Files and select it through the
app's native folder picker. Its `.mkv`, `.mka`, and poster sentinel files are inert
rename fixtures, **not playable media or displayable artwork**. Add a separate
short valid MP4 of your own for the playback test. Never use a production library
for initial mutation tests.

A SHA-256 manifest records the original file paths and contents. After Undo,
compare the original files to this manifest; ignore app-created history records
and newly downloaded artwork, which Undo keeps.

## Physical-device acceptance matrix

Record OS/version, device model, build commit, app version, provider, test date,
and pass/fail/evidence for every row. Run on Android 10+ with local Documents and,
where available, an SD card/cloud document provider; run on iOS 15+ with On My
 iPhone and iCloud Drive. Also test a short-screen phone and an iPad/tablet layout.

| Test | Procedure and required result | Android | iOS |
| --- | --- | --- | --- |
| Launch and identity | Launch cold; logo/name remain below status bar/notch. Resize/rotate where supported; Back/Next remain usable. | Not run | Not run |
| Native folder selection | Select the disposable root. Cancel picker once; cancel must return an error without changing the selection. | Not run | Not run |
| Persisted access | Close/relaunch, resume, rebuild preview, and verify the chosen provider folder is still accessible. Revoked access must ask for reselection. | Not run | Not run |
| Manual preview | Use subfolder `Manual`, manual titles `Pilot` and `Second`, season 1, Jellyfin naming, companions on. Preview includes both videos and matching subtitle/audio/NFO files. | Not run | Not run |
| Apply | Apply that preview; videos and companions have consistent basenames, language tags remain, and `s1` becomes `Season 01`. No original file content changes. | Not run | Not run |
| Existing image | Existing poster sentinel bytes remain unchanged when poster downloads are enabled. | Not run | Not run |
| Durable Undo | Relaunch, load history for the same selected folder/subfolder, Undo; original names and contents match the fixture manifest. | Not run | Not run |
| Undo conflict | Rename again; create an original-name collision or modify a renamed file, then Undo. It must refuse to overwrite/restore changed files and retain resumable history. | Not run | Not run |
| Destination conflict | Preview `Conflict` with manual season 1/Pilot. The existing destination blocks Apply, with both original byte sequences preserved. | Not run | Not run |
| Show match | Search `One Piece`, year `1999`; choose Animation/TVMaze. The preview stays bound to the anime ID. Change to 2023; stale selection must clear. | Not run | Not run |
| Anime gaps | Preview `Anime` with One Piece (1999), all seasons, absolute numbering. 001 and 003 map to their provider episodes without consuming 002. Verify the provider order rather than assuming arc numbering. | Not run | Not run |
| Specials/alternate order | Add an explicit relative-path mapping to season 0 or another episode. The mapping wins; unknown/malformed mappings fail safely. | Not run | Not run |
| Artwork | Review alternate TVMaze artwork. With a test TMDB key, verify IMDb-linked matches and explicit season poster assignment, especially differing anime season orders. | Not run | Not run |
| Profiles | Save a naming profile, relaunch, load it, and rebuild. Naming options persist; credentials and library-specific matches are not stored in the profile. | Not run | Not run |
| Health | Preview `Duplicates`; scan finds one exact-content duplicate group. Different bytes are not duplicates. Missing episodes/artwork and unmatched files match the preview. | Not run | Not run |
| QR pairing | Import a generated invitation image, review HTTPS server address, confirm, and preview remote media. Reuse and expired codes must fail. | Not run | Not run |
| Playback | Play the separate valid MP4 locally and through the remote server. Unsupported formats fail visibly without freezing the UI. | Not run | Not run |
| Ads | Use debug test units. Check both placements by switching panels/scrolling, consent/privacy flow, no-fill/retry, hidden/offscreen behavior, and no overlap with dialogs. | Not run | Not run |
| Suspend/resume | Suspend and resume around preview/rename using disposable files. Inspect results/history after interruptions; do not assume OS background execution is guaranteed. | Not run | Not run |
| Offline/provider failures | Disable network for metadata/artwork, revoke folder access, or disconnect the server; show actionable errors and preserve files. | Not run | Not run |

Background execution and native notifications remain separately tracked platform
work; a launch smoke test is not evidence that either is complete. Only mark a
physical row passed after observing it on that platform, with the actual provider.

## Local Windows emulator

The local installation lives in ignored `.tools/android-sdk`, `.tools/jdk`,
`.tools/android-avd`, and `.tools/android-user` directories. Reopen the configured
phone with `scripts/start_android_emulator.ps1 -InstallApp` from PowerShell.
The APK is the validated x86_64 build in `.tools/mobile-validation/android-evidence`.
These machine-local tools and binaries are not included in Git or release archives.

## Camera pairing implementation and current evidence

The current source replaces mobile QR-image import with native camera scanning:
Android uses a dedicated portrait ZXing activity with a square mint guide and
Cancel button; iOS uses AVFoundation with a square guide. Desktop QR generation
supports an embedded certificate-verified local TLS host and configured remote
HTTPS servers. Rust tests cover certificate rejection, one-use redemption and
host shutdown. The normal suite has 48 passing Rust tests; the camera harness is
opt-in and ignored in normal CI.

- Android/iOS native compilation and install/launch passed on commit
  `fcead9b729045245556d0e5f216d7dc7d6163a56` in
  [the shared validation run](https://github.com/exenreco/Media-Manager/actions/runs/40418886642).
- The final Android portrait activity compiled, installed and launched on commit
  `320e3aeeb5640ef42fb95e4b7e06ad2227235c80` in
  [the Android-only validation run](https://github.com/exenreco/Media-Manager/actions/runs/40436164285).
- Windows/macOS/Linux desktop checks passed on the validation branch. The Windows
  UI generated a local TLS QR for a disposable fixture library, with countdown
  and Stop local sharing.
- The final Android APK was installed in a separate local Android 15 Pixel 7 QA
  emulator. The Scan button opened the portrait camera and requested permission.
  The Cancel button returned to the app. With one-time permission and a file-backed
  simulated camera, a valid QR was decoded, the scanner closed, HTTPS redemption
  succeeded, and the remote server URL and masked bearer credential populated
  automatically. This is a passed simulated-camera local pairing test.
- Initial black/cropped feeds were resolved by a relative imagefile path, software
  rendering, and calibrating the file camera crop. These are test-emulator settings;
  the app does not change camera images or system trust stores on real devices.
- Physical Android/iOS camera scans, iOS local-network prompts and a publicly hosted
  remote pairing flow remain pending. Existing user emulator data and real media
  were preserved. No renaming was performed in the camera test.

### Reproduce the simulated-camera host

Use a disposable emulator with the new APK and camera backend
`-camera-back imagefile:.tools/mobile-validation/camera-qa/camera-input.png`
from the repository root (the relative path avoids Windows path-space issues).
The tested emulator used software graphics. Set these PowerShell variables:

```powershell
$env:MEDIA_MANAGER_CAMERA_HOST_IP = 'YOUR_PC_LAN_IP'
$env:MEDIA_MANAGER_CAMERA_IMAGE = Join-Path (Get-Location) '.tools/mobile-validation/camera-qa/camera-input.png'
cargo test -p tv-manager-app camera_pairing_fixture_host -- --ignored --nocapture
```

Create the output directory first. This opt-in helper creates a temporary one-file
library, starts an encrypted host for five minutes, and writes its one-use QR into
the simulated camera image. It never serves the user's library. Open Scan desktop
QR code in the disposable emulator and grant temporary simulated-camera access.
The helper stops automatically after five minutes. Camera framing is calibrated
for the tested Pixel 7/API 35 imagefile backend; other configurations may need a
different synthetic frame. Keep test images and credentials out of Git.


## Workspace redesign validation (2026-10-26)

The working source uses desktop icon navigation and a custom title bar, an inline
video/title inspector, and an expanding bottom tool grid on narrow/mobile views.
Native media reads are bounded, seekable, and preserve certificate validation for
paired remote libraries. Mobile startup gets its settings directory from the
native bridge before loading saved settings.

Validation snapshot: `c098b8ab0f54baafd85150efbb45dbe1c1650203`.

- [Desktop checks](https://github.com/exenreco/Media-Manager/actions/runs/40443050537)
  passed on Windows, macOS and Linux, including 50 Rust tests and strict linting.
- [Native mobile validation](https://github.com/exenreco/Media-Manager/actions/runs/40443050460)
  passed Android and iOS builds, installation and launch. Bridge checks passed
  15 JavaScript tests, including preventing ads from covering the floating tools.
- Windows visual checks passed for automatic sample-video playback, editing a
  proposed title without changing its extension, borderless resizing, and compact
  navigation. Only disposable fixtures were used; no user media was renamed.
- The final Android APK was additionally installed in the local API 35 QA
  emulator. Menu expansion/navigation and saving a naming field to private native
  app storage were checked; restarting the app displayed its resume prompt.
- Physical-device/provider acceptance and iOS interaction/playback acceptance
  remain untested. Video decoding depends on platform WebView codec support.

Earlier validation attempt 40441617157 compiled both mobile apps, but its hosted
Android emulator image download failed and its iOS launch timed out. The final
run above passed both launch checks. These source changes are not in v0.1.4.

## Current desktop design on mobile (2026-10-27)

Validation snapshot: `441a56a1e479f992437bf65177d3b2ecedf91342`.
[Native mobile validation](https://github.com/exenreco/Media-Manager/actions/runs/40585372499)
passed bridge checks, Android and iOS builds, installation, and emulator/simulator
launch. Both platforms use the current shared Rust UI, including Get Started,
Details, Library/Art Work, Health, Settings, and the expanding bottom navigation.

The resulting Android APK was installed and launched in the local API 35 QA
emulator. Interactive checks covered native folder selection, automatic Firefly
(2002) metadata matching and preview, the centered loading spinner, Library,
the ordering popup, responsive artwork grid, download toolbar, and poster
comparison inspector. Local posters appeared in color, missing provider posters
in grayscale with download badges, and replacement required explicit consent.
Only disposable sample files were used; no rename or poster save was applied.
The app was left running in the emulator.

iOS interaction checks and physical-device acceptance remain pending. Its native
build and simulator launch passed on the macOS runner; iOS cannot run locally on
this Windows PC. The Android emulator reported an advertising privacy-check
failure, so this validation does not establish successful ad delivery.


## Compact preview sheets (working source after 1.0.0 beta)

Media and artwork share a rounded black bottom sheet on native mobile apps and
compact desktop windows. Browser fixtures exercising the actual stylesheet and
controller passed phone (390x844), compact desktop (960x700), large desktop
(1200x800), and native-mobile tablet layout checks. Short drag snap-back, downward
dismissal, Escape, focus restoration, background inertness, artwork comparison,
selection preservation across resizing, and reduced-motion transitions were
checked. Native ad bridge tests include hiding advertisements through dismissal.

An isolated Windows build with a disposable sample library confirmed the compact
sheet, inline video, title field, and return to Library. No real media was changed.
App checks passed 23 tests (two manual tests ignored), strict Clippy and formatting;
19 JavaScript bridge tests passed.

Follow-up [native mobile validation](https://github.com/exenreco/Media-Manager/actions/runs/40630717594)
for snapshot `8f96b29206817dac10dcf3adff83772d968c7f3e` rebuilt Android
x86_64 and iOS ARM64 Simulator packages and passed installation/launch checks on
both hosted platforms. Local validation packages are saved in `dist/preview-sheet`
(ignored by Git). Physical-device gesture/feature acceptance remains untested.
The published 1.0.0 beta is unchanged.


### Interstitial and rewarded integration validation

Source snapshot `c8006a33b4f0caebba724959702e214daac9c97a`, mobile run [40637995277](https://github.com/exenreco/Media-Manager/actions/runs/40637995277): Android x86_64 emulator and iOS ARM64 simulator builds, installation and launch checks passed. Debug builds use Google test ad units. These are launch checks, not physical-device acceptance of fullscreen video, earned rewards, early dismissal or privacy forms.

Local validation: 25 JavaScript bridge tests, 23 Rust app tests (2 manual tests ignored), 10 iOS packaging tests, formatting and strict app Clippy passed. Desktop checks passed on Windows, macOS and Linux in run [40637995325](https://github.com/exenreco/Media-Manager/actions/runs/40637995325).

Before production distribution, exercise test interstitial presentation after a successful rename (two-minute initial session delay and ten-minute cooldown), optional rewarded completion and early dismissal, ad-free persistence and expiry, offline/no-fill handling, background/resume, and consent changes on physical devices. No live ad impressions were used for this validation.
