# Media Manager v0.1.4 preview

## Changes

- Added title-match selection, optional premiere-year search, and alternate TVMaze/TMDB posters.
- Added anime absolute numbering, detected episode numbers, and explicit episode mappings.
- Rename companion subtitles, audio and NFO files alongside media.
- Persistent rename history and explicit Undo for completed operations.
- Preview health scans, including SHA-256 exact duplicate detection.
- Saved naming profiles and single-use QR pairing with the remote server.
- Working splash progress and None/Plex/Jellyfin naming selection.
- Fixed iOS Simulator installation by packaging static Google SDK resources without embedding static archives as executable frameworks.
- Added automated Android/iOS install-and-launch validation and a local Android emulator launcher.

## Downloads

Native release jobs attach platform assets and SHA-256 checksums as they finish:

- Windows x64 portable ZIP with app and optional server.
- macOS Apple Silicon and Intel ZIPs with app bundle and optional server.
- Linux x64 TAR.GZ and Flatpak using a managed GTK/WebKitGTK runtime.
- Android ARM64 debug-signed preview APK with test ads.
- iOS ARM64 **Simulator-only** app ZIP, for Xcode on a Mac. This is not an iPhone IPA.

## Validation and limits

Rust tests and native bridge/packaging checks passed. Android and iOS installed,
launched and survived the startup check in hosted emulators/simulators. Android
also installed and launched in a local Windows Android 15 emulator with WHPX.
The iOS screenshot showed a test ad with no native-ad validator issues. Android
ad delivery remains unresolved (SDK/advertising privacy-check errors observed).
These startup checks do not replace full feature or physical-device acceptance.

Mobile background execution and notifications remain unfinished. Android previews
are debug-signed. macOS bundles are ad-hoc signed and not notarized. Publicly
trusted Windows signing requires configured publisher credentials; self-signed
internal builds do not establish public trust. Linux Flatpak uses GNOME 49;
the TAR.GZ requires compatible host libraries.

Renames affect real files and can partially succeed. Review destinations before
applying; use recorded history for explicit Undo and inspect its results.
Original code is Apache 2.0 licensed. Third-party dependencies, SDKs and provider
data retain their own terms. See README and docs/MOBILE_VALIDATION.md.
