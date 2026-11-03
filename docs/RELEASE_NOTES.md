# Media Manager 1.0.3

- Interactive first-run walkthrough on every app page, with highlights on real controls and a replay button beside the TV show/movie toggle.
- Improved Library and Artwork bottom-toolbar layout and mobile footer navigation, including tablet layouts.
- Windows and macOS Go Pro settings with verified store ownership and a free allowance of two distinct titles per two hours. Episodes of the same show share one title.
- iOS one-time Remove ads purchase and ten-minute restore window, native full-screen lifecycle support, folder-picker fixes, and clearer rewarded-ad availability messages.
- Rename handling and shared billing, playback, and advertising refinements.

Windows Pro checkout requires a Microsoft Store listing and identity, which are not configured yet. Store unavailability is reported in the app; it does not grant Pro.

Android ARM64 version code: **1000011**. CI APK/AAB files are **unsigned** and require the existing private Android upload/signing key before installation or Play upload. Native debug symbols accompany the Android build. No private signing material is included in the release.

Linux DEB, portable archive and managed-runtime Flatpak, Windows setup/portable packages, and macOS desktop packages are attached as their builds pass. Windows publisher signing depends on the repository signing configuration. Apple App Store submissions are managed separately; the iOS simulator archive is a preview, not an installable iPhone release.
