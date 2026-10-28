# Media Manager 1.0.0 beta 4

## Changes

- Added native compatibility playback using LibVLC on Android and VLCKit on iOS, alongside inline previews. Broadened media-extension recognition and shared MIME handling.
- Added optional 30-second H.264/AAC compatibility previews for desktop and paired libraries using FFmpeg installed on the library host. Original media is preserved.
- Moved Android player teardown off the UI thread and kept playback preparation cancellable to reduce freezes and stale playback when switching items.
- Moved Advertising below rename history in Settings.
- Added Android API 36 Play bundle tooling, private upload-key signing, 16 KB native alignment checks, and matching native debug symbol exports.
- Added the privacy policy, current Android screenshots, and Google Play listing assets; removed the older dashboard screenshot.

## Downloads and validation

Release CI rebuilds Windows x64 setup/portable packages, Apple Silicon and Intel macOS PKG/ZIP packages, Linux x64 DEB/TAR.GZ/Flatpak packages, an Android ARM64 preview APK, and an iOS ARM64 simulator ZIP. Checksums and signed desktop updater metadata appear as jobs complete. Check the workflow results before assuming every platform is available.

The preceding application snapshot passed native desktop checks on Windows, macOS and Linux, Android ARM64/x86_64 compilation, and iOS simulator compilation/launch. A generated MPEG-4 Part 2 MP4 played to completion in the Android compatibility player without a crash. Physical-device playback and the original user-reported MP4 remain unverified. See docs/PLAYBACK.md and docs/MOBILE_VALIDATION.md.

## Limits

GitHub Android and iOS preview packages use test ads. The separately signed Play AAB is an upload artifact, not a GitHub preview APK; its native symbols must match that exact build. The beta.3 version-code-1000004 Play upload remains separate from this release. A beta.4 Play rebuild uses version code 1000005.

The iOS ZIP is for Simulator, not an iPhone IPA. Device distribution requires Apple signing/provisioning. Windows publisher signing remains conditional on configured credentials. macOS apps are ad-hoc signed; installers are not Developer ID signed or notarized.

Playback depends on the container, codecs, device and decoder build. FFmpeg is not bundled; the Flatpak sandbox needs an accessible converter for compatibility conversion. Native decoder libraries retain their own licenses and notices. Linux Flatpak supplies GTK/WebKitGTK; DEB/TAR.GZ need compatible host libraries.

Mobile background execution and notifications, physical-device advertising acceptance, and permanent paid ad removal remain unfinished. Review rename destinations before applying; operations can partially succeed, with completed changes recorded for explicit Undo.
