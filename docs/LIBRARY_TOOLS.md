# Library tools

These features are included in the v0.1.4 preview. Desktop compilation, Rust integration tests, and JS bridge tests
are checked locally; Android/iOS native builds and emulator/simulator launch checks have passed;
physical-device and full feature acceptance tests remain pending.

## Match a title

Enter the title and optional premiere year, choose TVMaze, OMDb, or a compatible
custom API, then use **Find matching titles**. Results show the provider, title,
year, type, and available poster. Select a result to bind the preview to its provider
ID. Changing the title, year, or source clears the selection. OMDb requires your API
key and also supports movie matching. Manual movie naming remains available.

## Anime numbering and alternate orders

**Sequential files in each season** preserves the existing natural-sort behavior.
**Read SxxExx from filenames** uses the numbers in the name and skips unrecognized
files. **Anime absolute episode numbers** maps filenames such as `001.mkv`,
`One Piece - 003.mkv`, or `EP003.mkv` to regular episodes in provider order, preserving
gaps. Season-zero specials are excluded from the absolute sequence; explicit
`S00E01` names still match specials. Set the first absolute number if your selected
provider episode list begins at a different absolute number.

Use **Custom episode order / specials mapping** for DVD, anime arc, or other orders
that differ from the provider. Each line is `relative/path.mkv|season|episode`.
Mappings take precedence. This does not invent a universal mapping between TVMaze,
TMDB, aired, DVD, or absolute orders; inspect the preview before applying.

## Companion files

Enable the companion-file option to rename `.srt`, `.ass`, `.ssa`, `.vtt`, `.sub`,
`.idx`, `.mka`, `.aac`, `.flac`, and `.nfo` files sharing a video's original basename.
Language/flag suffixes are retained, e.g. `01.en.forced.srt`. Manually editing the
video destination also updates its planned companions. Destination conflicts block
the batch. NFO contents are retained; metadata is not rewritten or generated.

## History and Undo

After applying, expand **Library health & rename history** in the preview column,
load history, select **Undo**, and confirm restoration. Desktop/server journals are
stored under `.media-manager/history` in the selected folder. Mobile journals live
in app-private persistent storage and are associated with the selected provider
folder and subfolder. Keep these records if you need Undo after restarting.

Each operation is recorded before it runs. Undo reverses folder moves before file
moves, checks original-name conflicts and recorded file size/modification times and
directory contents, and never overwrites another file. It stops on changed/missing
files or unsupported provider operations; earlier restored names remain restored.
Reload history to resume. Downloaded artwork is kept. This is not a backup of media
contents and cannot recover deleted or edited media. A removed/reselected mobile
folder grant may require using the original grant or rebuilding the preview.

## Artwork sources

TVMaze offers available alternate posters. An optional **TMDB artwork API key** adds
a second source, linked using the matched show's IMDb ID. If no verified link exists,
the app reports that instead of mixing artwork from similarly named shows.

**Choose alternate artwork** below the poster preview lets you select the target
main/season poster and an image. The preview is rebuilt with that choice; downloads
still occur only when applying with **Download posters** enabled. Existing images
are preserved. TMDB automatically fills missing seasons only when the season number
and episode count agree; use an explicit selection for different season orders.
Provider outages or unavailable artwork do not invent images or episode metadata.
TVMaze works without a key; TMDB and OMDb features need your own credentials.

## Preview health

The health scan covers the videos in the current preview and reports missing
episodes, unmatched videos, rename conflicts, and missing provider artwork. It
compares SHA-256 content hashes for nonempty equal-size files to identify exact
duplicates; it never deletes them. Large duplicate groups can take time to read.
This is a selected-show preview report, not automatic multi-show identification of
an entire mixed library. Rebuild the preview after renaming or changing settings.

## Naming profiles

Save named presets for None/Plex/Jellyfin naming, movie/show mode, numbering mode,
absolute offset, companion behavior, and batch templates. Loading a preset changes
settings; rebuild the preview before applying. Profiles exclude library paths,
provider IDs, per-library episode mappings, artwork overrides, and API/server keys.
They persist in the app settings directory as `naming-profiles.json`.

## QR device pairing

The camera/local-host flow below is in the current source; existing v0.1.4 downloads
still use image-import pairing with a separately configured HTTPS backend.

On desktop, **Pair a mobile device** offers **Local network** or **Remote HTTPS server**.
Local mode uses the selected media root and detected LAN IP (editable for multi-adapter
PCs). **Generate pairing QR** starts an encrypted library server bound to that IP on
an available port. Keep the desktop app open and phone on the same reachable network.
Allow that app through the private-network firewall if needed; no firewall rules are
changed automatically. This mode needs no public domain, certificate installation,
Cloudflare tunnel, or separately launched backend.

The QR carries its version, desktop name, HTTPS endpoint, connection mode, one-use
five-minute code, and (for local mode) the desktop's public certificate. Private keys,
long-lived bearer tokens, and media paths are not in the QR. The phone trusts that
certificate only for its paired connection; system trust stores are unchanged.
The QR disappears when its countdown expires. Generate a fresh code for another phone.
**Stop local sharing**, closing the desktop app, or generating for a different library/IP
stops the previous host. Paired phones lose access and need a fresh QR after restart.

Remote mode uses the HTTPS URL and credentials configured under **Connect your library**.
The remote server must already be running and reachable from the phone.

On Android/iOS, press **Scan desktop QR code**, grant camera access, and align the QR
inside the camera guide. Scanning a valid invitation automatically redeems its code
and connects the phone. The old image-import, JSON-paste, and confirmation controls
are replaced. Camera frames remain on the device. Cancel or a permission failure
returns to the app; an invalid, expired, already-used, or unreachable invitation
reports an error and requires another scan. Scanning authorizes that desktop/server's
library permissions. Camera scans and local-network access still require device testing.


## Validation and next release

The automated suite covers numbering gaps/specials, companion conflicts and edits,
nested-folder Undo after reload, changed-file/conflict refusal, exact duplicates,
profile credential exclusion, QR image round-trip, pairing authentication/expiry/
single-use, and the existing rename/server/mobile-bridge regressions. Native picker,
cloud-provider, and runtime behavior must also be tested on Android and iOS devices.
No paid tier, purchase flow, subscription, or new ad policy is introduced here.

See [Mobile validation](MOBILE_VALIDATION.md) for the emulator/simulator workflow,
launch evidence helper, disposable fixture generator, and unrun physical-device matrix.
