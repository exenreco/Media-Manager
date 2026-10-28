# Media Manager 1.0.0 beta 3

## Changes

- Added desktop update checks in Settings, with stable/beta channels and verified installer downloads. Update manifests use Ed25519 signatures; downloaded packages are checked for size and SHA-256 before opening the installer.
- Replaced Android video transfer through the JavaScript storage bridge with native streaming for local and paired-server files, including seeking and stream cleanup when selection changes. Inline preview requires Android 8 or newer.
- Added cancellable preview preparation and buffering feedback to reduce stale preview work.
- Matched API source, artwork sorting and library ordering sheets to the mobile preview sheet, including dimensions, backdrop, animation and dismissal.

## Downloads and validation

Platform jobs attach packages and SHA-256 checksums as they complete: Windows x64 setup and portable ZIP; macOS Apple Silicon and Intel PKG and ZIP; Linux x64 DEB, TAR.GZ and Flatpak; Android ARM64 preview APK; iOS ARM64 simulator ZIP.

The source preceding this version bump passed desktop checks on Windows, macOS and Linux, Android ARM64 compilation, Android x86_64 emulator installation/launch, and iOS ARM64 simulator installation/launch. Playback range tests and strict app Clippy passed. Release packages are rebuilt from this tag. Actual playback with user media and physical-device codec compatibility remain unverified.

## Limits

Mobile previews use test ads. Fullscreen playback, earned rewards and early dismissal still need physical-device acceptance. Permanent paid ad removal is not implemented while store account setup is pending.

The iOS ZIP is simulator-only, not an installable iPhone IPA. Device distribution requires Apple signing and provisioning. Android previews are CI debug-signed; upgrading an older preview may require reinstalling if its signing key differs. Windows publisher signing needs configured credentials. macOS apps are ad-hoc signed; installers are not Developer ID signed or notarized.

Linux DEB/TAR.GZ require compatible host libraries; Flatpak supplies its managed GNOME 49 GTK/WebKitGTK runtime. Mobile background execution and notifications remain unfinished. Review rename destinations before confirming; operations can partially succeed. Original code is Apache 2.0 licensed; dependencies, SDKs and provider data retain their own terms.
