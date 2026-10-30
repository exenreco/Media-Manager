# Android: one-time ad removal

The same Android app supports free and paid users. Settings > Advertising offers
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

## Current validation — October 29, 2026

- Product `remove_ads` is active at USD 5.99, with one `standard` Buy option,
  Google-generated regional prices, and multi-quantity disabled.
- Repository variables `PLAY_BILLING_PUBLIC_KEY` (RSA-2048 public key) and
  `AD_FREE_PRODUCT_ID=remove_ads` are configured. The existing Media Manager
  Internal Testers list is enabled for license testing with `RESPOND_NORMALLY`.
- Android version name **1.0.0**, version code **1000008**, includes the configured
  key and the fix allowing Settings, purchase and restore before connecting a
  library. Settings does not start library scans or provider requests.
- [Play release build 40926073208](https://github.com/exenreco/Media-Manager/actions/runs/40926073208)
  passed key validation, generated release configuration checks, bridge tests,
  native receipt tests, bundle and APK packaging, ABI/alignment checks and symbol
  export. APK and AAB were signed locally with the existing upload identity and
  their embedded public key was verified.
- [Android validation 40926078786](https://github.com/exenreco/Media-Manager/actions/runs/40926078786)
  passed ARM64 and x86_64 builds, native receipt tests, emulator launch and H.264
  8-bit/10-bit inline playback. The navigation-policy regression test passed
  locally; formatting and diff checks passed.

### Live Google Play license-test results

Tested in the API 35 Google Play emulator using the x86_64 debug APK from the
same source revision as the signed release packages:

| Check | Result |
| --- | --- |
| Settings opens without a media folder | Passed |
| Product lookup and localized USD 5.99 price | Passed |
| Checkout uses Test card, always approves, with no charge | Passed |
| Cancel checkout leaves the app unowned | Passed |
| Successful test purchase displays Ads removed | Passed |
| Receipt verifies against the configured Google Play RSA public key | Passed |
| Package/product, PURCHASED state and quantity 1 | Passed |
| Google Play acknowledgement after purchase refresh | Passed |
| Clear only test-app data: cached receipt absent | Passed |
| Fresh launch restores signed, acknowledged entitlement from Play | Passed |
| Restore purchases button retains verified ad-free ownership | Passed |

The test receipt and purchase token were not exported or logged. Local evidence
contains screenshots and validation flags only under `dist/play-billing/live-tests`.
The signed packages and matching native symbols are under
`dist/play-billing/1000008`. The new bundle is an **internal testing** release;
this does not publish the app to production.

### Refund and pending-payment follow-up

All transactions used no-charge license-test instruments on the same emulator
and sideloaded debug build, rather than a Play-installed signed release.

| Check | Observed result |
| --- | --- |
| Refund and revoke | Console confirmed Refunded. App retained cached ownership through Restore and restarts; an approved Store data reset restored unowned status with no receipt. Automatic revocation did not pass. |
| Slow decline | No paid entitlement while pending, including after app restart. Console confirmed Canceled. App remained pending until an approved Store data reset. Automatic cancellation reconciliation did not pass. |
| Slow approval | No entitlement while pending. Console then confirmed Processed, but Restore and a Store process restart still returned pending with no verified receipt. Final delivery and acknowledgement remain unresolved. |

Evidence under `dist/play-billing/live-tests` includes
`pending-test-final-orders.png`, `test-order-refunded.png`,
`slow-decline-pending-after-restart.jpg`, `slow-decline-final-unowned.jpg`,
`slow-approve-checkout.jpg`, `slow-approve-pending-unowned.jpg` and
`slow-approve-processed-client-still-pending.jpg`. No raw receipts or tokens
were exported.

**Release limitation:** withholding entitlement before payment passed, but timely
synchronization did not. Refund and pending acceptance are not fully passed.
Investigate Play caching and repeat on a physical device with the Play-installed
release. The client-only verifier cannot independently establish current server
ownership when Play returns stale signed purchases. Backend purchase verification
and real-time developer notifications would provide stronger reconciliation.
Store data resets are a test workaround, not an acceptable user recovery flow.

Connectivity loss during acknowledgement, offline restart and physical-device
Play-installed release checks also remain outstanding.

### Synchronization correction (awaiting live payment validation)

The Android listener now processes verified receipts from `onPurchasesUpdated`
directly instead of discarding them and relying exclusively on another ownership
query. An in-flight query started before that event cannot undo it. A stale
PENDING receipt for the identical token cannot regress a previously verified
PURCHASED receipt; an empty successful ownership query still revokes ownership.

While the activity is foregrounded, pending ownership refreshes every 15 seconds,
and settled ownership refreshes every five minutes. Resume and Restore refresh
immediately. Transient failures retry, and a stalled query releases its busy state
after 30 seconds so Restore cannot remain locked indefinitely. Pausing or closing
the activity stops polling. Successful acknowledgement schedules a receipt refresh.
These changes do not bypass signature verification or grant pending payments.

Native regression tests cover stale-query ordering and same-token pending
regression. The previous live-test results above describe the old build and are
not proof that this correction passed live billing tests. Client polling cannot
force Google Play to invalidate its cache; independent refund reconciliation
still requires an authenticated server-side Google Play Developer API integration.

The 28 local billing/advertising bridge tests passed. Both Android architectures
compiled, and the native receipt/signature and synchronization tests passed in
[validation run 40956901227](https://github.com/exenreco/Media-Manager/actions/runs/40956901227).
The CI emulator launch and H.264 8-bit/10-bit inline playback checks also passed.
The verified x86_64 APK is saved locally at
`dist/play-billing/sync-fix-emulator/Media-Manager-emulator.apk`. A data-preserving
update was rejected because the CI debug signing identity differs. After explicit
user approval, the test app was replaced and successfully launched. Play Store
data and the Google account were preserved. Settings loaded the localized offer,
but Play still returned the previous pending transaction with no verified paid
receipt, even with fresh Media Manager app data. This is not a live payment pass;
the stale Store transaction remains unresolved pending independent verification.
Evidence: `dist/play-billing/sync-fix-emulator/local-billing-still-pending.jpg`.

### Requirements for independent server reconciliation

A Play Console developer account alone does not host a verification service.
The owner currently has no hosted verification endpoint. Completing this part
requires the following external setup (not provisioned by the client correction):

1. Create or select a Google Cloud project and enable the Google Play Developer API.
2. Create a dedicated server identity. Grant it access only to Media Manager in
   Play Console, with the purchase permissions required by the API. Prefer an
   attached service account on the hosting platform over exporting a JSON key.
3. Host an HTTPS verifier that validates the app/product and checks purchase
   state with Google. Only PURCHASED grants ownership; pending, canceled and
   revoked purchases do not. Acknowledgement must be idempotent.
4. Authenticate requests and bind verified purchases to the entitlement model;
   apply rate limits and redact receipts, tokens and credentials from logs.
   Never trust a client-supplied ownership flag or ship service credentials in an APK.
5. Configure authenticated real-time developer notifications, verify their
   identity, and query Google for current purchase state after each notification.
   Notifications alone are not proof of ownership. Deduplicate events and handle
   out-of-order delivery.
6. Add the HTTPS integration to Android and repeat approve, decline, refund,
   restore and offline tests before promoting the build.

Cloud project `portfolio-457414` has the Google Play Android Developer API enabled.
With explicit owner approval, the keyless service account
`media-manager-play-verifier@portfolio-457414.iam.gserviceaccount.com` was created
without project-wide IAM roles or downloaded keys. Console confirmed Enabled and
No keys. Evidence: `dist/play-billing/cloud-setup/service-account-created.png`.
With explicit owner approval, Play Console access was granted only for
`com.tvshowmanager.app`: View app information (read-only), its included app-quality
read access, View financial data, and Manage orders and subscriptions. Console
confirmed the verifier account Active, with no access expiry. No account-wide,
admin, store-editing or release permissions were selected. The financial/order
permissions include viewing sales data, issuing refunds and canceling subscriptions.
Evidence: `dist/play-billing/cloud-setup/play-verifier-access-active.png`.
HTTPS deployment and Android server integration remain unconfigured;
an Active permission grant is not proof that a server API request has succeeded.
An alternative backend is now implemented locally as the bundled Colossal CMS
plugin `plugins/media-manager-play` in the adjacent Colossal CMS repository. It
reuses the CMS site's HTTPS origin, verifies signed receipts against fresh Google
ProductPurchaseV2 state, acknowledges completed purchases and authenticates Pub/Sub
notifications. It is disabled by default and has not been deployed. The CMS host
still needs scoped workload identity federation; no private-key fallback was added.
The existing Android app does not yet call this verifier. See the plugin README
for its native integration protocol and limitations before rebuilding Android.
The Cloud console reports the free trial has ended; no upgrade was performed.
The configured Play licensing public key is for receipt signature verification;
it is not a server API credential.

Official setup: [Google Play Developer API](https://developers.google.com/android-publisher/getting_started),
[purchase security](https://developer.android.com/google/play/billing/security),
[real-time notifications](https://developer.android.com/google/play/billing/rtdn-reference).
