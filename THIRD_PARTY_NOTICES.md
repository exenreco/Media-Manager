# Third-party notices

Media Manager's original source and generated branding artwork are distributed
under the Apache License 2.0 in LICENSE. Dependencies retain their respective licenses;
this license does not relicense third-party libraries, SDKs, artwork or provider data.

- Rust dependency versions and sources are recorded in Cargo.lock. Dioxus,
  Tokio, Axum, reqwest and their dependencies carry their own notices.
- Google Mobile Ads and User Messaging Platform SDKs are governed by Google's
  SDK licenses and AdMob terms. Their frameworks and bundled privacy manifests
  must be preserved in mobile distributions. Public AdMob app/ad-unit IDs are
  configuration identifiers, not authentication secrets.
- TVMaze data/artwork is subject to the [TVMaze API terms](https://www.tvmaze.com/api).
  OMDb data/artwork is subject to the [OMDb terms](https://www.omdbapi.com/legal.htm).
  Provider artwork is fetched at runtime and is not part of the app's original art.
- The third party design-reference images are third-party works.
  Local copies under assets/branding/references are excluded from Git and releases.
  Branding notes remain in assets/branding/README.md.

Do not include developer credentials, signing keys or personal library data in
source or binary distributions.
