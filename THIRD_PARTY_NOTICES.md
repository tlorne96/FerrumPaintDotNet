# Third-party notices

Ferrum Paint is its own program and is not affiliated with Paint.NET or dotPDN LLC. Most of it is
original code. The parts listed below are Rust ports of open-source Paint.NET plugins by
**Zach Walker (pyrochild)**, published at <https://github.com/bsneeze> under the MIT licence. The
MIT licence requires that its copyright and permission notice travel with the ported code, so
they are reproduced here.

## What was ported

| Ferrum Paint feature | Source repository (commit read) | Copyright line | Rust file |
| --- | --- | --- | --- |
| Effects → Render → Borders N' Shapes | [bsneeze/pdn-bordersnshapes](https://github.com/bsneeze/pdn-bordersnshapes) (698867c) | Copyright (c) 2007, 2016 Zach Walker | `src/fx/pyrochild.rs` |
| Effects → Render → Gradient Bars | [bsneeze/pdn-gradientbars](https://github.com/bsneeze/pdn-gradientbars) (add8892) | Copyright (c) 2007, 2016 Zach Walker | `src/fx/pyrochild.rs` |
| Effects → Render → Gradient Grid | [bsneeze/pdn-gradientgrid](https://github.com/bsneeze/pdn-gradientgrid) (2fe3d60) | Copyright (c) 2011, 2017 Zach Walker | `src/fx/pyrochild.rs` |
| Effects → Object → Trail | [bsneeze/pdn-trail](https://github.com/bsneeze/pdn-trail) (f77deef) | Copyright (c) 2018 Zach Walker | `src/fx/pyrochild.rs` |
| Adjustments → Gradient Mapping (presets, channels, offset/wrap, lock alpha) | [bsneeze/pdn-gradientmapping](https://github.com/bsneeze/pdn-gradientmapping) (0f07e0d) | Copyright (c) 2007, 2016 Zach Walker | `src/fx/pyrochild.rs` |
| Adjustments → Curves+ (per-mode pixel maths only) | [bsneeze/pdn-curvesplus](https://github.com/bsneeze/pdn-curvesplus) (5d502f0) | Copyright (c) 2007, 2016 Zach Walker | `src/curves_plus.rs` |
| Effects → Tools → Smudge | [bsneeze/pdn-smudge](https://github.com/bsneeze/pdn-smudge) (9a2e359) | Copyright (c) 2008, 2016 Zach Walker | `src/interactive.rs` |
| Effects → Tools → Liquify | [bsneeze/pdn-liquify](https://github.com/bsneeze/pdn-liquify) (8df75f4) | Copyright (c) 2010-2026 Zach Walker | `src/interactive.rs` |
| Effects → Tools → Grid Warp | [bsneeze/pdn-gridwarp](https://github.com/bsneeze/pdn-gridwarp) (c6de19b) | Copyright (c) 2012, 2016 Zach Walker | `src/interactive.rs` |

Only the rendering maths was ported. The user interfaces were rebuilt in egui, and no icons, images
or other resources from those repositories are included.

Some files in those repositories carry Paint.NET (dotPDN LLC) copyright headers rather than the MIT
licence, for example `SplineInterpolator.cs`, `ColorBgraBlender.cs`, `SrgbGamma.cs`,
`DiskBackedSurface.cs`, `SliderControl.cs` and Curves+'s `ConfigDialog`, `ConfigToken`,
`CurvesPlus.cs` and `CurveControl*` files. **None of those files were used.** Curves+ draws its
curves with Ferrum Paint's own spline code, and Liquify and Smudge use Ferrum Paint's own buffers.

The other plugin-style effects (Jitter, Stitch, Rectangular Pixelate, Gradient Blocks, Outline
Object, Film, Random Shapes, Splatter, Random Effect, Black and White+, Color Balance, Transparency,
Dream, Pastel, Complementary Colors, Floyd-Steinberg Dithering, Switch Alpha to Gray, Switch Gray to
Alpha, Feather Object, Bevel Object, Inner Shadow and Grid / Checkerboard) are original clean-room
code. They were written from the public names and one-line descriptions of those plugins only,
because their source is not published under an open licence. No plugin DLL was decompiled.

## MIT licence (applies to the ported code listed above)

```
The MIT License (MIT)

Copyright (c) 2007-2026 Zach Walker

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

The per-repository copyright years are listed in the table above. The combined line covers all of
them.
