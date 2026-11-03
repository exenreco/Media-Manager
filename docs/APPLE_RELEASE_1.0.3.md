# Apple release 1.0.3

Prepared on 2026-11-02 for App Store Connect app `6829643613`, bundle ID
`com.tvshowmanager.app`, team `PBA93F33RM`.

Both iOS and macOS release builds use marketing version **1.0.3** and build
**1.0.4**. Both signed archives were uploaded successfully using the existing
Xcode account. Uploading does not submit a version to App Review.

## Local artifacts

- `dist/ios/Media-Manager-1.0.3-build-1.0.4.ipa` and SHA-256 checksum.
- `dist/macos/Media-Manager-1.0.3-build-1.0.4.pkg` and SHA-256 checksum.
- Archives are registered under `~/Library/Developer/Xcode/Archives/2026-11-02/`.
- Matching QA screenshots and packaging/export options are under
  `/tmp/media-manager-run/apple-1.0.3/`.

The iOS archive uses the existing Media Manager App Store distribution profile.
The Mac archive uses the new **Media Manager Mac App Store** profile and a
**Mac Installer Distribution** certificate. Signing keys remain in Keychain.
The Mac installer signature and both app signatures were verified locally.

## Mac App Store variant

Build with `cargo build --release -p tv-manager-app --features mac-app-store`.
This variant removes the external release updater and states that updates are
provided by the Mac App Store. Other desktop distributions retain their updater.
The signed Mac bundle enables App Sandbox, user-selected file read/write access,
and client/server networking for metadata, remote libraries and optional local
pairing. A separate sandboxed QA copy of the same release executable launched
successfully and loaded StoreKit's $10.99 Desktop Pro price.

The Mac product is `desktop_pro` (Apple ID `6829836504`); iOS continues to use
`remove_ads`. Neither ownership nor a successful purchase is simulated by the app.
Actual store sandbox purchase/restore acceptance remains pending.

## Validation and review

- Mac App Store variant: 31 Rust tests passed, two manual tests ignored.
- Purchase, fullscreen-ad and walkthrough JavaScript: 17 tests passed.
- iOS packaging: 12 Python tests passed.
- Physical-device iOS release and native macOS release compiled successfully.
- Xcode's separate iOS validation completed with a missing VLCKit dSYM warning.
  The existing vendor dSYM was verified against the new framework's UUID
  `ACCF2C63-55FF-3A32-89D7-3D63521C537B`, included in the new archive, and supplied
  in the successful upload. Native app/Swift plugin symbols are also included.
- Current simulator screenshots use 1320×2868 iPhone and 2064×2752 iPad output;
  Mac screenshots use the actual 1280×800 app window.

Apple rejected the previous iOS 1.0.2 submission under Guideline 2.1 and requested
additional review information, including a recording captured on a physical
phone/tablet running the latest OS, beginning with launch and showing the typical
flow and paid feature. Simulator screenshots do not fulfill that recording
requirement. The new draft notes describe purpose, audience, setup, paid features,
external services, regional behavior and rights; physical-device QA/recording
must be completed before resubmission. No physical device was connected at the
initial release check.

France remains excluded under the owner's existing decision. No new financial,
privacy, content-rights or account agreements are made by this release record.

## App Store Connect draft status

Both uploads processed successfully and were marked **Ready to Submit** in
TestFlight after completing the standard-encryption questions with France
excluded. Both **1.0.3** version drafts now have build **1.0.4** attached and saved.
The Mac draft has three current screenshots; iOS has updated iPhone/iPad
screenshots and detailed review notes. The previous iOS build **1.0.3**
(marketing version 1.0.2) was replaced with the new **1.0.4** build.

The interrupted iOS association was completed after the Mac was unlocked.
Neither new version has been submitted to App Review by this release preparation.
Physical-device QA/recording requested in the previous iOS rejection and native
store purchase/restore acceptance remain pending.

### iOS follow-up build 1.0.5 (November 3, 2026)

Uploaded successfully to App Store Connect for TestFlight after the owner requested
using TestFlight for continued physical-device testing. Uses production ad IDs,
no DEBUG/test-device overrides, and the existing release Rust executable with a
rebuilt native Swift plugin. Changes: explicit folder-picker dismissal and
completion logging; rewarded no-fill error reports unavailable inventory rather
than blaming connectivity. Native Swift compile, 12 iOS packaging checks and
`git diff --check` passed. Folder-picker behavior still requires TestFlight QA.
See `PHYSICAL_DEVICE_REVIEW.md` for diagnostics and current distribution status.

### macOS submitted (November 3, 2026)

Installed the exact 1.0.3 (1.0.4) Mac build through Apple's TestFlight. StoreKit
reported verified Pro ownership. Restore completed after the owner privately
completed Apple authentication, and Pro remained active after quit/relaunch.

Mac listing screenshots had failed because JPEG bytes were stored under .png
filenames. Removed the failed version associations (assets remain in Asset
Library), reuploaded correctly named .jpg captures, and Apple's submission
validation accepted them. Updated Mac review notes with completed StoreKit QA.

Submitted the Mac app and Desktop Pro together. App Store Connect confirms
**Waiting for Review** for both items. Submission:
`3ff274ca-53a0-42ce-9775-4c686fb72939`.
https://appstoreconnect.apple.com/apps/6829643613/distribution/reviewsubmissions/details/3ff274ca-53a0-42ce-9775-4c686fb72939

Proof: `/tmp/media-manager-run/physical-review/macos-submitted.jpg` and
`/tmp/media-manager-run/physical-review/macos-testflight-pro-active.png` (the latter
contains JPEG data, despite the historical filename).

### iOS resubmitted (November 3, 2026)

The verified owner-provided physical iPhone recording was attached as
`Media-Manager-iOS-1.0.5-review-demo.zip` (unchanged MP4 plus device/build README).
The direct MP4 upload stalled; the ZIP processed and passed Apple's validation.
Review notes disclose the actual XS Max/iOS 18.7.11 device and request
consideration without claiming iOS 27 footage. Resubmitted iOS 1.0.3 (1.0.5) and
Remove ads together. Both now show **Waiting for Review** in submission
`cf389c37-3e0d-46c4-b374-221e718652fe`.
Proof: `/tmp/media-manager-run/physical-review/ios-resubmitted.jpg`.
Both iOS and macOS submissions are now awaiting Apple's review; neither is
approved or live yet.

### macOS Guideline 2.4.5 clarification and resubmission

Apple's automated rejection stated `com.apple.security.network.server` did not
appear to have matching functionality. Inspected the submitted TestFlight app's
actual Get Started > Remote > Pair a mobile device controls and source. The
Mac build includes an optional local HTTPS library host (`host_library`), bound
to the selected private LAN address and an OS-assigned port. It accepts incoming
authenticated paired-phone requests. Removing the entitlement would break that
feature. The QR code is single-use/valid five minutes; Stop local sharing drops
the listener. The listener itself is not claimed to expire with the QR.

Posted the requested clarification to Apple, added exact reviewer steps and the
technical explanation to Mac App Review Information, and attached
`Media-Manager-macOS-pairing-evidence.zip` (actual TestFlight control screenshot
and README). Direct JPEG attachment stalled in processing; the ZIP completed.
No new binary was required: retained 1.0.3 (1.0.4) with its needed entitlement.
Update Review passed and resubmitted the app and Desktop Pro. Both confirmed
**Waiting for Review** on Nov 3, 2026 at 1:29 PM, submission
`3ff274ca-53a0-42ce-9775-4c686fb72939`.
Proof: `/tmp/media-manager-run/physical-review/macos-2.4.5-resubmitted.jpg`.
No local library listener was started or personal media shared for this check.
