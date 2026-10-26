# Media Manager preview

Rust UI and backend for TV/movie rename previews, season grouping, missing-file
reporting, editable proposed filenames, local/remote media playback, poster
previews/downloads, saved-workspace resume and desktop rename notifications.

Desktop layout includes resizable side panels and a compact gear dropdown. The
startup splash uses a transparent window with its native border/shadow disabled.
Android and iOS include separate native AdMob integrations; preview builds use test ads.

## Downloads

Platform jobs attach assets here as they finish:

- Windows x64 portable ZIP with app and optional server.
- macOS Apple Silicon and Intel ZIPs with app bundle and optional server.
- Linux x64 TAR.GZ with app and optional server, built on Ubuntu 24.04.
- Android ARM64 debug-signed preview APK with test ads.
- iOS ARM64 **Simulator-only** app ZIP with test ads, for use with Xcode on a Mac.

Verify downloads against the accompanying SHA-256 files. Missing assets mean the
corresponding job is pending or failed; inspect the Actions run before assuming
that platform has been validated.

## Preview limitations

Native mobile local-file access, background execution and mobile notifications
remain unfinished. Android is a development-signed preview, not a production
Play release. iOS is not an iPhone IPA or App Store/TestFlight distribution.
macOS bundles are ad-hoc signed, not Developer ID signed/notarized. Windows EXEs
are unsigned. Linux requires a compatible GTK/WebKitGTK runtime.

Renames modify real files, may partially succeed, and have no automatic undo.
Review every destination before applying. Original project code is Apache 2.0 licensed;
third-party libraries, ad SDKs and provider data retain their own terms.

See README and docs/RELEASING.md in the source for prerequisites and release status.
