# MCGUI Crafter identity

Selected modular C: six mint cells and one amber cell, with stepped pixel corners.

- `app-icon.svg`: square application icon; transparent outer corners, dark rounded tile.
- `logo-mark.svg`: transparent symbol for dark interfaces, without a tile.
- `logo-mono.svg`: single-color symbol using `currentColor`; inline SVG inherits text color. An SVG used through `<img>` does not inherit the host text color.
- `png/app-icon-{size}.png`: rendered application icons at 16–1024 pixels.
- `png/logo-mark-320.png`: transparent raster logo.
- `source/vectorize-original.svg`: unmodified Vectorize trace of the user-selected image, revision `0033870f14a140b3a0d0690e6edd6694`.

The production SVGs regularize that trace into seven identical 12-unit stepped cells, 2-unit gutters, a symmetric 60-unit rounded tile, and flat fills. No embedded bitmap, filters, fonts, or external resources. Palette from Vectorize: charcoal `#1c2129`, mint `#65faa5`, amber `#feac23`.

Use the tile icon on light surfaces; use the bare color symbol on dark surfaces. For a bare logo on a light surface, use the monochrome variant in dark ink. Preserve the amber cell at the top right, equal gutters, and the empty right side of the C.

Regenerate a PNG with `rsvg-convert -w 256 -h 256 assets/branding/app-icon.svg -o assets/branding/png/app-icon-256.png`.

The toolbar, start panel, and favicon use `app-icon.svg`. Tauri bundles `src-tauri/icons/icon.png`, copied from the 512-pixel export. The canonical brand kit and original Vectorize export are documented in `docs/brand-kit/README.md`.
