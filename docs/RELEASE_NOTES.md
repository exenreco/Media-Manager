# Media Manager 1.0.1

## Changes

- Android ad removal now verifies purchases through the HTTPS purchase service. Pending payments never grant ad removal; approved payments, Restore, refunds and revocations synchronize with server-confirmed ownership.
- Authenticated Google Play notifications update purchase status. Native requests preserve signed transaction proofs without exposing them to the WebView.
- Foreground retries and request revision guards prevent stale Play responses from overwriting newer purchase results.
- Settings adds dividers around Advertising and optional rewarded ads, plus spacing between purchase and restore buttons.
- Restore remains visible and is enabled for 24 hours after the first confirmed payment on the device. It is then disabled; automatic ownership checks and purchased ad removal continue.

## Validation

Live Android emulator tests passed for slow payment approval, acknowledgement, restore, refund/revocation and slow decline. Authenticated purchase and refund notifications returned HTTP 200; unauthenticated requests were rejected. New packages are built from this release revision.

## Packages and limits

Windows x64 installer/portable; macOS Apple Silicon and Intel PKG/ZIP; Linux x64 DEB/TAR.GZ and Flatpak; Android ARM64 signed APK and Play upload AAB with native symbols; iOS ARM64 Simulator ZIP.

Android version code: 1000009. Google Play closed-testing and production publication are separate from this GitHub release. Production access requires Google's tester and duration criteria.

The iOS ZIP is for Simulator, not a signed iPhone IPA. Windows publisher signing depends on configured credentials; macOS remains ad-hoc signed and unnotarized. Physical-device acceptance remains outstanding. Codec support depends on native libraries and the device; FFmpeg compatibility conversion requires FFmpeg on the library host. Original files are preserved.
