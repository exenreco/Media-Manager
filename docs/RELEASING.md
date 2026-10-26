# Release process

## Automated platform previews

The `Platform releases` workflow runs when a version tag such as `v0.1.0` is pushed.
It can also rebuild an existing tag through Actions → Platform releases → Run workflow;
choose all platforms, desktop, Windows, Linux runtime, Android or iOS when retrying packaging.
The application source is always checked out at the requested tag. Mobile, Windows
and managed Linux runtime packaging helpers come from the workflow revision, so packaging-only
repairs can rebuild an existing version without moving its tag.
The tag must exactly match `[workspace.package].version` in Cargo.toml.

```sh
git tag -a v0.1.0 -m "Media Manager 0.1.0 preview"
git push origin v0.1.0
```

It creates one GitHub prerelease per version with separate assets for every
platform. Desktop jobs run tests before building and packaging the GUI and server.
Each job uploads its own assets so one platform failure does not hide successful
builds. Inspect all job results; release creation alone does not mean every build passed.
The workflow publishes no signing secrets and does not force-push or move tags.

| Asset | Build host / target | Distribution |
| --- | --- | --- |
| Windows x64 ZIP | windows-latest / native x64 | Conditional, verified Authenticode signing; unsigned until configured |
| macOS ARM64 ZIP | macos-15 / Apple Silicon | Ad-hoc signed app bundle and server |
| macOS Intel ZIP | macos-15-intel / x64 | Ad-hoc signed app bundle and server |
| Linux x64 TAR.GZ | ubuntu-24.04 / x64 | Portable binaries; GTK/WebKitGTK runtime required |
| Linux x64 Flatpak | ubuntu-24.04 / GNOME 49 | Managed GTK/WebKitGTK runtime; startup checked under Xvfb |
| Android ARM64 APK | ubuntu-24.04 / aarch64-linux-android | Debug-signed preview, Google test ads |
| iOS ARM64 Simulator ZIP | macos-15 / aarch64-apple-ios-sim | Simulator only, ad-hoc signed, Google test ads |

Dioxus CLI 0.7.10 is downloaded from its official release and its archive hash
is checked against that release's checksum file. Rust dependencies use Cargo.lock.
Desktop archives contain README, LICENSE and THIRD_PARTY_NOTICES. Binary archives
and mobile artifacts receive SHA-256 checksum files on the release page.

## Production/store releases still required

- Android: use a privately held stable release keystore, create release Gradle
  builds, sign APK/AAB files and configure Play distribution. Do not reuse the
  CI-generated debug key for production. Release-mode SDK integration selects
  the publisher's AdMob unit after consent; debug previews use Google's test unit.
- iOS devices/TestFlight/App Store: configure an Apple developer team, app-specific
  provisioning and distribution certificates, build the device target and run
  the documented Google framework embedding/signing step. A simulator ZIP cannot
  be renamed to IPA or installed on an iPhone.
- macOS: Developer ID signing and notarization before public production distribution.
- Windows: activate publisher signing in `.github/workflows/release.yml`
  after identity validation; optional installer packaging remains separate.
- All platforms: native acceptance tests, mobile document/background bridges,
  privacy disclosures, SDK policy review and store approval where applicable.

No private keys, certificates, provisioning profiles, API tokens or user workspace
files belong in Git. Store future release credentials in GitHub Actions secrets
or a protected signing environment. The `.gitignore` excludes common signing
and local-environment formats; always review the staged diff before committing.

## Local validation

Run the commands in README and verify the package helper on the intended native
host. Do not claim support from a cross-target check alone. Release pipeline jobs
are the source of truth for native compilation; successful compilation does not
prove mobile device behavior or policy approval.

See [managed Linux runtime packaging](LINUX_RUNTIME.md) for Flatpak installation,
permissions, runtime updates and source archives for redistributed libraries.
