# Media Manager v0.1.2 preview

## Changes

- Revised three-column dashboard: library, naming and API settings on the left;
  posters on the right; preview and rename actions in the center.
- Back/Next navigation in single-column windows. Successful previews open posters
  automatically. Removed the settings gear and menu.
- Windows GUI builds no longer create a console window; packaging rejects console builds.
- Fixed the macOS splash transition that could leave resizing disabled.
- Added Android launcher icons, iOS app icons and a macOS bundle icon.
- Added Android document-tree access and iOS security-scoped folder access for
  local previews, renaming, poster writes and playback. These new integrations
  still require real-device/provider acceptance testing.
- Improved mobile ad bridge readiness, compact layouts, consent error recovery,
  and failure reporting with a manual Retry ad button. Real ad delivery remains
  subject to SDK, network, consent and AdMob account configuration.
- Apache 2.0 license; README now includes the app logo and dashboard screenshot.

## Downloads

Platform jobs attach assets as they finish:

- Windows x64 portable ZIP with app and optional server.
- macOS Apple Silicon and Intel ZIPs with app bundle and optional server.
- Linux x64 TAR.GZ with app and optional server, built on Ubuntu 24.04.
- Android ARM64 debug-signed preview APK with test ads.
- iOS ARM64 **Simulator-only** app ZIP with test ads, for Xcode on a Mac.

Verify downloads against the accompanying SHA-256 files. A missing asset means
its job is pending or failed; inspect Actions for build status.

## Preview limitations

Native mobile storage and ad changes require device testing. Background execution
and mobile notifications remain unfinished. Android is development-signed, not a
production Play release. iOS is not an iPhone IPA or App Store/TestFlight build.
macOS bundles are ad-hoc signed and not notarized. Windows EXEs are unsigned.
Linux requires a compatible GTK/WebKitGTK runtime.

Renames affect real files, can partially succeed, and have no automatic undo.
Review destinations before applying. Original project code is Apache 2.0 licensed;
third-party dependencies, SDKs and provider data retain their own terms.
