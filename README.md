# Ferrum Paint

Ferrum Paint is a layered raster image editor for **Windows** and **Ubuntu 26.04**, written in Rust with [egui](https://github.com/emilk/egui) (UI) and [tiny-skia](https://github.com/RazrFalcon/tiny-skia) (rendering). Its workflow, layout, tools and keyboard shortcuts follow the conventions of Paint.NET. The same code paths run on both platforms, so it behaves the same on each.

![Ferrum Paint icon](docs/icon-256.png)

## Clean-room statement

Ferrum Paint is an independent clean-room implementation. Its behaviour is based only on Paint.NET's **public user documentation** (see [docs/REFERENCE_NOTES.md](docs/REFERENCE_NOTES.md) for the sources). No Paint.NET binaries were decompiled or inspected, and no code, icons, images or other assets were copied. All icons are drawn procedurally in `src/icons.rs`. Ferrum Paint is not affiliated with or endorsed by Paint.NET or dotPDN LLC.

The one exception is a set of effects and tools ported from Zach Walker's (pyrochild's) **MIT-licensed** open-source Paint.NET plugins: Borders N' Shapes, Gradient Bars, Gradient Grid, Trail, Gradient Mapping, Curves+ (pixel maths only), Smudge, Liquify and Grid Warp. Their licence and the exact list of what was ported are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Files in those repositories that carry Paint.NET's own copyright were not used.

## Features

- **Paint.NET-style layout**
  - Menu bar with image thumbnail tabs, a main toolbar and a per-tool options bar.
  - Floating Tools, History, Layers and Colors windows (toggle with F5–F8) and a status bar.
- **Tools**
  - Selection: Rectangle Select, Lasso Select, Ellipse Select and Magic Wand. Each supports replace, add (Ctrl), subtract (Alt), invert and intersect.
  - Move: Move Selected Pixels and Move Selection, both with drag, right-drag rotate and arrow-key nudge.
  - Navigation: Zoom and Pan.
  - Painting: Paint Bucket, Gradient, Paintbrush, Eraser, Pencil, Color Picker, Clone Stamp, **Smart Clone** and Recolor.
  - Drawing: Text, Line / Curve and Shapes.
- **Shapes tool** with two drawing modes, chosen in the options bar under **Draw from: Corner / Centre**:
  - *Corner*: the drag runs from one corner to the opposite corner.
  - *Centre*: the drag starts at the centre and grows the shape outwards symmetrically.
  - Holding **Alt** while dragging switches to the other mode for that shape.
  - **Shift** constrains proportions (square or circle).
  - There are 14 shape types: rectangle, rounded rectangle, ellipse, triangle, right triangle, diamond, pentagon, hexagon, octagon, star, arrow, heart, cross and speech bubble. Press A or Shift+A to cycle through them.
  - Draw modes are outline, filled, or filled with an outline. The primary colour draws the outline and the secondary colour fills.
  - A shape stays editable after you draw it. Drag inside it to move it or drag a handle to resize it, then press Enter (or switch tool) to finish.
- **Layers**: add, delete, duplicate, merge down, move up or down, visibility, and properties (name, opacity, and 21 blend modes, including Paint.NET's Reflect, Glow, Negation and bitwise Xor). A full undo/redo history is kept in the History window.
- **Image menu**: crop to selection, resize (with resampling choice), canvas size (with anchor), flip, rotate 90°/180°, flatten, and Rotate/Zoom layer.
- **Adjustments** (19): Auto-Level, Black and White, Black and White+, Brightness/Contrast, Color Balance, Color Match, Curves, Curves+, Exposure, Gradient Mapping, Highlights/Shadows, Hue/Saturation, Invert Alpha, Invert Colors, Levels, Posterize, Sepia (with intensity), Temperature and Tint, and Transparency.
  - **Curves+** has Luminosity, RGB, HSV, CMYK, Alpha and Advanced modes. Advanced maps any input channel to any output channel, and the per-channel *Edit* boxes link curves together.
  - **Smart Clone** (key L, cycles with Clone Stamp; helmet icon) has a mode drop-down in the toolbar: *Blend clone* (Ctrl+click a source, drag; copied pixels are colour-matched to their surroundings so there are no seams) and *Content-aware heal* (paint over a blemish and release; it is filled from the surrounding texture, honouring the selection). Clean-room.
  - **Color Match** recolours the layer to resemble a reference image from the clipboard or a file, by histogram matching or Lab statistics transfer (clean-room).
  - **Gradient Mapping** maps any of 12 input channels (A, R, G, B, C, M, Y, K, H, S, V, Luminosity) through a gradient, with 15 presets, an offset that can wrap, and a Lock alpha option.
- **Effects → Tools** (interactive, in their own canvas window with brush preview, Ctrl+wheel zoom and their own undo/redo):
  - **Liquify**: Push, Bloat, Pucker, Twist left/right, Reconstruct, Freeze and Thaw, with size, pressure and density. Right-drag uses the opposite mode, and holding still keeps bloating or twisting.
  - **Grid Warp**: drag the handles of an adjustable grid (1–100 cells each way) to warp the image.
  - **Smudge**: soft or hard brush with size, strength, jitter and quality.
- **Effects** (69, in the same submenus as Paint.NET, plus **Effects → Random Effect**):
  - Artistic: Dream, Ink Sketch, Linocut, Mosaic, Oil Painting, Pastel, Pencil Sketch, Pointillism, Splatter, Stained Glass.
  - Blurs: Bokeh, Fragment, Gaussian, Gradient Blocks, Median, Motion, Radial, Sketch, Square, Surface, Zoom.
  - Color: Complementary Colors, Quantize (median cut or octree, with dithering).
  - Distort: Bulge, Crystalize, Dents, Frosted Glass, Jitter, Morphology, Pixelate, Polar Inversion, Rectangular Pixelate, Spherize, Stitch, Tile Reflection, Twist, Waves.
  - Noise: Add Noise, Reduce Noise.
  - Object: Bevel Object, Drop Shadow, Feather Object, Inner Shadow, Outline Object, Switch Alpha to Gray, Switch Gray to Alpha, Trail.
  - Photo: Film, Glow, Red Eye Removal, Sharpen, Soften Portrait, Straighten, Vignette.
  - Render: Borders N' Shapes, Clouds, Gradient Bars, Gradient Grid, Grid / Checkerboard, Julia Fractal, Mandelbrot Fractal, Random Shapes, Turbulence (with blend modes).
  - Stylize: Edge Detect, Emboss, Floyd-Steinberg Dithering, Halftone, Outline, Relief.
  - **Popular plugins, built in.** Ferrum Paint can't load Paint.NET plugin DLLs, because they depend on Paint.NET's own .NET assemblies and Windows-only UI code. Instead, popular plugins are built in natively. Borders N' Shapes, Gradient Bars, Gradient Grid, Trail, Gradient Mapping, Curves+, Liquify, Grid Warp and Smudge are ports of pyrochild's MIT-licensed source. Splatter, Gradient Blocks, Jitter, Rectangular Pixelate, Stitch, Outline Object, Film, Random Shapes and Random Effect are clean-room versions of the rest of pyrochild's pack. Black and White+, Color Balance, Transparency, Dream, Pastel, Complementary Colors, Floyd-Steinberg Dithering, the alpha/gray switches, Feather Object, Bevel Object, Inner Shadow and Grid / Checkerboard are clean-room versions of effects from BoltBait's pack. The clean-room ones were written from each plugin's public name and one-line description, because that source isn't openly licensed.
  - Dialogs are generated from each effect's parameter list: sliders, an angle dial, drop-downs, colour pickers, a centre-point pad over a thumbnail of the layer, Reseed and Reset.
  - The preview renders on a background thread, so the UI stays responsive. Cancel restores the original, and effects apply only inside the selection.
  - Ctrl+F repeats the last effect with the same settings.
- **Snap to Grid** (**View → Snap to Grid**, Ctrl+; or the toolbar magnet-grid button): shapes, lines/curves, rectangle and ellipse selections, gradients, text placement and moves snap to a grid. The grid size (default 16 px) is set in the View menu or in the toolbar next to the button, and the grid is drawn over the canvas while snapping is on. Freehand tools (brush, pencil, eraser, lasso, etc.) are never snapped. The setting is remembered.
- **Look**: dark mode and light mode. **View → Theme** offers Use System Setting (the default; on Windows it follows changes to the system theme while the app is running), Light or Dark, and the sun/moon button at the right end of the toolbar switches instantly. The window title bar matches on Windows, and the choice is remembered. Also native UI font (Segoe UI on Windows, Ubuntu on Ubuntu), compact tool-window headers.
- **Clipboard**: cut, copy, copy merged, paste, paste into new layer and paste into new image. Images are exchanged with other applications through the system clipboard.
- **Files**:
  - Native layered format: `.ferrum`. Like `.pdn`, it keeps every layer with its name, visibility, opacity and blend mode, so you can close the app and carry on later without merging anything.
  - New images save as `.ferrum` by default. If you save a layered image as PNG, JPEG or another flat format, Ferrum Paint asks first and offers **Save as .ferrum instead**. Flattening only affects the exported file; the open image keeps its layers.
  - File association: `install-linux.sh` registers the `application/x-ferrum-paint` MIME type on Ubuntu. On Windows, run `ferrum-paint.exe --register-file-type` once to make `.ferrum` files open in Ferrum Paint (current user only). The Windows installer and the .deb do this for you.
  - **File → Export...** (Ctrl+Alt+E) writes a flattened PNG, JPEG (with a quality slider), BMP, TGA, TIFF or WebP copy. The open image keeps its layers, its file name and its saved state, so Ctrl+S still saves the layered `.ferrum`. The next export starts with the same file name and folder.
  - **Opens and saves Paint.NET `.pdn` files**, with every layer, name, visibility, opacity and blend mode intact. Files from Paint.NET 3.x to 5.x open; saved files are written in the Paint.NET 5.2 layout and have been checked by opening them in Paint.NET 5.2. Open a `.pdn`, edit it and Ctrl+S saves it back as `.pdn`. Each save is read back and verified before it replaces the file. Layers using a blend mode Paint.NET lacks (Hard Light, Soft Light, Exclusion, Hue, Saturation, Color, Luminosity) are saved as Normal, and Ferrum Paint tells you which. The reader follows public documents (Microsoft's MS-NRBF spec and the MIT-licensed pypdn project); the writer copies the record layout seen in .pdn files' data. No Paint.NET code was used. The new format planned for Paint.NET 6.0 is not supported yet.
  - **Command line:** `ferrum-paint --convert in.pdn out.png` converts between any supported formats (`.ferrum` and `.pdn` keep layers).
  - Import and export: PNG, JPEG, BMP, GIF, TIFF, WebP, TGA and ICO. JPEG is composited over white; ICO is limited to 256×256.
  - Files can be opened from the command line or by drag-and-drop.
  - Saves are atomic: the file is written to a temporary file and then renamed.
  - Closing an image with unsaved changes asks before discarding them.

## Keyboard shortcuts (summary)

The full list is under **Help → Keyboard Shortcuts** (F1) in the app.

| Keys | Action |
|---|---|
| S, M, Z, H, F, G, B, E, P, K, L, R, T, O | Tool selection (press again to cycle within a group; O = Line/Curve, then Shapes) |
| A / Shift+A | Next / previous shape type |
| [ / ] | Brush width −1 / +1 (Ctrl: ±5) |
| X | Swap primary and secondary colours |
| Enter / Esc | Finish the current shape, line, text or gradient |
| Ctrl+N / O / S / Shift+S / W | New / Open / Save / Save As / Close |
| Ctrl+Alt+E | Export a flattened PNG/JPEG copy |
| Ctrl+; | Snap to Grid on/off |
| Ctrl+Z / Ctrl+Y | Undo / Redo |
| Ctrl+X / C / V, Ctrl+Shift+C / V, Ctrl+Alt+V | Cut, copy, paste, copy merged, paste into new layer, paste into new image |
| Ctrl+A / D / I | Select all / Deselect / Invert selection |
| Ctrl+Shift+X, Ctrl+R, Ctrl+Shift+R | Crop to selection, Resize, Canvas size |
| Ctrl+H / G / J | Rotate 90° clockwise / 90° anticlockwise / 180° |
| Ctrl+Shift+N / D / Del, Ctrl+M, F4 | Add / Duplicate / Delete layer, Merge down, Layer properties |
| Ctrl++ / Ctrl+- / Ctrl+0 / Ctrl+B | Zoom in / out / actual size / zoom to window |
| F5–F8 | Toggle Tools / History / Layers / Colors windows |
| Space + drag | Pan with any tool |

## Building

**Just want to install it?** Grab the Windows `Setup.exe` or the Ubuntu `.deb` from [Releases](https://github.com/tlorne96/FerrumPaintDotNet/releases). [INSTALL.md](INSTALL.md) has the details. This section is for building from source.

You need a recent stable Rust toolchain (installed through [rustup](https://rustup.rs)).

### Windows

```powershell
cargo build --release
.\target\release\ferrum-paint.exe
```

On some Windows setups, folders under `Documents` have a read-only attribute that cannot be cleared, often because of Controlled Folder Access or OneDrive. The build then fails with `output path is not a writable directory` (from `autocfg`). To fix this, point Cargo's output somewhere writable:

```powershell
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\ferrum-paint-target"
cargo build --release
```

### Ubuntu 26.04

Install the build dependencies, then build:

```bash
sudo apt install build-essential pkg-config libxkbcommon-dev libgl1-mesa-dev \
    libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
    xdg-desktop-portal xdg-desktop-portal-gtk
cargo build --release
./target/release/ferrum-paint
```

Ferrum Paint runs natively on both Wayland and X11. Its file dialogs use the XDG desktop portal. To install it for your user, with a menu entry and icons, run:

```bash
bash packaging/install-linux.sh
```

### Tests

```bash
cargo test
```

The tests run without a window and check the tool behaviour. They cover:

- Shapes in corner and centre modes, the Alt toggle, the Shift constraint, fill and outline colours, and editing with move and resize handles.
- Tool shortcuts and exact modifier matching.
- Text entry, invert within a selection, and undo.
- The `.ferrum` round-trip and PNG/JPEG export.
- Paintbrush strokes, and crop to selection.
- Every adjustment and effect: it runs with default settings, keeps the image size, changes the image, and leaves pixels outside the selection untouched.
- Saving a layered image to a flat format warns first and never merges the open document's layers.

To time every effect on a 2000 × 1500 image: `cargo test --release -- --ignored effect_timing --nocapture`. On the development machine each effect takes 0.2–2 s.

### Building the installers

- Windows: install [Inno Setup 6](https://jrsoftware.org/isinfo.php) (`winget install JRSoftware.InnoSetup`), then run `powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1`. The output is `dist\FerrumPaint-<version>-Setup.exe`.
- Ubuntu: `cargo install cargo-deb`, then `cargo deb`. The output is `target/debian/ferrum-paint_<version>-1_amd64.deb`.
- Pushing a `v*` tag runs `.github/workflows/release.yml`, which builds both and attaches them to a GitHub Release.

### Exporting the app icon

```bash
ferrum-paint --write-icon icon.png 256
```

## Project layout

| Path | Contents |
|---|---|
| `src/app.rs` | Application state, commands, shortcut mapping, save and close flow, clipboard |
| `src/ui.rs` | Menus, image tabs, toolbars, status bar, Tools/History/Layers windows |
| `src/colors.rs` | Colors window (colour wheel, sliders, hex, palette) |
| `src/canvas.rs` | Canvas view, tool input state machine, live previews |
| `src/document.rs` | Layers, documents, undo history |
| `src/raster.rs` | Pixel operations and the classic adjustments |
| `src/fx/` | Effect registry, parameter specs and all parametric adjustments and effects (`pyrochild.rs`: MIT ports; `extras.rs`, `pack2.rs`: clean-room plugin-style effects) |
| `src/curves_plus.rs` | Curves+ (MIT port of the pixel maths) |
| `src/color_match.rs` | Color Match (clean-room) |
| `src/smart_clone.rs` | Smart Clone: blend clone and content-aware heal (clean-room) |
| `src/interactive.rs` | Liquify, Grid Warp and Smudge (MIT ports) and their canvas window |
| `src/theme.rs` | Light and dark themes, UI fonts, tool-window headers, saved preferences |
| `src/paint.rs` | Shape, line, brush, text and gradient rasterisation (tiny-skia) |
| `src/dialogs.rs` | Modal dialogs and the shortcut table |
| `src/fileio.rs` | `.ferrum` format and image import/export |
| `src/pdn.rs` | Paint.NET `.pdn` import and export (NRBF reader/writer, layer pixel blocks) |
| `packaging/` | Windows installer (Inno Setup), `.deb` assets, icons, desktop entry, script installers |
| `src/icons.rs` | Procedurally drawn original icons |
| `vendor/egui-winit` | egui-winit 0.31.1, patched so that pasting image-only clipboard contents (Ctrl+V) works |

## Known limitations

- There is no printing and no plugin system (Paint.NET plugin DLLs can't be loaded). Paint.NET 6.0's new `.pdn` format isn't supported yet, and saved `.pdn` files carry no EXIF metadata.
- The installers are not code-signed, so Windows SmartScreen will warn the first time you run `Setup.exe`.
- Effects follow Paint.NET's names, menus and controls, but the algorithms are original (apart from the MIT ports listed above), so results will not be pixel-identical. Mosaic offers 7 tile shapes rather than Paint.NET's 21. Fractal colouring uses our own palette.
- Effect previews of heavy effects on very large images take a second or two to appear.
- The Text tool uses the two fonts bundled with egui. It does not use system fonts.
- The Ubuntu 26.04 build has not been tested on real hardware. Development and testing were done on Windows, and the Linux code paths rely on the platform layers of winit, arboard and rfd.

## Licence

Copyright (c) 2026 Tim Lorne. Released under the MIT licence; see [LICENSE](LICENSE). Ported third-party code keeps its own MIT notice; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
