# Android Play upload builds

The Play bundle uses `com.tvshowmanager.app`, targets API 36 and contains the
ARM64 application payload. `MEDIA_MANAGER_PLAY_BUNDLE=true` configures a release
build with debugging disabled and CI signing disabled. Private upload signing
happens locally after download; CI never receives the private key.

The current upload version is `1.0.0-beta.3`, version code `1000003`. Increment
the Play version code in `android/kotlin-toolchain.init.gradle` before a subsequent
Play upload, and keep its version name and workflow filenames aligned with Cargo.

1. Build using `.github/workflows/play-bundle.yml` or the all-platform rebuild.
2. Run `python scripts/verify_android_bundle.py path/to/unsigned.aab` to check the
   bundle structure, native ABI and ELF load-segment alignment.
3. Validate the bundle and inspect its manifest with Google's official bundletool.
4. On Windows, sign with `scripts/sign_android_bundle.ps1 -Bundle <unsigned.aab>
   -Output <signed.aab>`. The script reuses the local upload key, verifies the JAR
   signature and creates a SHA-256 file and public upload certificate.

The default private signing directory is `%USERPROFILE%\Documents\Media Manager Signing`.
It contains an encrypted PKCS#12 keystore and a password protected with Windows
DPAPI. Keep this directory private and preserve the upload key for future builds.
DPAPI protection is tied to the Windows account/machine: before migrating, export
the password securely to your password manager. Copying the DPAPI file to another
PC alone is not a password-recovery plan. Never commit these files or send the
private key/password in chat. The exported public certificate is safe to provide
to Play Console when requested.

Google Play App Signing manages the distribution signing key separately from this
upload key. A signed, validated bundle is an upload artifact; store declarations,
privacy disclosures, account verification and testing requirements remain part
of the Play Console submission process.

## Validated upload build

The Play bundle workflow now retains Rust release DWARF line tables and exports
`native-debug-symbols.zip`, while also embedding the same symbols in AAB metadata
before signing. It checks the ELF build ID against the packaged `libmain.so`;
symbols from a different build must never be uploaded for an existing AAB.
The symbols cover the Rust application library. Precompiled VLC/vendor libraries
need matching symbols from their vendors for equivalent crash detail.
The symbol-enabled rebuild uses Android version code `1000004`.
Run 40760509362 (snapshot `299b86ed343424e83f0e5595765d8fdcf6a42754`)
successfully exported and embedded matching DWARF symbols for application ELF
build ID `5dfed43b9a8fc321ce7336d39e33e1d53df61100`. The standalone ZIP SHA-256 is
`c738c4db5d5ec357cfc69ec9991f651464483129c9e0899509f545acec6183cc`.
Both the new signed AAB and `native-debug-symbols.zip` are in Downloads. Use these
together; the ZIP is not intended for the earlier version-code-1000003 bundle.

The 2026-10-28 ARM64 release bundle was built from snapshot
`104f37babedf1d29574fb28b863bfaaf6c119715` in run 40750947812. Google's bundletool
validated the unsigned and locally signed AAB. Its manifest reports
`com.tvshowmanager.app`, `1.0.0-beta.3`, version code `1000003`, target API 36 and
both debugging/test-only flags disabled. All four native libraries have 16 KB
ELF load alignment; the bundle requests `PAGE_ALIGNMENT_16K` ZIP alignment. The
JAR signature was verified and its public certificate matched the local upload
identity. No private signing material was sent to CI or added to Git.

Native Windows, Intel/Apple Silicon macOS and Linux installer builds, Linux
Flatpak/runtime packaging and iOS simulator installation/launch completed in
run 40750120004 (the Apple Silicon runner needed one retry after queue cancellation).
The first Android job in that run hit a verifier filename mismatch; the corrected
independent Android run above passed. App source under `crates/` is identical
between the desktop/iOS and corrected Android snapshots. This is build/signature
validation, not a production-store compliance review or physical-device acceptance.
