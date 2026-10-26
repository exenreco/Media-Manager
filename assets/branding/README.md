# Media Manager visual identity

Original artwork generated with the built-in ImageGen tool.

- `app-icon.png`: 1254 × 1254 RGBA master. Matte folded TV/play mark in mint,
  jade, periwinkle and apricot; transparent exterior and play aperture.
- `splash.png`: 1586 × 992 RGBA master. Floating navy information panel and
  layered cinematic play portal; transparent exterior.
- `prompts.json`: the final generation prompt set and tool mode.
- `references/`: local-only user-supplied design references, excluded from Git
  and release archives. The links below preserve their design context.

The [Copilot reference](https://1000logos.net/wp-content/uploads/2023/11/Copilot-Logo-history.png)
informed the folded-band treatment. The
[Photoshop splash reference](https://ajdesignco.com/wp-content/uploads/2016/11/photoshop-2017-1030x649.png)
informed the split information/artwork composition. The final TV/play mark and
portal illustration were created for this application.

The app embeds both PNG masters, preserving alpha without relying on external
asset paths. The icon appears in the header and native desktop window; the
window icon is scaled at runtime. Desktop startup uses a transparent borderless
window for about 2.2 seconds, then restores the normal decorated, resizable
library window. **Open library →** skips the splash. Transparency depends on
native desktop compositor support. The existing mobile launch flow is unchanged.

These are source design masters. Platform store icon sets and executable bundle
resources belong to the pending release-packaging work.
