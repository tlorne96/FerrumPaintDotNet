# Behaviour reference notes (clean-room)

Ferrum Paint's workflow and shortcuts are modelled on the *publicly documented* behaviour of
Paint.NET. No Paint.NET binaries, code, resources or assets were inspected, decompiled or copied.
All icons are drawn procedurally by Ferrum Paint's own code.

Source used: Paint.NET documentation, "Keyboard & Mouse Commands" (revision 14 Oct 2026)
https://www.getpaint.net/doc/latest/KeyboardMouseCommands.html

Key points taken from the public docs:
- Tools share letters and repeated presses cycle the group (S: Rectangle/Lasso/Ellipse/Magic Wand,
  M: Move Pixels/Move Selection, O: Line-Curve/Shapes). Shift+letter cycles in reverse.
- F5-F8 toggle Tools, History, Layers, Colors windows. X swaps primary/secondary colours.
- Selection modifiers: Ctrl+drag add, Alt+drag subtract, Ctrl+right drag invert (xor),
  Alt+right drag intersect, Shift forces square/circle. Magic wand / bucket: Shift = global.
- Shapes: drag with left = primary, right = secondary; A / Shift+A cycle shapes; Shift locks ratio;
  Alt resizes about centre; drag inside shape moves it; Enter/Esc/Ctrl+D commits.
- Line/Curve: Alt draws from centre; , . / cycle start cap, line style, end cap.
- [ and ] change brush width by 1, Ctrl+[ / Ctrl+] by 5.
- Menu shortcuts listed in README.md.

## Full shortcut list from the public docs (for implementation)
UI: wheel scroll; Shift+wheel horizontal; Ctrl+wheel zoom; Ctrl+0 actual size; Ctrl+B zoom to window (press again restores);
Space+drag pan; middle-drag pan; Ctrl+D or Enter deselect; arrows nudge current tool 1px, Ctrl+arrows 10px;
F5 Tools, F6 History, F7 Layers, F8 Colors; Ctrl+Shift+F5..F8 reset window; Esc/Enter/Ctrl+D commit tool drawing.
Images: Ctrl+Tab / Ctrl+PgDn next, Ctrl+Shift+Tab / Ctrl+PgUp previous, Ctrl+1..9 / Alt+1..9 switch image.
Toolbar: [ ] brush width -/+1, Ctrl+[ Ctrl+] -/+5.
File: Ctrl+N new, Ctrl+O open, Ctrl+W / Ctrl+F4 close, Ctrl+S save, Ctrl+Shift+S save as, Ctrl+Alt+S save all, Ctrl+P print, Alt+F4 exit.
Edit: Ctrl+Z undo, Ctrl+Y redo, Ctrl+X / Shift+Del cut, Ctrl+C / Ctrl+Ins copy, Ctrl+Shift+C copy merged,
Ctrl+V / Shift+Ins paste, Ctrl+Shift+V paste into new layer, Ctrl+Alt+V paste into new image, Delete erase selection,
Backspace fill selection (Shift+Backspace secondary), Ctrl+I invert selection, Ctrl+A select all.
View: Ctrl++ zoom in, Ctrl+- zoom out, Ctrl+B zoom to window, Ctrl+Shift+B zoom to selection, Ctrl+0 actual size.
Image: Ctrl+Shift+X crop to selection, Ctrl+R resize, Ctrl+Shift+R canvas size, Ctrl+H rotate 90 CW, Ctrl+G rotate 90 CCW,
Ctrl+J rotate 180 (also HH/GG), Ctrl+Shift+F flatten.
Layers: Ctrl+Shift+N add, Ctrl+Shift+Del delete, Ctrl+Shift+D duplicate, Ctrl+M merge down, Ctrl+, toggle visibility,
Ctrl+Shift+Z rotate/zoom, Ctrl+Alt+PgUp top layer, Alt+PgUp layer above, Alt+PgDn layer below, Ctrl+Alt+PgDn bottom,
Ctrl+Alt+Shift+PgUp move to top, Alt+Shift+PgUp move up, Alt+Shift+PgDn move down, Ctrl+Alt+Shift+PgDn move to bottom, F4 properties.
Adjustments: Ctrl+Shift+L auto-level, Ctrl+Shift+G B&W, Ctrl+Shift+T brightness/contrast, Ctrl+Shift+M curves,
Ctrl+Shift+U hue/sat, Ctrl+Alt+I invert alpha, Ctrl+Shift+I invert colors, Ctrl+L levels, Ctrl+Shift+P posterize, Ctrl+Shift+E sepia.
Effects: Ctrl+F repeat last effect. Help: F1.
Tools: S x1..4 rect/lasso/ellipse/wand (Shift+S reverse), F bucket, B brush, P pencil, L clone, T text, O line, OO shapes (Shift+O),
M move pixels, MM move selection, Z zoom, H pan, G gradient, E eraser, K picker, R recolor.
Colors window: X swap, C switch active colour; wheel left = active slot, right = inactive; Ctrl-drag hue only, Alt-drag saturation only.
Bucket: Shift global, Ctrl sample image. Picker: Ctrl samples image. Gradient: right-drag reverses; right-click nub swaps; Shift 15 deg.
Clone: Ctrl+click source. Recolor: left replaces secondary with primary; right reverse.
Text: Esc commits. Shapes: A / Shift+A cycle shape type.

## Adjustments and Effects menus (public documentation, read 2026-11-27)

Sources: https://www.getpaint.net/doc/latest/AdjustmentsMenu.html and https://www.getpaint.net/doc/latest/EffectsMenu.html
(sub-pages EffectsArtisticMenu, EffectsBlursMenu, EffectsColorMenu, EffectsDistortMenu, EffectsNoiseMenu,
EffectsObjectMenu, EffectsPhotoMenu, EffectsRenderMenu, EffectsStylizeMenu .html). Only menu names, effect names
and the user-visible descriptions of each control were used; all algorithms are original.

- Adjustments: Auto-Level, Black and White, Brightness / Contrast, Curves, Exposure, Highlights / Shadows,
  Hue / Saturation, Invert Alpha, Invert Colors, Levels, Posterize, Sepia (intensity), Temperature and Tint.
- Artistic: Ink Sketch, Linocut, Mosaic, Oil Painting, Pencil Sketch, Pointillism, Stained Glass.
- Blurs: Bokeh, Fragment, Gaussian, Median, Motion, Radial, Sketch, Square, Surface, Zoom.
- Color: Quantize (Median Cut / Octree, dithering 0-8).
- Distort: Bulge, Crystalize, Dents, Frosted Glass, Morphology, Pixelate, Polar Inversion, Spherize,
  Tile Reflection, Twist, Waves.
- Noise: Add Noise, Reduce Noise. Object: Drop Shadow.
- Photo: Glow, Red Eye Removal, Sharpen, Soften Portrait, Straighten, Vignette.
- Render: Clouds, Julia Fractal, Mandelbrot Fractal, Turbulence.
- Stylize: Edge Detect, Emboss, Halftone, Outline, Relief.
- Ctrl+F repeats the last effect with the same parameters without showing a dialog.
## .pdn import (public sources only)

- Microsoft [MS-NRBF] .NET Remoting Binary Format, open specification: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-nrbf/
- pypdn (MIT), reader description of the PDN3 layout: https://github.com/addisonElliott/pypdn (pypdn/reader.py)
  - "PDN3" magic, 3-byte LE header size, UTF-8 XML header (width, height, layers, savedWithVersion, thumbnail), bytes 00 01, NRBF graph.
  - Root Document: width, height, layers (LayerList/ArrayList: _items, _size). BitmapLayer: Layer+properties (name, visible, isBackground, opacity, blendMode enum value__), surface (width, height, stride, scan0 MemoryBlock length64).
  - After the NRBF MessageEnd, one pixel block per layer in order: format byte (0 gzip, 1 raw), BE u32 chunk size, then chunks of (BE u32 index, BE u32 size, data). BGRA, straight alpha.
  - Blend enum order: Normal, Multiply, Additive, ColorBurn, ColorDodge, Reflect, Glow, Overlay, Difference, Negation, Lighten, Darken, Screen, Xor. 3.x uses a blendOp object named *BlendOp instead.
  - Reflect = min(A^2/(1-B),1); Glow = min(B^2/(1-A),1); Negation = 1-|1-A-B| (A = below, B = layer), composited as (a-ab)A + (b-ab)B + ab*F.
- Paint.NET roadmap: Paint.NET 6.0 (planned 2027) moves to a new, publicly documented .pdn format: https://paint.net/roadmap.html
- No Paint.NET source (including the old OpenPDN tree) was consulted.
### .pdn export

- With the user's approval, the writer reproduces the record layout, class names, field names and assembly names observed in the *data* of .pdn files saved by Paint.NET 5.2 (dumped with Ferrum Paint's own reader). No Paint.NET code or binaries were examined.
- Output is verified by reading it back with the importer and by opening it in Paint.NET 5.2 (layers, blend modes, 7017×4959 drawings).
- Xor is a bitwise XOR of 8-bit colour values; this was confirmed by comparing Paint.NET's rendering of an exported test file, not by reading its code.
## Plugin-inspired effects (v0.1.3)
The user supplied a folder of third-party Paint.NET plugin DLLs (pyrochild's pack, by Zach Walker). Only the DLL version information and the list of referenced assemblies were read, to see what they target: Paint.NET 3.5 (.NET 2.0), 4.x (.NET 4.6/4.7) and 5.1 (.NET 9), all linking Paint.NET's own PaintDotNet.* assemblies, some with WinForms UIs. Nothing was decompiled. Loading them would need Paint.NET's proprietary assemblies or a reimplementation of its plugin API, and would be Windows-only, so Ferrum Paint instead has native effects written from the public descriptions on the plugin forum:
- Pack thread: https://forums.paint.net/topic/7291-pyrochild-plugins-2026-10-04/
- Jitter: https://forums.paint.net/topic/4681-jitter/ (rows shifted by a random amount; angle, row width, edge behaviour).
- Outline Object: https://forums.paint.net/topic/4923-outline-object/ (outline around non-transparent pixels in the primary colour; width, softness). Ours uses an exact Euclidean distance transform (Felzenszwalb & Huttenlocher, "Distance Transforms of Sampled Functions", 2012).
- Stitch: https://forums.paint.net/topic/5936-stitch/ (stitched/embroidered look, two angles).
- Random Shape Fill: https://forums.paint.net/topic/10633-random-shape-fill/ (shape modes, fill/outline colour modes, size, transparency, keep inside, antialiasing). Ours reuses Ferrum's Shapes-tool outlines.
- Trail: https://forums.paint.net/topic/18653-trail/ (object trail, distance, angle, fading tail).
- Film, Gradient Blocks, Gradient Bars, Gradient Grid, Gradient Mapping, Borders N' Shapes, Pixelate+, Splatter, Random Effect: one-line descriptions in the pack thread only.
Of these, Jitter, Stitch, Rectangular Pixelate (Pixelate+), Gradient Blocks, Outline Object, Film, Random Shapes (Random Shape Fill), Splatter and Random Effect remain clean-room, because those plugins have no published source.

## MIT ports of pyrochild's open-source plugins (v0.1.4)
At the user's request, the MIT-licensed repositories at https://github.com/bsneeze were then read and these plugins were ported to Rust: Borders N' Shapes, Gradient Bars, Gradient Grid, Trail, Gradient Mapping, Curves+ (pixel maths from `Histograms/UnaryPixelHistogramOps.cs` only), Smudge, Liquify and Grid Warp. The repositories, commits and copyright lines are listed in `THIRD_PARTY_NOTICES.md`, together with the MIT licence text.
- Files in those repositories that carry a Paint.NET / dotPDN LLC copyright header were not used. They include `SplineInterpolator.cs`, `ColorBgraBlender.cs`, `SrgbGamma.cs`, `DiskBackedSurface.cs`, `SliderControl.cs`, and Curves+'s `ConfigDialog`, `ConfigToken`, `CurvesPlus.cs` and `CurveControl*`. Curves+ therefore uses Ferrum's own curve spline, and its dialog is Ferrum's own.
- Gradient Mapping preset colours were converted from the source's BGR order.
- Liquify's "Turbulence" mode is a no-op in the published source, so it was left out.
- No pyrochild plugin DLL was decompiled. Twist has no published source and was not done. Color Match has none either (the bsneeze account only hosts the ten repositories above), so it was built clean-room from two published methods: per-channel histogram matching and Reinhard et al.'s Lab statistics transfer (2001).

## Clean-room effects from BoltBait's plugin pack (v0.1.4)
The user added BoltBait's pack to the demo folder. It is freeware: https://www.boltbait.com/pdn/ offers it for free use, but publishes no source and no open licence. So nothing was decompiled or read beyond each DLL's version information. The following were written from the effect names and menu placements listed on that page, plus common knowledge of what such adjustments do:
- Adjustments: Black and White+ (channel mixer with filter presets and toning), Color Balance (shadows / midtones / highlights, preserve luminosity), Transparency.
- Artistic: Dream (screen with a blurred copy), Pastel. Color: Complementary Colors. Stylize: Floyd-Steinberg Dithering (the standard 7/3/5/1 error-diffusion kernel, Floyd & Steinberg 1976).
- Object: Switch Alpha to Gray, Switch Gray to Alpha, Feather Object, Bevel Object, Inner Shadow (all built on the Euclidean distance transform already used by Outline Object). Render: Grid / Checkerboard.
- Skipped because Ferrum already has an equivalent: Flip, Gaussian Blur+, Sharpen Classic, Oil Paint+, Edge Detect Classic, Drop Shadow+, Vignette+, Outline Object/Selection. Skipped for now: Charts, Calendar, Dimensions, Meme Maker, Text Fun Factory, Seam Carving, Level Horizon, Remove Dust, Fill from Clipboard/File, Paste Alpha, Apply Alpha Mask, HSV Gradients, Hue/Sat+, Flames, Polygon/Stars, Sharpen Landscape, the selection tools.