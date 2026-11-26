//! File loading and saving.
//!
//! Native format (`.ferrum`), little-endian:
//!   magic "FERRUMP1", u32 width, u32 height, u32 layer count, u32 active layer,
//!   then per layer: u32 name length + UTF-8 name, u8 visible, u8 opacity, u8 blend id,
//!   u32 PNG length + PNG bytes (straight RGBA, encoded by tiny-skia).

use crate::document::{Document, Layer, LayerBlend};
use crate::raster;
use std::io::{Read, Write};
use std::path::Path;
use tiny_skia::Pixmap;

const MAGIC: &[u8; 8] = b"FERRUMP1";

fn ext(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".into())
}

pub fn load(p: &Path) -> Result<Document, String> {
    let mut doc = if ext(p) == "ferrum" {
        load_native(p)?
    } else {
        let px = load_flat(p)?;
        let (w, h) = (px.width(), px.height());
        let mut l = Layer::new("Background", w, h);
        l.pixmap = px;
        Document::from_layers(file_name(p), w, h, vec![l], "Open Image")
    };
    doc.name = file_name(p);
    doc.path = Some(p.to_path_buf());
    doc.mark_saved();
    Ok(doc)
}

pub fn load_flat(p: &Path) -> Result<Pixmap, String> {
    let img = image::ImageReader::open(p)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return Err("The image is empty.".into());
    }
    Ok(raster::pixmap_from_straight(rgba.as_raw(), w, h))
}

fn rd_u32(r: &mut impl Read) -> Result<u32, String> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b).map_err(|e| e.to_string())?;
    Ok(u32::from_le_bytes(b))
}

fn rd_u8(r: &mut impl Read) -> Result<u8, String> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b).map_err(|e| e.to_string())?;
    Ok(b[0])
}

fn load_native(p: &Path) -> Result<Document, String> {
    let data = std::fs::read(p).map_err(|e| e.to_string())?;
    let mut r = std::io::Cursor::new(data);
    let mut magic = [0u8; 8];
    r.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != MAGIC {
        return Err("Not a Ferrum Paint image.".into());
    }
    let w = rd_u32(&mut r)?;
    let h = rd_u32(&mut r)?;
    let n = rd_u32(&mut r)?;
    let active = rd_u32(&mut r)? as usize;
    if w == 0 || h == 0 || n == 0 || n > 10_000 {
        return Err("The file is corrupt.".into());
    }
    let mut layers = vec![];
    for _ in 0..n {
        let nl = rd_u32(&mut r)? as usize;
        if nl > 1 << 20 {
            return Err("The file is corrupt.".into());
        }
        let mut name = vec![0u8; nl];
        r.read_exact(&mut name).map_err(|e| e.to_string())?;
        let visible = rd_u8(&mut r)? != 0;
        let opacity = rd_u8(&mut r)?;
        let blend = LayerBlend::from_id(rd_u8(&mut r)?);
        let pl = rd_u32(&mut r)? as usize;
        let mut png = vec![0u8; pl];
        r.read_exact(&mut png).map_err(|e| e.to_string())?;
        let pm = Pixmap::decode_png(&png).map_err(|e| e.to_string())?;
        if pm.width() != w || pm.height() != h {
            return Err("Layer size mismatch.".into());
        }
        layers.push(Layer { name: String::from_utf8_lossy(&name).to_string(), pixmap: pm, visible, opacity, blend });
    }
    let mut doc = Document::from_layers(file_name(p), w, h, layers, "Open Image");
    doc.active = active.min(doc.layers.len() - 1);
    Ok(doc)
}

fn save_native(doc: &Document, p: &Path) -> Result<(), String> {
    let mut out: Vec<u8> = vec![];
    out.extend_from_slice(MAGIC);
    for v in [doc.width, doc.height, doc.layers.len() as u32, doc.active as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for l in &doc.layers {
        let name = l.name.as_bytes();
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend_from_slice(name);
        out.push(l.visible as u8);
        out.push(l.opacity);
        out.push(l.blend.id());
        let png = l.pixmap.encode_png().map_err(|e| e.to_string())?;
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&png);
    }
    write_atomic(p, &out)
}

fn write_atomic(p: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = p.with_extension(format!("{}.tmp", ext(p)));
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(bytes).map_err(|e| e.to_string())?;
        f.sync_all().ok();
    }
    std::fs::rename(&tmp, p).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

pub fn save(doc: &Document, p: &Path) -> Result<(), String> {
    let e = ext(p);
    if e == "ferrum" {
        return save_native(doc, p);
    }
    let comp = doc.composite();
    let (w, h) = (comp.width(), comp.height());
    let rgba = raster::pixmap_to_straight(&comp);
    let fmt = match e.as_str() {
        "png" => image::ImageFormat::Png,
        "jpg" | "jpeg" => image::ImageFormat::Jpeg,
        "bmp" => image::ImageFormat::Bmp,
        "tga" => image::ImageFormat::Tga,
        "tif" | "tiff" => image::ImageFormat::Tiff,
        "webp" => image::ImageFormat::WebP,
        "gif" => image::ImageFormat::Gif,
        "ico" => image::ImageFormat::Ico,
        _ => return Err(format!("Unsupported file type \".{e}\". Use .ferrum, .png, .jpg, .bmp, .tga, .tif or .webp.")),
    };
    let mut buf = std::io::Cursor::new(Vec::<u8>::new());
    if fmt == image::ImageFormat::Jpeg {
        // JPEG has no alpha: composite over white.
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for px in rgba.chunks_exact(4) {
            let a = px[3] as u32;
            for c in 0..3 {
                rgb.push(((px[c] as u32 * a + 255 * (255 - a) + 127) / 255) as u8);
            }
        }
        let img = image::RgbImage::from_raw(w, h, rgb).ok_or("encode error")?;
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 95);
        img.write_with_encoder(enc).map_err(|e| e.to_string())?;
    } else {
        if fmt == image::ImageFormat::Ico && (w > 256 || h > 256) {
            return Err("ICO images can be at most 256 × 256 pixels.".into());
        }
        let img = image::RgbaImage::from_raw(w, h, rgba).ok_or("encode error")?;
        img.write_to(&mut buf, fmt).map_err(|e| e.to_string())?;
    }
    write_atomic(p, &buf.into_inner())
}
