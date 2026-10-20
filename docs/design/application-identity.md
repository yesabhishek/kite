# Kite identity

Kite is a focused native code editor and terminal. Its identity should feel like a useful instrument: quiet surfaces, legible controls, precise geometry, and a single purposeful accent.

The public name is **Kite**. Use lowercase `kite` for the executable, package names, and the drawn wordmark. The short description is **A native code editor and terminal.** The brand line is **A small space for serious work.**

## Mark

Four flat planes form an asymmetric kite. Paper seams separate the planes, while one orange plane gives the silhouette direction. The construction is original vector artwork, defined in [the export script](../../scripts/make-icon.swift). It does not reproduce the identity of another product.

The mark has no gradients, simulated materials, shadows, outlines, or tiny decorative details. Its shape works in one ink. The app icon places the same geometry on a warm paper tile with a subtle keyline and transparent corners.

- Preserve the plane geometry, seam width, and proportions.
- Leave clear space of at least one fifth of the mark's height on every side.
- Use the full color mark on paper or other quiet light backgrounds.
- Use the ink version for single color print and the paper version on dark surfaces.
- Keep the mark at least 24 pixels tall in interface and editorial use. The supplied desktop icon contains dedicated 16 pixel and higher raster variants.
- Use the supplied icon for launchers and installers. Keep the unboxed mark for the application interior and documentation.

## Color

| Role | Color | Use |
| --- | --- | --- |
| Paper | `#F4F2EC` | Light canvas, icon tile, documentation |
| Ink | `#232323` | Primary text and mark |
| Graphite | `#141414` | Dark canvas |
| Orange | `#DE4B2C` | Mark accent, selected controls, small emphasis |
| Muted | `#706D66` | Secondary labels on paper |
| Rule | `#D8D5CD` | Dividers and tile keyline |

Orange is an accent rather than a background for large fields of copy. Body copy uses ink on paper or a high contrast light tone on graphite. Theme colors for syntax and diagnostics remain semantic and may extend beyond the brand palette.

## Typography and composition

The lowercase wordmark is a compact, bold sans serif with careful spacing. The committed SVG contains glyph outlines, so it requires no installed web font and redistributes no font files. Its export uses the macOS `HelveticaNeue-Bold` face. Editorial headings use Helvetica Neue; small technical labels use Menlo. The application uses platform sans serif for interface text and a system monospace face for code. Its welcome screen uses the canonical outlined SVG wordmark on every platform.

Use generous margins and align content to a visible grid. Prefer flat surfaces, fine rules, sentence case labels, and small monospaced technical annotations. Numbered labels can give long pages or feature groups a useful rhythm. Leave the editor content visually dominant.

A reference can inform restraint and precision without becoming a visual template. Avoid copying another product's wordmark, icons, layout signatures, or distinctive controls.

## Voice

Write in direct, short sentences. Explain the action or capability with concrete words. Use **Open folder**, **New terminal**, and **Build from source**. Keep claims factual and acknowledge experimental platform support where relevant. Avoid promotional superlatives, emoji, and em dashes in project authored copy.

## Assets

| Asset | Purpose |
| --- | --- |
| [`assets/package/kite.svg`](../../assets/package/kite.svg) | Canonical vector app icon |
| [`assets/package/kite.png`](../../assets/package/kite.png) | 1024 pixel RGBA application icon |
| [`assets/package/kite.icns`](../../assets/package/kite.icns) | macOS icon, 16 through 1024 pixels |
| [`assets/package/kite.ico`](../../assets/package/kite.ico) | Windows icon, 16, 24, 32, 48, 64, 128, and 256 pixels |
| [`assets/brand/mark.svg`](../../assets/brand/mark.svg) | Full color unboxed mark |
| [`assets/brand/mark-ink.svg`](../../assets/brand/mark-ink.svg) | Single color ink mark |
| [`assets/brand/mark-paper.svg`](../../assets/brand/mark-paper.svg) | Single color paper mark |
| [`assets/brand/wordmark.svg`](../../assets/brand/wordmark.svg) | Mark and lowercase wordmark lockup |
| [`assets/brand/readme-banner.svg`](../../assets/brand/readme-banner.svg) | 1600 by 680 pixel README masthead |
| [`assets/brand/readme-banner.png`](../../assets/brand/readme-banner.png) | Raster version of the README masthead |
| [`assets/brand/social-preview.png`](../../assets/brand/social-preview.png) | 1280 by 640 pixel GitHub social preview |
| [`assets/brand/social-preview.svg`](../../assets/brand/social-preview.svg) | Editable vector source of the social preview |

The artwork is authored as vector geometry in this repository and distributed under its MIT license. The SVG files are self contained and contain no remote assets, embedded raster images, font dependencies, or scripts.

## Rebuild

On macOS, from the repository root:

```sh
swift scripts/make-icon.swift
```

An optional output directory redirects the four package icons. Brand assets are always written to `assets/brand`.

```sh
swift scripts/make-icon.swift /tmp/kite-icons
```

If a restricted environment blocks the default compiler cache, select a writable module cache:

```sh
swift -module-cache-path /tmp/kite-swift-module-cache scripts/make-icon.swift
```

The script draws every raster size directly from vector geometry. ICNS and ICO are assembled from their PNG variants without additional dependencies. Text becomes vector outlines through CoreText. Committed assets are the canonical files; text outline details can vary with the macOS font version used to regenerate them.
