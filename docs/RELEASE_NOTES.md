# Media Manager 1.0.0 beta

## Changes

- Redesigned desktop workspace with an icon rail, custom title bar, resizable media inspector and inline video preview/title editing.
- Updated Android and iOS layouts with larger readable text, safe-area spacing and animated bottom navigation.
- Added Get Started connection setup, Local/Remote tabs, API settings and connection checks before entering the workspace.
- Details changes refresh the rename preview automatically. Ordering and saved profiles are available from the Library ordering popup.
- Combined Library and Art Work views, with searchable/sortable responsive poster grids, local artwork inventory, side-by-side replacement previews and explicit single/bulk poster downloads.
- Added automatic library health scans and charts; rename history and explicit Undo are on Settings.
- Added TMDB movie matching and TMDB-compatible custom movie providers, including optional year filtering.
- Added camera-based single-use QR pairing with authenticated local HTTPS sharing and pinned server certificates.
- Added Windows setup, macOS PKG and Debian/Ubuntu DEB installers while retaining portable archives and Linux Flatpak.

## Downloads

Platform jobs attach assets and SHA-256 checksums as they finish:

- Windows x64 setup EXE and portable ZIP, including the optional server. Setup installs WebView2 if missing.
- macOS Apple Silicon and Intel PKG installers and portable ZIPs, including the optional server.
- Linux x64 DEB for Ubuntu 24.04-compatible systems, TAR.GZ and Flatpak with managed GTK/WebKitGTK runtime.
- Android ARM64 debug-signed beta APK with Google test ads.
- iOS ARM64 **Simulator-only** ZIP for Xcode on a Mac; this is not an installable iPhone IPA.

## Validation and limits

Native installer validation passed Windows install/upgrade/startup/uninstall,
installation/reinstall on both Mac architectures, and Linux dependency installation,
launcher/startup/removal. Android and iOS builds and simulator startup passed on
preceding design snapshots; the final beta release is rebuilt from its tag.
Physical-device and full provider acceptance remain pending. Android advertising
privacy checks/ad delivery remain unresolved. No live TMDB account test was made.

Windows publisher signing requires configured credentials. macOS apps are ad-hoc
signed; PKGs are not Developer ID signed or notarized. Android uses a CI debug key;
iOS device distribution requires Apple provisioning. Mobile background execution
and notifications remain unfinished. Linux DEB/TAR.GZ use compatible host libraries;
Flatpak supplies a managed GNOME 49 runtime.

Renames require confirmation and can partially succeed. Review destinations and
inspect recorded Undo results. Poster replacement requires explicit consent.
Original code is Apache 2.0 licensed; dependencies, SDKs and provider data retain
their own terms. See README and the installation/mobile validation documentation.
