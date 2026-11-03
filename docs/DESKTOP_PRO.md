# Desktop Pro

macOS and Windows Settings offer **Go Pro** and **Restore Pro purchase**.
Pro is a permanent, non-consumable entitlement for unlimited distinct movie
and TV show titles. The displayed purchase price comes from the platform store.
It is separate from the iOS `remove_ads` product. iOS, Android and Linux are
unchanged by desktop rename accounting.

## Free allowance

The first distinct title may be renamed repeatedly (including its episodes)
without using another slot. Renaming a second distinct title starts a two-hour
cooldown. The second operation's complete batch is allowed, including all its
episodes; subsequent rename operations wait until the cooldown expires. At
expiry, the allowance resets to two titles. Title identity normalizes case and
whitespace and includes the matched release year.

The desktop backend enforces this before both local and remote rename requests.
Poster-only operations and undo are not charged. A usage file next to the saved
workspace survives restarts and clearing workspace preferences. A cross-process
file lock prevents concurrent app windows from overspending. Reservations are
written before filesystem/network work. Known all-failed results refund the
new title; interrupted/unknown results keep the reservation to avoid retrying
an operation whose files may already have changed. A backward clock change
cannot reset the recorded cooldown. This is local free-tier accounting, not a
server-backed anti-tampering system.

## Store verification

- macOS: `macos/DesktopPro.swift` uses StoreKit 2. Only verified, unrevoked
  `desktop_pro` non-consumables in current entitlements grant Pro. Restore uses
  `AppStore.sync()`. Each rename queries the native store; a UI Boolean cannot
  grant access. Cargo's build script compiles the Swift bridge for the target
  Mac architecture. Xcode is required for macOS builds.
- Windows: `Windows.Services.Store.StoreContext` queries active add-on licenses
  with the in-app offer token `desktop_pro`. Purchase dialogs are bound to the
  app HWND. Product lookup uses associated Durable products and the actual Store
  ID returned by Microsoft. Restore re-queries licenses. Validated existing
  licenses remain usable when store pricing is temporarily unavailable.

## Release setup status (2026-11-02)

Apple product **Desktop Pro**, ID `desktop_pro`, Apple ID `6829836504`, was
created under Media Manager (`6829643613`). Its US base price is **USD 10.99**,
explicitly approved by the owner. English localization and review notes are
saved; availability excludes France, matching the current release decision.
The product is a draft, not approved or available for public purchase. A native
Mac App Store version implementing Pro must be prepared, signed, tested with
the store sandbox, and submitted with this product. Do not attach it to the
existing iOS build, which implements only `remove_ads`.

A macOS platform and version **1.0.2** draft now exist in App Store Connect.
Its description and review notes explain Desktop Pro and the title allowance;
the inherited iOS advertising instructions were replaced for this Mac draft.
The local native Mac app loaded `desktop_pro` from StoreKit and displayed the
store-provided **$10.99** price. No purchase was made. A 1280×800 JPEG of the
actual Settings panel was uploaded for product review after adding the Mac
platform; Apple was processing that asset at the last check.

The owner has no Microsoft Partner Center developer account or Media Manager
Store listing yet. Those must be created by the owner, then the Windows app
must receive its Store identity and a Durable add-on with in-app offer token
`desktop_pro`. Configure the approved base price before publication. The current
installer/portable build is not a published Microsoft Store package. Actual
Windows purchase/restore and Store identity testing require that environment.

Builds show store unavailability honestly when the product cannot be loaded;
they do not fake ownership.
Creating product metadata does not make checkout live. Neither desktop feature
is included in the existing iOS review submission.

See [Apple StoreKit](https://developer.apple.com/storekit/) and
[Microsoft in-app purchases](https://learn.microsoft.com/windows/uwp/monetize/enable-in-app-purchases-of-apps-and-add-ons).

The Windows/macOS Settings page places Go Pro after the App updates section,
separated by a divider beneath the installer safety explanation. The shared interactive app
walkthrough opens each workspace page, highlights real controls, and advances
when the highlighted control is clicked. File, network, and purchase actions
are intercepted during the guide; unavailable controls are skipped. The tour
returns to the starting page when closed. It covers every workspace page and can be replayed using the question
mark beside the titlebar's media type toggle. Completing or skipping the guide
stores a separate `walkthrough-v1` preference in the app's settings directory, so
clearing or resuming the workspace does not reset first-run onboarding.

### Native App Store build 1.0.3

The sandboxed Mac App Store release **1.0.3 (1.0.4)** was signed with the
Mac App Store profile and uploaded successfully on 2026-11-02. The new draft
contains current Settings and interactive-guide screenshots. The same release
executable launched in a sandboxed QA bundle and retrieved the $10.99 product
price. Purchase/restore acceptance and App Review approval are still pending.
See [Apple release 1.0.3](APPLE_RELEASE_1.0.3.md) for artifacts and release status.

### November 3 review update

The exact macOS TestFlight build 1.0.3 (1.0.4) reported verified Pro ownership, completed Restore after Apple authentication, and retained Pro after relaunch. macOS and Desktop Pro are Waiting for Review following the local-pairing network-entitlement explanation. This does not enable purchase in standalone GitHub Mac packages or configure a Microsoft Store identity.
