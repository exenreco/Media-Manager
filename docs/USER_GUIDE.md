# Media Manager

The application's backend and frontend are implemented in this Rust workspace.
It is an initial migration milestone, **not a completed five-platform release**.

## Architecture

- `crates/core`: shared models, TVMaze/OMDb/custom metadata clients, guide parsing,
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
| Android | **Pending native folder/document bridge** | Implemented in shared source | ARM64 preview APK built in CI; device acceptance pending |
| iOS | **Pending native document bridge** | Implemented in shared source | ARM64 Simulator archive built/signed in CI; device signing/testing pending |

Local mobile mode deliberately reports that document access is unavailable.
It does not claim to rename arbitrary mobile paths. The requirement is to support
**both** device-local files and remote libraries on mobile; that requirement is
not finished yet. Android needs Storage Access Framework tree grants and
DocumentsContract operations. iOS needs document picking, security-scoped access
and coordinated file operations. These must use provider document identifiers,
not pretend that content URIs are normal filesystem paths.

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
source, build a preview, inspect the mapping and apply the plan. Pairing follows
natural filename order; it does not infer missing episode numbers from filenames.
Single-season API results retain their original episode numbers, including gaps.
Movie title/year are entered manually, matching the original app's movie flow.

Show previews are grouped into collapsible season sections in numeric order.
The season field is optional: leave it blank to include all seasons from folder
names, or enter a number to select one season. Manual titles and batch numbering
use Season 1 when the field is blank.
Each section includes existing files and missing episodes, including seasons
with no matching files. Files without a detected season appear in an
"Unassigned files" section. Movies remain in one section. Rebuild the server
alongside the app when upgrading so remote previews include season metadata.

Each season table includes an **Actions** column. The pencil edits a file's
proposed filename; it does not rename immediately. The extension is preserved,
invalid/path filenames are rejected, and collisions are recalculated for the
whole preview. Edited names use the normal confirmation flow. Edits belong to
the current preview and are not saved as reusable settings. The play icon opens
local videos in the system's default player. Remote videos open in an in-app
player using a temporary streaming link with seek support; MP4/MKV/AVI decoding
depends on device codec support. Copy the link into a compatible network player
if needed. Playback follows applied file and Plex folder renames. Folders and
missing files have no actions; text files can be edited but cannot be played.
Build a fresh preview to edit after applying.

## Poster preview and downloads

The artwork box above "1. Connect your library" starts with a placeholder image.
The dashboard has three independently scrolling columns: posters and naming on
the left, rename previews in the center, and grouped library/API settings on the
right. Both side panels span the full window height, with the logo, title and
tagline inside the left panel. Main panels have square corners, and the bottom
rename toolbar sits under the center preview, between the side panels. Drag the dividers to resize
them, use arrow keys when a divider is focused, or double-click to reset its
width. Widths adapt to preserve room for the preview. At window widths of
900 CSS pixels or less, a gear icon opens a dropdown with Naming, Preview and
Library & API options to switch between full-height panels without horizontal overflow. The rename toolbar
stays at the bottom of the Preview panel while its table scrolls. The main and season
posters share a carousel: previous/next icons or adjacent thumbnails change the
active poster, which is displayed larger. The info icon beside Download posters
toggles the saving explanation.
Building a rename preview loads the show's poster and season poster thumbnails.
Missing artwork and image-load errors retain the placeholder. TVMaze and
TVMaze-compatible sources supply show and season artwork when available; OMDb
supplies its show poster. Manual titles, batch sequences and scraped guides do
not supply artwork. Movie naming remains manual and does not fetch movie posters.
The provider integration follows the [TVMaze API](https://www.tvmaze.com/api)
and [OMDb API](https://www.omdbapi.com/) response fields.

Select **Download posters**, then build a preview to review the download targets.
Images are saved only when you confirm applying the preview, on the device or
server that holds the library. Downloads can run even if all filenames are
already correct. Existing posters are kept; failures appear per poster without
undoing successful renames. The show image is saved as `poster.jpg` (or `.png` /
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
  per-entry results. There is no automatic rollback, durable journal or undo.
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
