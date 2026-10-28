# Media Manager

The application's backend and frontend are implemented in this Rust workspace.
It is an initial migration milestone, **not a completed five-platform release**.

## Architecture

- `crates/core`: shared models, TVMaze/TMDB/OMDb/custom metadata clients, guide parsing,
  natural file ordering, manual titles, batch templates, movie naming, all-season
  matching, missing episode reporting, rename previews, and Plex folder naming.
- `crates/app`: Dioxus 0.7 Rust UI with responsive CSS and local/remote backends.
  Dioxus uses platform WebViews; the app's UI logic and components are Rust.
- `crates/server`: Axum service for controlling a selected media folder remotely.
  This is optional for local desktop operation. The server holds API keys only
  in memory; client-side saved credentials are handled as described below.

## Status

| Platform | Local library | Remote library | Validation in this workspace |
| --- | --- | --- | --- |
| Windows | Implemented | Implemented | Workspace compilation and backend tests pass |
| macOS | Implemented in shared source | Implemented | Native ARM64/x64 CI builds/tests pass; UI acceptance pending |
| Linux | Implemented in shared source | Implemented | Native x64 CI build/tests pass; UI acceptance pending |
| Android | Native document-tree bridge in source | Implemented in shared source | ARM64 preview APK built in CI; device acceptance pending |
| iOS | Native security-scoped folder bridge in source | Implemented in shared source | ARM64 Simulator archive built/signed in CI; device signing/testing pending |

For local mobile libraries, open **Welcome → Local**, then **Choose media folder**.
Android opens the system document-tree picker and keeps the granted permission.
iOS opens Files and saves a bookmark for the selected folder. Select a folder
again if access is revoked. Preview scans names without copying media, and
native provider operations apply the reviewed renames. The current working source
streams local and remote media into the built-in preview player. Older releases
use an Android video player or iOS Quick Look. Poster writes preserve existing images.

Android restricts which roots can be selected (including Android/data and
Android/obb); choose a media subfolder rather than the storage root. Provider
support for renaming and writing varies. Read-only providers fail explicitly.
Scan limits are 20,000 entries and 32 folder levels. Renames can partially
succeed; inspect results before retrying. These native bridges compiled in Android/iOS CI and are included in v0.1.3
previews. Device/provider validation is still pending.
Remote server mode remains available.

## Run the desktop UI

The app uses an original matte TV/play icon and transparent desktop startup
splash. The splash closes automatically or via **Open library →**. Design
masters, reference notes and generation prompts are in
[`assets/branding/`](../assets/branding/README.md).

Install stable Rust with the platform's native build prerequisites. On Windows,
install Visual Studio C++ build tools, the Windows SDK and WebView2 runtime.
On Linux, install GTK 3, WebKitGTK 4.1, libxdo and app-indicator development packages.
On macOS, install Xcode command-line tools.

From the project root:

```sh
cargo run -p tv-manager-app
```

Choose a media root, optionally enter a subfolder, set the title and episode
source in Workspace. The preview refreshes automatically after a short pause in
edits; inspect the mapping in Library and apply the plan. Pairing follows
natural filename order; it does not infer missing episode numbers from filenames.
Single-season API results retain their original episode numbers, including gaps.
Movie titles and optional years can be entered manually or matched using TMDB,
OMDb, or a TMDB-compatible custom provider.

Show previews are grouped into collapsible season sections in numeric order.
The season field is optional: leave it blank to include all seasons from folder
names, or enter a number to select one season. Manual titles and batch numbering
use Season 1 when the field is blank.
Each section includes existing files and missing episodes, including seasons
with no matching files. Files without a detected season appear in an
"Unassigned files" section. Movies remain in one section. Rebuild the server
alongside the app when upgrading so remote previews include season metadata.

Each season table includes an **Actions** column. In the current working source,
click a row, pencil, or play icon to select a file. The right inspector automatically
plays supported videos, initially muted. The title field below the player updates
the proposed filename while keeping its extension. **Save title** recalculates
conflicts; the file is changed only after applying the rename plan. Folders and
missing files have no edit/play actions. Non-video companions can be edited but
have no video preview. Decoder support depends on the platform WebView.

## Workspace navigation (working source)

Desktop uses a custom draggable title bar, with minimize, maximize/restore, close,
and resize edges. A global TV Shows / Movies toggle sits in the title bar before
the window controls and applies across all workspace pages. The icon rail expands on hover or keyboard focus. Its views are
Get Started, Details, Library, Health, and Settings. The ordering icon beside the Library search bar opens a popup containing naming
format, numbering, custom episode mapping, companion-file settings, and saved
naming profiles. Profiles appear below custom episode mapping. Configuration occupies the main area only when that view is selected.
Welcome appears after the splash and contains an API settings panel above the
Local/Remote connection tabs. Open workspace leads to TV Show Details or Movie
Details, depending on the active type. Changes refresh the preview asynchronously
without leaving the page; superseded requests are cancelled.
Local contains folder settings; Remote contains server settings and QR pairing.
Open workspace verifies access first. Missing or invalid connections show a
top-center animated confirmation notice with Set up connection and Not now.
Workspace tools are not opened until connection validation succeeds.
Library contains **Library** and **Art Work** tabs. Library keeps the media
table; Art Work contains a search bar and responsive grid of main, season, and
alternate posters, alongside download settings and poster destination controls.
The tabs sit above the page heading; Art Work displays the heading “Art work”. Artwork has no separate navigation item. The right
inspector opens only when a media item is selected and contains file playback
and title editing; drag its
divider, use arrow keys while the divider is focused, or double-click to reset it.
The rename toolbar stays below the library table.

Library health opens a separate dashboard without the media table or rename
toolbar. The current preview is scanned automatically while this page is open.
Charts show video matching and counts of missing episodes, unmatched videos,
filename conflicts, missing artwork, and exact duplicate groups. Results cover
the current preview, not every folder on the device. Rename history and Undo
controls are directly accessible on Settings, below Health in navigation.

At 760 CSS pixels or narrower, a floating bottom capsule replaces the icon rail.
Its plus button expands into a tool grid; X or Escape closes the grid. Welcome,
Workspace and Library remain directly accessible from the capsule.
Selecting a file or artwork opens its preview. On Android/iOS and compact desktop
windows (1050 CSS pixels wide or less, or 650 pixels high or less), previews use a
rounded black bottom sheet above the library. Drag its handle down, select Back
or Close, click the dimmed backdrop, or press Escape to return. Short drags snap
back. The sheet scrolls independently; keyboard focus stays inside until it closes.
Large desktop windows retain the inline right sidebar. Resizing between these
layouts keeps the selected item. Motion respects the system reduced-motion
preference. The sheet change requires a new build after the 1.0.0 beta release.

## Poster preview and downloads

The Library page’s Art Work tab lists available posters in a responsive grid.
The search bar filters the current preview’s artwork by title or season; it does
not perform a separate provider search. Local posters appear in color; when a
local image is missing, its provider poster appears grayed out. Clicking a poster
only changes the selection and opens the right sidebar, without rebuilding the
preview. The sidebar compares existing and selected artwork side by side.
**Save selected poster** is an explicit action; enable **Replace existing poster**
to overwrite an existing image. A changed file blocks replacement until refreshed.
Provider alternatives can be previewed for a show/season destination chosen
using **Preview alternate artwork for** in the selection sidebar.
Building a rename preview loads the show's poster and season poster thumbnails.
Missing artwork and image-load errors retain the placeholder. TVMaze and
TVMaze-compatible sources supply show and season artwork when available; OMDb
supplies its show poster. Manual titles, batch sequences and scraped guides do
not supply artwork. Movies can use TMDB, OMDb, or a custom movie API to match titles and posters.
The provider integration follows the [TVMaze API](https://www.tvmaze.com/api)
and [OMDb API](https://www.omdbapi.com/) response fields.

Use **Save selected poster** in the sidebar to download an image to the device
or server holding the library. Poster saving is independent of renaming; the
Download posters checkbox has been removed. The fixed Art Work toolbar
provides **Download all posters** to save every available missing show/season
poster, independently of the search filter. Existing local images are kept;
provider alternatives are not downloaded as extra duplicate files. The toolbar
reports saved, kept, and failed counts. Existing images require explicit
replacement confirmation. The show image is saved as `poster.jpg` (or `.png` /
`.webp`), and season images as `poster` in their matching season folders. If a
season folder is absent, its image is saved as `season01-poster.jpg` in the
selected root. Selecting a season folder itself uses `show-poster` for the show
image and `poster` for that season. Plex folder moves are accounted for before
saving. Only HTTPS JPEG/PNG/WebP images up to 10 MB are accepted; downloads do not
overwrite files or follow redirects. Rebuild both app and server for this feature.

## Resume the previous workspace

Settings are saved automatically after changes, with a short debounce and a
final save during normal shutdown. After a workspace has been used, the next
launch asks **Pick up where you left off?** after the splash screen. Settings
are not loaded into the controls until **Resume workspace** is selected.
**Start fresh** discards the saved workspace and keeps the default controls.
Simply opening a fresh installation without changing anything does not create
a resume prompt.

The saved workspace includes the library root/subfolder or remote connection,
data source, naming fields and templates, manual titles, season selection,
poster preferences, and side-panel widths. If a preview was previously open,
resuming fetches a fresh preview. Saved rename tasks are never replayed, and
resuming does not rename files or download posters. An unavailable folder or
server leaves the restored settings available with a refresh error.

On Windows the entire settings file, including API keys and bearer tokens, is
encrypted with DPAPI for the current Windows user. It is stored at
`%LOCALAPPDATA%\MediaManager\workspace.bin`. macOS uses
`~/Library/Application Support/MediaManager/workspace.json`; Linux uses
`$XDG_STATE_HOME/MediaManager/workspace.json` or
`~/.local/state/MediaManager/workspace.json`. On these platforms credential
fields are omitted and must be re-entered; settings files are written through
private temporary files. `MEDIA_MANAGER_STATE_DIR` overrides the storage folder
for isolated testing. Corrupt settings are reported without loading them, and
atomic writes preserve the previous workspace if a save fails.

This feature starts saving settings with this version; earlier versions did
not persist a workspace to recover.

The desktop and mobile feature sets are separate:

```sh
cargo check -p tv-manager-app --no-default-features --features mobile --target aarch64-linux-android
cargo check -p tv-manager-app --no-default-features --features mobile --target aarch64-apple-ios
```

Install those Rust targets and configure the native SDK/toolchain first. Use the
Dioxus CLI on the appropriate host for development and packaging; Rust target
checks alone do not generate installable mobile applications. See the official
[Dioxus mobile tooling guide](https://dioxuslabs.com/learn/0.7/guides/platforms/mobile/)
and [platform setup guide](https://dioxuslabs.com/learn/0.7/getting_started/).
The iOS build must run on macOS with Xcode. App identifiers, icons, store signing
and distribution credentials still need to be finalized before release.

## Run the remote backend

Example in PowerShell, from this directory:

```powershell
$env:TV_MANAGER_ROOT = 'D:\Media'
$env:TV_MANAGER_TOKEN = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
cargo run -p tv-manager-server
```

The generated token remains in the current shell environment. Supply that token
to the UI's remote connection settings. The service defaults to
`127.0.0.1:8787`. `TV_MANAGER_BIND` can override the listen address.
For a phone or another computer, place it behind an HTTPS reverse proxy and
connect to that HTTPS URL. The service does not terminate TLS itself. The UI
allows plain HTTP only for loopback development. Do not transmit bearer tokens
over an unencrypted LAN/public connection.

The token grants administrative access to this media root, including metadata
requests from the server. This is a single-owner service, not a multi-tenant
hosting system. Use a trusted root; do not allow untrusted local processes to
rearrange its directories during operations. Path checks reject traversal and
symlink ancestors but are not a defense against adversarial filesystem races.

| Route | Purpose |
| --- | --- |
| `GET /v1/health` | Authenticated version/health check |
| `POST /v1/plans` | Preview using a serialized `PlanRequest` |
| `POST /v1/plans/{id}/apply` | Apply the server-held preview once |
| `PATCH /v1/plans/{id}/tasks/{index}` | Edit a proposed filename using `{"filename":"New name.mp4"}` |
| `POST /v1/plans/{id}/tasks/{index}/play` | Issue a temporary playback ticket |
| `GET /v1/media/{ticket}` | Stream ticket-authorized media with HTTP byte ranges |

Control routes require `Authorization: Bearer <token>`. Media streams use an
unguessable temporary ticket instead of forwarding server credentials to the
player. Links expire with their ticket/preview after at most 15 minutes; press
Play again or refresh the preview when needed. Up to 64 tickets and 32 completed
plans are retained temporarily for playback after applying. Apply accepts a plan ID;
the server does not trust client-supplied rename tasks. Previews expire after
15 minutes, are capped at 32 pending plans, and disappear on server restart.
One preview/apply operation runs at a time. Relative paths are resolved under
the configured root. There is no filesystem browser endpoint in this milestone.

## Rename behavior

- Supported file extensions: `.mp4`, `.mkv`, `.avi`, `.txt`.
- Existing destinations and duplicate planned destinations block applying a plan.
- Before applying, the backend rechecks source type, file size and modification
  time, folder scope and destination existence. This is not a content checksum.
- Files use atomic no-replace OS rename APIs: `MoveFileExW` on Windows,
  `renameat2(RENAME_NOREPLACE)` on Linux, and `renamex_np(RENAME_EXCL)` on Apple.
  Unsupported filesystems return errors without falling back to an overwrite.
- Files run first; Plex season folder renames run deepest-first. Table paths show
  the paths used for each operation, before any ancestor folder rename.
- Execution is not a transaction. A batch can partially succeed and reports
  per-entry results. Completed operations are journaled. Use Library health & rename history to undo names with conflict checks; failed batches do not roll back automatically.
  If a remote request is interrupted, inspect the library or build a fresh preview.
- Templates support `{Series Name}`, `{Season}`, `{Season:02d}`, `{Episode}` and
  `{Episode:02d}`. Unknown tokens are rejected. Manual/batch input uses one season;
  all-season matching requires folder season names and API/guide season metadata.
- Guide scraping is best-effort table parsing. JavaScript-only or protected
  pages need manual title input. Empty title lines are ignored.

## Validation

```sh
cargo check --workspace --locked
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Tests use temporary libraries, never a user's media folder. They cover naming,
episode gaps, ordering, collisions, changed sources, no-op files, nested season
folders, scraping, auth, root escape, remote preview/apply and replay prevention.
GUI interactions still require visual acceptance tests. An opt-in live TVMaze
poster lookup/download integration test also passes and can be run explicitly:

```sh
cargo test -p tv-manager-server live_tvmaze_poster_preview_and_download -- --ignored
```
Desktop CI configuration is in `.github/workflows/rust.yml`; it must
run successfully before claiming macOS or Linux validation.

The Windows app and server executables were also built successfully. All 29
offline tests (21 backend and 8 frontend/persistence) pass in a normal host process, plus the
opt-in live poster test. The restricted Windows execution
sandbox denies the native rename call in its redirected temporary directory;
run filesystem integration tests in a normal development shell. Formatting and
Clippy with warnings denied pass as well. Basic Windows layout interactions were visually checked; full native acceptance testing remains pending.

For this checkout, Rust was installed under the project's ignored `.tools`
directory without modifying the user's PATH. PowerShell can use it as follows:

```powershell
$env:CARGO_HOME = Join-Path (Get-Location) '.tools\cargo'
$env:RUSTUP_HOME = Join-Path (Get-Location) '.tools\rustup'
& "$env:CARGO_HOME\bin\cargo.exe" test -p tv-manager-core -p tv-manager-server
```

## Remaining migration work

Android Native advanced AdMob source integration is configured with the
publisher's supplied IDs. Debug builds select Google's native test unit.
See [Android advertising](../crates/app/android/README.md) for build requirements,
publisher setup and the outstanding Android compilation/device checks. Ads are
Android-only; it does not add ads to desktop. The separate
[iOS advertising integration](../crates/app/ios/README.md) uses the supplied iOS
IDs and a native Swift SDK bridge, with debug test ads. Its Mac/Xcode build,
SDK embedding/signing and simulator/device validation remain pending.

### Notifications and background operations

The desktop app sends a native system notification after an apply operation
returns its rename and poster results, including failure counts. Windows uses
the Media Manager application identity registered for the current user.
Notification delivery is best effort and respects system notification settings;
an unavailable notification service does not change the rename result.
The initial WebView document has a transparent canvas before the splash mounts.

Remote apply operations run in a server-owned task, retaining the filesystem
gate through both renames and poster downloads even if the HTTP client disconnects.
The server must remain running. This does not keep a phone process alive or
provide completion recovery after a disconnected request.

Android/iOS native background execution and mobile notifications are **not yet
implemented**. They require packaged native bridges, notification permissions,
Android foreground-service/work scheduling, iOS OS-managed background tasks,
and device validation. An ordinary Dioxus UI task cannot guarantee execution
while the mobile OS suspends or terminates the app.

1. Implement and test Android and iOS document-provider bridges for local mode.
2. Build and test remote mode on mobile devices, including network permissions,
   backgrounding, interrupted operations and HTTPS certificate handling.
3. Run macOS/Linux CI and native UI acceptance tests on all five platforms.
4. Add platform packaging, app icons, signing and release installers.
5. Complete acceptance tests for accessibility, preview tree layout and settings
   interactions.

This preview is not ready for a five-platform production release until these
steps pass.

For title matching, anime numbering, companion files, Undo, artwork sources,
health scans, profiles, and QR pairing, see [Library tools](LIBRARY_TOOLS.md).


## Movie metadata providers

In **Get Started → API settings**, select **Movies** in the title bar, then choose
**TMDB**, **OMDb**, **Custom movie API (TMDB-compatible)**, or manual title entry.
TMDB requires a v3 API key. Enter it in **TMDB API key**, then use **Find matching
titles** on Details. Searches include the optional year; selecting a card pins
the movie ID and loads the title, release year, main poster, and available
alternate posters. Missing images remain placeholders.

Custom movie APIs use an HTTPS base URL without embedded credentials or query
parameters. An optional key is sent as `api_key`. The contract is:

- `GET {base}/search/movie?query={title}&year={optional-year}&include_adult=false`
  returns `{"results":[{"id":123,"title":"Example","release_date":"2020-01-01","poster_path":"https://images.example/poster.jpg"}]}`.
- `GET {base}/movie/123?append_to_response=images` returns the same movie fields,
  optionally with `"images":{"posters":[{"file_path":"https://images.example/alternate.jpg"}]}`.
- IDs are numeric. Image references may be full HTTPS URLs or TMDB image paths.
  TV mode continues to use the existing TVMaze-compatible custom contract.

API details: [TMDB movie search](https://developer.themoviedb.org/reference/search-movie),
[details](https://developer.themoviedb.org/reference/movie-details), and
[image URLs](https://developer.themoviedb.org/docs/image-basics).
Rebuild the backend alongside the app for remote TMDB/custom movie support.


### Optional ad-free time on mobile

In Settings, choose **Watch an ad — 30 minutes without ads** to watch an optional rewarded ad. Once the ad confirms that the reward was earned, all ads are paused on that device for 30 minutes, including after restarting the app. Closing before earning the reward does not activate ad-free time. If no ad is ready, try again later; your library tools remain available.

Mobile apps can show an occasional fullscreen advertisement after a successful rename batch. Ads never delay file operations, and unavailable opportunities are skipped.
