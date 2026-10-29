# Android: one-time ad removal

The same Android app supports free and paid users. Settings → Advertising offers
**Remove ads** with the localized Google Play price and **Restore purchases**.
This is a non-consumable purchase: never consume it. A purchase removes native,
interstitial and rewarded ads on Android. It does not unlock iOS or desktop
purchases. Temporary rewarded ad-free time remains available to non-buyers.

## Play Console setup

1. Finish the Google Play payments/merchant setup for `com.tvshowmanager.app`.
2. Create a one-time product with product ID **`remove_ads`**, name **Remove ads**,
   and description **Remove all advertising from Media Manager on Android with
   one payment. No subscription.** Keep a single ordinary buy purchase option;
   enable it in the countries where the app is distributed and choose its price.
   The implementation deliberately rejects ambiguous multiple offers.
3. Obtain this app's Base64 **licensing public key** from Play Console's licensing
   settings. This is distinct from AdMob IDs, the upload signing key and any
   service-account credentials. The public key verifies Google's signed receipts.
4. Configure the build settings below, then build/sign a new AAB with a higher
   version code and upload it to an internal testing track. Install through that
   track using an opted-in tester account. A sideloaded debug APK is not evidence
   that the real product is purchasable.
5. Add test accounts under Play Console's license testing settings. Test through
   Google Play's test payment methods before making the product public.

Product activation, price, merchant setup and test accounts require the owner's
Play Console configuration; this code change does not publish those settings.
If Play Console offers only an upload button instead of product creation, upload
the new billing-enabled AAB to a testing track first. The older bundle did not
declare the Billing permission.

## Build settings

The existing Kotlin Gradle init script reads:

| Setting | Purpose |
| --- | --- |
| `MEDIA_MANAGER_AD_FREE_PRODUCT` | Product ID; defaults to `remove_ads` |
| `MEDIA_MANAGER_PLAY_PUBLIC_KEY` | Base64 licensing public key |
| `MEDIA_MANAGER_PLAY_PUBLIC_KEY_FILE` | Alternative UTF-8 file containing that public key |

In GitHub Actions, set repository **variables** `PLAY_BILLING_PUBLIC_KEY` and,
if the ID differs, `AD_FREE_PRODUCT_ID`. Mobile validation and release workflows
pass them to Gradle. The public key is embedded in BuildConfig; no private key,
service-account JSON or payment details belong in the application or repository.
If the public key is absent or invalid, purchases remain unavailable rather than
accepting unverified receipts. Existing free builds continue to work.

## Ownership and recovery

Only native Google Play callbacks/queries can supply receipts. The Android bridge
accepts state, buy and restore requests, never an ownership flag or purchase token.
The code checks the package, configured product, purchased state, quantity, token
and RSA signature. Pending payments do not remove ads. Verified purchases are
acknowledged, with short retries and another attempt on future refreshes. Failed
acknowledgement must be corrected before Google's refund deadline.

Signed receipts are kept in private app preferences and reverified at startup so
paid users can remain ad-free offline. Successful purchase queries on resume or
restore reconcile ownership, including removal of ownership when Play no longer
returns the purchase. Query failures preserve previously verified ownership.
Refund/revocation changes are therefore not immediate while offline. Google
recommends secure backend verification and real-time developer notifications for
stronger fraud/refund handling; this first implementation is client-side and
does not operate a shared purchase-verification service.

The price is loaded from Google Play and fetched again before checkout. The
user confirms payment in Google's purchase screen. Checkout suppresses ad
presentation. No credit-card details enter Media Manager.

## Acceptance checks before selling

- Test a successful purchase and verify all native and fullscreen ads disappear.
- Restart offline, reinstall and restore using the same Play account; try a
  different account with no purchase.
- Cancel checkout and test declined payment: neither grants ownership.
- Test a pending payment, then completion and cancellation; resume the app.
- Refund and revoke a test purchase, then refresh ownership while online.
- Test lost connectivity during acknowledgement, recovery and duplicate taps.
- Verify the localized price and that the purchase is non-consumable.

JavaScript tests cover UI actions, pending/owned states, checkout suppression and
restoration requests. Native JVM tests exercise receipt signature verification
with real RSA keys, tampering and wrong-key failures. CI compilation/launch and
synthetic tests do not replace the Play internal-track payment checks above.

References: [Billing integration](https://developer.android.com/google/play/billing/integrate),
[Billing security](https://developer.android.com/google/play/billing/security),
[Billing testing](https://developer.android.com/google/play/billing/test).

## Current validation

Snapshot `d5ccd25efd25f486263a5bb34826b6e92ecc51b6` passed Android ARM64 and
x86_64 builds, native RSA receipt-signature tests, emulator installation/launch
and both inline H.264 playback fixtures in
[40824586252](https://github.com/exenreco/Media-Manager/actions/runs/40824586252).
All 35 JavaScript tests passed. Windows, macOS and Linux regression checks passed
in [40824573993](https://github.com/exenreco/Media-Manager/actions/runs/40824573993).
The public key and Play product were not configured, so no real purchase,
pending-payment, acknowledgement, restore or refund transaction was exercised.
Those require the internal-track acceptance checks above. No release was published.

## Play billing configuration (October 29, 2026)

The `remove_ads` product is active with the `standard` Buy purchase option at
USD 5.99 and Google-generated regional prices. Multi-quantity purchases are off.
The app's RSA-2048 licensing public key is configured as the repository variable
`PLAY_BILLING_PUBLIC_KEY`; `AD_FREE_PRODUCT_ID` is `remove_ads`. The existing
Media Manager Internal Testers list is enabled for license testing with
`RESPOND_NORMALLY`.

The dedicated Play bundle workflow now uses version code **1000007** while
retaining version name **1.0.0**. It requires a valid configured public key,
checks the generated release BuildConfig contains it, runs billing bridge and
native receipt-signature tests, and exports an unsigned APK, AAB and native
symbols for local upload-key signing. Existing published 1000006 packages do
not gain billing configuration automatically.

Real Play purchase, acknowledgement, reinstall/restore, pending-payment and
refund checks must be recorded separately from automated tests. They require a
Play Store installation and a signed-in license tester. Use the test payment
method shown by Google Play; do not mistake an ordinary USD 5.99 checkout for a
test purchase.

Build 1000007 passed [Play bundle build 40921532894](https://github.com/exenreco/Media-Manager/actions/runs/40921532894)
and [Android validation 40921981643](https://github.com/exenreco/Media-Manager/actions/runs/40921981643).
The latter includes native receipt tests, emulator launch, and inline H.264
8-bit/10-bit playback fixtures. Locally signed APK/AAB packages were inspected
and both contain the configured public key and `remove_ads`; the AAB manifest
reports versionCode 1000007, versionName 1.0.0 and the BILLING permission.

A separate `Media_Manager_Play_Billing` API 35 emulator with Google's Play Store
image is available for manual license-tester sign-in. No successful Play payment,
acknowledgement or purchase restoration has been recorded yet.

The signed 1000007 bundle is published to the existing Play internal testing
track (release 2) and Console reports **Available to internal testers**. This
is an internal test rollout, not a production release. A validated x86_64
debug APK containing the public key is installed in the local Play Store
emulator; Google Play sign-in and real test transactions are still pending.
