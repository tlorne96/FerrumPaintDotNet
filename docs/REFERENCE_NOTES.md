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