# Release process

## Automated platform previews

The `Platform releases` workflow runs when a version tag such as `v0.1.0` is pushed.
It can also rebuild an existing tag through Actions → Platform releases → Run workflow;
choose all platforms, desktop, Windows, Linux runtime, Android or iOS when retrying packaging.
The application source is always checked out at the requested tag. Mobile, Windows
and desktop/managed Linux runtime packaging helpers come from the workflow revision, so packaging-only
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
| Windows x64 setup EXE | windows-latest / Inno Setup 6 | Per-user install, shortcuts, uninstall and WebView2 bootstrapper; conditional publisher signing |
| macOS ARM64 ZIP | macos-15 / Apple Silicon | Ad-hoc signed app bundle and server |
| macOS Intel ZIP | macos-15-intel / x64 | Ad-hoc signed app bundle and server |
| macOS ARM64 / Intel PKG | corresponding native macOS runner | Installs app and bundled server in /Applications; unsigned installer |
| Linux x64 TAR.GZ | ubuntu-24.04 / x64 | Portable binaries; GTK/WebKitGTK runtime required |
| Linux x64 DEB | ubuntu-24.04 / x64 | Native dependencies discovered by dpkg-shlibdeps; installs launcher, icon and server |
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
  after identity validation. Both payload executables and the setup EXE are signed
  when enabled; the setup checksum is regenerated after signing.
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

## Desktop installer validation

The `Desktop installer validation` workflow can be dispatched manually or run on
`codex/installer-validation-*` branches. It builds native release binaries and
installers on Windows, Apple Silicon macOS, Intel macOS and Ubuntu 24.04, without
publishing a GitHub release. Windows checks installation, reinstall/upgrade,
startup and uninstall in a disposable runner. macOS checks installation, bundle
signature integrity and reinstall. Linux checks dependency installation, desktop
entry validation, startup under Xvfb and package removal. These checks do not
establish publisher trust, notarization or acceptance on every Linux distribution.

Local builds run `scripts/package_desktop.py` first, then
`scripts/package_installer.py` with the same platform, architecture and version.
Windows also requires Inno Setup 6.3+ and a verified WebView2 bootstrapper;
`scripts/prepare_windows_installer.ps1` provisions them on the CI runner. macOS
uses the built-in pkgbuild/codesign tools. Linux requires dpkg-dev and the native
development libraries used to build the application.

### Validated installer snapshot (2026-10-27)

[Native installer validation](https://github.com/exenreco/Media-Manager/actions/runs/40591527005)
for `4d0a9f4609223126d9940a0e6c50602de3b45cf7` passed Windows setup,
upgrade/reinstall, startup and uninstall; installation/reinstall and bundle
integrity on both Apple Silicon and Intel macOS; and Linux dependency
installation, launcher validation, startup and removal. Ten Python packaging
tests passed on every runner. Generated installer artifacts have SHA-256 files.
This validation did not publish a release, configure publisher signing, test
the missing-WebView2 path on a clean Windows installation, or establish macOS
Gatekeeper acceptance. The final release workflow also refreshes the Windows
installer checksum after optional publisher signing.
