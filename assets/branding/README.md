# Media Manager visual identity

Original artwork generated with the built-in ImageGen tool.

- `app-icon.png`: 1254 × 1254 RGBA master. Matte folded TV/play mark in mint,
  jade, periwinkle and apricot; transparent exterior and play aperture.
- `splash.png`: 1586 × 992 RGBA master. Floating navy information panel and
  layered cinematic play portal; transparent exterior.
- `prompts.json`: the final generation prompt set and tool mode.
- `references/`: local-only user-supplied design references, excluded from Git
  and release archives.

The app embeds both PNG masters, preserving alpha without relying on external
asset paths. The icon appears in the header and native desktop window; the
window icon is scaled at runtime. Desktop startup uses a transparent borderless
window for about 2.2 seconds, then restores the normal decorated, resizable
library window. **Open library →** skips the splash. Transparency depends on
native desktop compositor support. The existing mobile launch flow is unchanged.

Native icons are in `icons/`: Android density and adaptive launcher resources,
an opaque iOS app-icon catalog, and a macOS ICNS. Android's Gradle init script
registers the resources. The iOS finishing helper compiles its catalog with
Xcode actool before signing. Desktop packaging copies the ICNS into the macOS
bundle's Resources directory and registers it in Info.plist.
