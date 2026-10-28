# Media Manager 1.0.0 beta 2

## Changes

- Added animated black preview sheets for media and artwork on phones and compact desktop windows, with swipe dismissal, keyboard handling and focus restoration.
- Added matching mobile sheets for API provider selection, artwork sorting and library ordering.
- Restored the rounded gray bottom navigation capsule while keeping its outer container transparent; removed Android tap-highlight boxes and added spacing above Stop sharing.
- Kept the Library / Art Work tab labels on one line on narrow screens.
- Added Android and iOS interstitial ads after successful rename batches, with a two-minute initial session delay and ten-minute cooldown. Unavailable ads are skipped.
- Added optional rewarded ads in Settings for 30 minutes without ads on that device. Only the SDK earned-reward callback grants the benefit, which survives app restarts.
- Added consent-aware fullscreen ad loading, cache expiry and privacy options. Debug mobile previews use Google test ads.
- Expanded mobile validation to build both Android ARM64 phone and x86_64 emulator APKs.

## Downloads and validation

Platform jobs attach packages and SHA-256 checksums as they complete: Windows x64 setup and portable ZIP; macOS Apple Silicon and Intel PKG and ZIP; Linux x64 DEB, TAR.GZ and Flatpak; Android ARM64 preview APK; iOS ARM64 simulator ZIP.

The source preceding this version bump passed 25 JavaScript bridge tests, 23 Rust app tests (two manual tests ignored), 10 iOS packaging tests and strict Clippy. Native installer checks passed on Windows, both macOS architectures and Linux. Android emulator and iOS simulator installation and launch passed. Release packages are rebuilt from this tag.

## Limits

Mobile previews use test ads. Fullscreen playback, earned rewards and early dismissal still need physical-device acceptance. Permanent paid ad removal is not implemented while store account setup is pending.

The iOS ZIP is simulator-only, not an installable iPhone IPA. Device distribution requires Apple signing and provisioning. Android previews are CI debug-signed; upgrading an older preview may require reinstalling if its signing key differs. Windows publisher signing needs configured credentials. macOS apps are ad-hoc signed; installers are not Developer ID signed or notarized.

Linux DEB/TAR.GZ require compatible host libraries; Flatpak supplies its managed GNOME 49 GTK/WebKitGTK runtime. Mobile background execution and notifications remain unfinished. Review rename destinations before confirming; operations can partially succeed. Original code is Apache 2.0 licensed; dependencies, SDKs and provider data retain their own terms.
