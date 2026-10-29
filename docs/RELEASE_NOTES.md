# Media Manager 1.0.0

Stable release of the Rust desktop and mobile source.

## Changes

- Browse connected local or paired libraries without entering Details, a title, year or provider credentials. Read-only listings support season grouping and playback; Details supplies rename suggestions separately.
- Android inline previews now use LibVLC, with pause and seek controls. Synthetic H.264/AAC 24 fps and 10-bit samples completed playback without detected crashes or ANRs.
- Mobile advertisements follow Selected provider ID on Details and the search/sort controls on Artwork and Library.
- Added Android one-time ad-removal purchase and restore controls, signed-receipt verification, pending-payment handling and purchase acknowledgement. Checkout remains unavailable until the Play product and licensing public key are configured.
- Settings places Release channel and automatic checking inline, with App updates below Rename history.
- Includes title matching, anime numbering and mappings, Plex/Jellyfin naming, companion-file renaming, profiles, artwork management, health checks, rename history/Undo and QR pairing.

## Packages

Windows x64 setup and portable packages; macOS Apple Silicon and Intel PKG/ZIP; Linux x64 DEB/TAR.GZ and Flatpak with managed GTK/WebKitGTK; Android ARM64 release APK and Play upload AAB with matching native symbols; iOS ARM64 Simulator ZIP.

SHA-256 checksums cover the final packages. Desktop updater metadata is signed with the application's pinned Ed25519 key. The release is promoted from staging to stable after builds, Android signing and asset verification finish.

## Platform limits

The Android release APK uses production ad units and the existing private upload signing identity. Google Play re-signs delivered apps with its own app-signing identity. Ad removal still needs an active `remove_ads` product and the Google Play licensing public key, followed by internal-track transaction testing. Version code: 1000006. No Play Console release or listing is published by this GitHub release.

The iOS ZIP is for Xcode Simulator and uses test ads; it is not an iPhone IPA. Apple device signing/provisioning is not configured. Windows publisher signing depends on configured credentials. macOS apps remain ad-hoc signed; PKG installers are not Developer ID signed or notarized.

Physical-device acceptance, mobile background jobs and notifications remain pending. Playback depends on codecs/device support. FFmpeg compatibility conversion requires FFmpeg on the library host; original files are preserved. Linux DEB/TAR.GZ require compatible host libraries. Bundled native decoders retain their own licenses and notices.

Rename operations require confirmation and may partially succeed. Completed changes are recorded for explicit Undo. Review all destinations before applying.
