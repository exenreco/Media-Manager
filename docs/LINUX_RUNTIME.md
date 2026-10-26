# Managed Linux runtime

The Flatpak download runs Media Manager against **GNOME Platform 49**, which
provides GTK 3 and WebKitGTK 4.1, plus its matching graphics, GLib, networking and
multimedia stack. It does not load the host distribution's GTK/WebKitGTK libraries.
The existing TAR.GZ is still available for users who manage native dependencies.

## Install and run

Install Flatpak using your Linux distribution's package manager, then download
the `.flatpak` asset from the project's GitHub release:

```sh
flatpak install --user ./Media-Manager-0.1.3-linux-x86_64.flatpak
flatpak run com.tvshowmanager.app
flatpak update --user
```

The bundle identifies Flathub as its runtime source. First installation needs
network access to install the GNOME runtime; the application bundle does not
contain the entire runtime. Runtime updates are managed by Flatpak. App updates
currently require installing the newer GitHub release bundle; this is not a
Flathub app listing or an automatically updated app repository.

The app has read/write access to host media folders because its existing local
renamer and playback use normal filesystem paths. The native folder chooser uses
GTK portals when available. Network access supports metadata, posters and remote
libraries; Wayland/X11, audio, GPU and notifications permissions support the UI.
The optional server can run with:

```sh
flatpak run --command=media-manager-server com.tvshowmanager.app
```

Configure its existing root/token environment settings before exposing it.

## Build and validation

Install `org.gnome.Platform//49` and `org.gnome.Sdk//49` from Flathub. Build the
app/server on Ubuntu 24.04 and run `python scripts/package_flatpak.py --version
0.1.3`. The helper checks dynamic dependencies inside the **runtime**, not merely
the SDK, and packages the small app-specific libraries absent from that runtime.
GTK, WebKitGTK, libc and graphics drivers are never copied from Ubuntu.

The release job tests startup with Xvfb inside the real Flatpak runtime before
uploading the bundle and checksum. This tests startup and dependency resolution,
not every GPU, desktop environment or media codec. Runtime-library copyright and
corresponding source files accompany the release separately.

References: [GNOME 49 runtime components](https://github.com/GNOME/gnome-build-meta/blob/gnome-49/elements/sdk-platform.bst),
[Flatpak runtimes](https://docs.flatpak.org/en/latest/available-runtimes.html),
[single-file bundle dependencies](https://docs.flatpak.org/en/latest/single-file-bundles.html).
