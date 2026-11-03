# Physical-device review preparation

The owner chose to test using the only available physical device, an iPhone XS
Max running **iOS 18.7.11 (22H384)**. Finder trust is complete. Do not describe
this device as running iOS 27 or replace its footage with simulator footage.
Apple's request for the latest OS remains a review consideration.

Use the exact uploaded iOS **1.0.3 (1.0.4)** build through TestFlight. Internal
group **Physical Device QA** (`00d3e0bc-b06e-47e3-9e20-be8dc648b01e`) has manual
build distribution, one build and one invited existing account-holder tester.
The owner explicitly authorized Apple's email invitation to their existing
developer account. The owner confirmed installation, and QuickTime's physical
screen feed shows Media Manager open at interactive guide step 1 of 28.

TestFlight avoids development signing and requires no Developer Mode change for
this installation path. Device Hub currently asks for Developer Mode, so use
QuickTime Player's USB **Screen → Exenreco’s iPhone** input for recording instead.
Do not select the phone's Camera or Microphone input for an app-screen demo.
QuickTime is prepared but recording has not started.
The sample ZIP was delivered by AirDrop (Sent). The owner has been asked to
extract it in Files and return to Home before recording begins with app launch.

Synthetic samples and a plain-text test script are in
`/tmp/media-manager-run/physical-review/Media-Manager-Demo-Samples.zip`.
The two two-second gray MP4 videos contain no private media or third-party footage.
The script covers launch, guide, folder selection, manual metadata, filename
preview, media playback, confirmed rename, artwork/health navigation, advertising
purchase UI and guide replay. No physical-device test result or successful
purchase is claimed until observed.

Before sending anything to App Review, finish real-device QA, review the recording
for accidental private information, update review notes with the actual model/OS
and recording location, and verify all required review information is complete.

## Observed physical-device results

- Manual metadata generated both episode previews; applying reported **2 renamed,
  0 rename failures**. README.txt was skipped.
- TestFlight displayed the correct $5.99 Remove ads purchase sheet with its
  explicit no-charge testing notice. Recording stopped before authentication.
- After retrying the sandbox purchase, Settings showed **Ads removed** and
  **All ads removed on iOS**, with Restore purchases enabled.
- The owner tapped Restore; the same active entitlement message remained.
  This confirms continued ownership, not an independent reinstall restoration test.
- After the owner closed and reopened the app, the physical screen still showed
  the active ad-removal entitlement and restored workspace settings.
- The unpurchased flow showed an advertising privacy-check failure (code 3).
  Rewarded-ad completion has not been verified and needs investigation.

Saved recordings (not yet reviewed for upload or submitted):
`/tmp/media-manager-run/physical-review/Media-Manager-iPhone-demo.mov` and
`/tmp/media-manager-run/physical-review/Media-Manager-iPhone-purchase-persistence.mov`.
The second recording was stopped and saved after verifying relaunch persistence.

## Rewarded-ad investigation

Google UMP documents consent-request error 3 as a network error communicating
with Funding Choices, distinct from ad-load no-fill errors. The observed message
comes from `requestConsentInfoUpdate`, before rewarded-ad loading is authorized.
On a real host-network DNS check (outside the sandbox), both
`fundingchoicesmessages.google.com` and `googleads.g.doubleclick.net` resolved to
`0.0.0.0`, while `www.apple.com` resolved normally. This confirms filtering on the
Mac's network; whether the phone uses the same filtered resolver is unverified.
Retest on an unfiltered connection to distinguish phone/network configuration.
Do not bypass UMP consent or grant the 1800-second reward without Google's earned
reward callback. The current phone's verified Remove ads ownership intentionally
suppresses rewarded ads, so this phone now needs a separate non-owning sandbox
test entitlement environment to exercise the rewarded flow; do not revoke or
clear an owner's purchase merely to test ads.
Reference: https://developers.google.com/admob/ios/privacy/api/reference/Enums/UMPRequestErrorCode

## Isolated development QA app

Owner approved development signing. Xcode created a development signing identity
and a device-inclusive `iOS Team Provisioning Profile: *`. Installed and launched
`com.tvshowmanager.app.adqa` (display name Media Manager Ads QA), alongside the
TestFlight app, using the existing release Rust executable and a locally rebuilt
Swift plugin with DEBUG Google test ad units. QA-only UMP debug settings are in
`/tmp/media-manager-run/rewarded-test/Sources`, not production sources. Production
AdMob app ID is retained for consent configuration. This QA variant is not an
App Store candidate and has no separately configured purchase product.

The physical QA app currently reports UMP error 3 with description `bad URL`.
Rewarded loading/reward completion remains unverified; consent has not been
bypassed. Build/sign/install scripts and app-only console logs are under
`/tmp/media-manager-run/rewarded-test/`. The successful live-ad request's no-fill
code 1 is a separate observed result from TestFlight, not this QA consent failure.

## TestFlight follow-up build 1.0.5

Owner requested delivery through TestFlight after the QA folder picker remained
open. Production iOS 1.0.3 (1.0.5) was uploaded successfully via Xcode on November
3, 2026. Apple reported `Upload succeeded` / `EXPORT SUCCEEDED`; processing and
internal-group availability must be verified separately.

This build retains the released Rust executable and rebuilds the native Swift
plugin from repository source without DEBUG or QA consent overrides. It includes
explicit document-picker dismissal and callback diagnostics, plus a distinct
no-fill message for rewarded ad error 1. These changes are not yet physically
verified in TestFlight. Owner reported the QA 30-minute ad worked, but an earned
reward state has not yet been independently observed in the recording.

Archive: `/tmp/media-manager-run/apple-1.0.3/Media-Manager-iOS-1.0.3-build-1.0.5.xcarchive`.
IPA: `dist/ios/Media-Manager-1.0.3-build-1.0.5.ipa` (with SHA-256 sidecar).
The local archive includes the full UUID-matched compiler-generated plugin dSYM.

Apple finished processing build 1.0.5. Saved the standard-encryption declaration
with France excluded, then added 1.0.3 (1.0.5) to **Physical Device QA**. App Store
Connect confirms **Testing**, one tester and two builds. Build UUID:
`4e4add6a-29b8-430a-aa80-c3f57f17c434`. Screenshot:
`/tmp/media-manager-run/physical-review/testflight-1.0.5-testing.png`.

Owner confirmed TestFlight build 1.0.5 checks passed: folder selection opens,
Library shows both videos, and rewarded-ad flow works (including the requested
30-minute check). These are owner-reported results, not a new captured recording.
Requested a final recording beginning at Home/app launch for the current build
before updating App Review evidence and resubmitting.

Current build 1.0.5 is selected and saved on the iOS review draft, with actual
owner-reported QA results and the iPhone XS Max/iOS 18.7.11 limitation stated.
Not resubmitted. Final capture showed active purchased ad removal, but QuickTime
returned to its capture preview after Stop without exposing a verifiable new
movie document. Do not label any older unsaved take as the build-1.0.5 recording.
Need a verified saved final recording (phone Screen Recording is a fallback).
Draft proof: `/tmp/media-manager-run/physical-review/ios-1.0.5-review-draft.png`.

Recovered the interrupted QuickTime take after opening the Movie Recording
window and acknowledging Apple's Recording Stopped conflict alert. Saved it as
`Media-Manager-iPhone-1.0.5-final.mov` (65 seconds). It starts at the lock screen;
selected frames show workspace prompt at 8 seconds and purchased ad removal at
18/31 seconds. It must not be uploaded without checking/removing authentication
footage. A complete launch/folder/Library/purchase demonstration remains needed;
requested an iPhone-native Screen Recording to avoid the USB capture conflict.

## Owner-provided iPhone-native recording reviewed

Owner supplied `/Users/exenrecobell/Downloads/ScreenRecording_11-03-2026 03-41-25_1.mp4`.
Copied unchanged to `/tmp/media-manager-run/physical-review/Media-Manager-iOS-1.0.3-build-1.0.5-device-demo.mp4`
(18,209,929 bytes, approximately 61 seconds). QuickTime review confirmed opening
from App Library, the interactive guide, folder-selection flow, Library showing
`S01E01 - Pilot.mp4` and `S01E02 - Second Episode.mp4` at ~47 seconds, and Settings
showing Ads removed at ~54 seconds. It is a physical iPhone-native recording,
not a simulator. Do not claim a new purchase or rewarded completion is recorded;
it shows existing purchased ad removal. Disclose iPhone XS Max/iOS 18.7.11.

Attachment and iOS resubmission are NOT complete. On resuming after heartbeat,
CUA reported the Mac locked and App Store Connect at login. Asked the owner to
unlock and sign back in. Mac submission was already confirmed Waiting for Review.

## iOS resubmission completed (November 3, 2026)

After the owner signed back in, attached the verified iPhone-native MP4. Apple's
direct MP4 attachment remained Processing and prevented Update Review with
“There are still attachment uploads in progress.” Replaced that incomplete
association with `Media-Manager-iOS-1.0.5-review-demo.zip` (17,762,376 bytes),
containing the unchanged MP4 and README identifying the physical device, OS,
build and recording contents. The ZIP completed processing successfully.

Saved final review notes identifying the ZIP, stating that the video demonstrates
existing purchased ad removal, and explicitly disclosing iPhone XS Max/iOS
18.7.11 and inability to run iOS 27. Requested consideration of this footage and
reviewer guidance if a newer device is still required; did not claim latest-major
OS compliance.

Update Review passed; resubmitted iOS 1.0.3 (1.0.5) with Remove ads. App Store
Connect confirms **Waiting for Review** for both items in submission
`cf389c37-3e0d-46c4-b374-221e718652fe`, submitted Nov 3, 2026 at 1:09 PM.
Proof: `/tmp/media-manager-run/physical-review/ios-resubmitted.jpg`.
