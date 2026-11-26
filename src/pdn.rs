//! Import and export of Paint.NET `.pdn` images (the "PDN3" container used by Paint.NET 3.x to 5.x).
//!
//! Clean-room note: this reader was written only from public documentation:
//! - Microsoft's open specification [MS-NRBF] ".NET Remoting: Binary Format Data Structure"
//!   (https://learn.microsoft.com/openspecs/windows_protocols/ms-nrbf), and
//! - the MIT-licensed pypdn project's description of the PDN layout
//!   (https://github.com/addisonElliott/pypdn).
//! No Paint.NET code was read, decompiled or copied. The writer emits the same record layout,
//! class names and field names that appear in .pdn files saved by Paint.NET 5.2 (observed by
//! reading those files' data with the reader below), and is checked by opening its output in Paint.NET.
//!
//! Layout: `"PDN3"`, a 24-bit little-endian header length, a UTF-8 XML header (size and
//! thumbnail), the bytes `00 01`, an NRBF object graph describing the document and its layers,
//! and then, for each layer in order, the raw pixel block: a format byte (0 = gzip chunks,
//! 1 = uncompressed), a big-endian chunk size, and `(index, size, data)` chunks. Pixels are
//! BGRA with straight (non-premultiplied) alpha.

use crate::document::{Document, Layer, LayerBlend};
use std::collections::HashMap;
use std::io::Read;
use tiny_skia::Pixmap;

#[derive(Clone, Debug)]
enum V {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    Str(String),
    Ref(i32),
}

#[derive(Debug)]
struct Obj {
    class: String,
    members: Vec<(String, V)>,
}

#[derive(Debug)]
enum Entry {
    Obj(Obj),
    Arr(Vec<V>),
    Bytes(#[allow(dead_code)] Vec<u8>),
    Str(String),
}

#[derive(Clone)]
struct Meta {
    class: String,
    names: Vec<String>,
    /// (BinaryTypeEnum, primitive type for Primitive members); `None` when the record has no type info.
    types: Option<Vec<(u8, u8)>>,
}

enum Item {
    V(V),
    Nulls(usize),
    End,
}

struct Nrbf<'a> {
    b: &'a [u8],
    pos: usize,
    objs: HashMap<i32, Entry>,
    metas: HashMap<i32, Meta>,
    root: i32,
    depth: u32,
    trace: Option<Vec<String>>,
}

type R<T> = Result<T, String>;

fn bad<T>(what: &str) -> R<T> {
    Err(format!("This .pdn file could not be read ({what})."))
}

impl<'a> Nrbf<'a> {
    fn log(&mut self, s: String) {
        let d = self.depth as usize;
        if let Some(t) = &mut self.trace {
            t.push(format!("{}{s}", "  ".repeat(d)));
        }
    }
    fn need(&self, n: usize) -> R<()> {
        if self.pos + n > self.b.len() {
            return bad("unexpected end of file");
        }
        Ok(())
    }
    fn bytes(&mut self, n: usize) -> R<&'a [u8]> {
        self.need(n)?;
        let s = &self.b[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn u8(&mut self) -> R<u8> {
        Ok(self.bytes(1)?[0])
    }
    fn i32(&mut self) -> R<i32> {
        Ok(i32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    fn count(&mut self) -> R<usize> {
        let n = self.i32()?;
        if n < 0 || n as usize > self.b.len() {
            return bad("bad count");
        }
        Ok(n as usize)
    }
    fn u64le(&mut self) -> R<u64> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }
    /// LengthPrefixedString: 7-bit encoded length, then UTF-8.
    fn lps(&mut self) -> R<String> {
        let mut len = 0usize;
        for i in 0..5 {
            let b = self.u8()?;
            len |= ((b & 0x7f) as usize) << (7 * i);
            if b & 0x80 == 0 {
                break;
            }
        }
        Ok(String::from_utf8_lossy(self.bytes(len)?).into_owned())
    }

    fn prim(&mut self, t: u8) -> R<V> {
        Ok(match t {
            1 => V::Bool(self.u8()? != 0),
            2 => V::UInt(self.u8()? as u64),
            3 => {
                // UTF-8 encoded char
                let first = self.u8()?;
                let extra = if first >= 0xF0 { 3 } else if first >= 0xE0 { 2 } else if first >= 0xC0 { 1 } else { 0 };
                let mut v = vec![first];
                v.extend_from_slice(self.bytes(extra)?);
                V::Str(String::from_utf8_lossy(&v).into_owned())
            }
            5 => V::Str(self.lps()?), // Decimal as text
            6 => V::Float(f64::from_le_bytes(self.bytes(8)?.try_into().unwrap())),
            7 => V::Int(i16::from_le_bytes(self.bytes(2)?.try_into().unwrap()) as i64),
            8 => V::Int(self.i32()? as i64),
            9 | 12 | 13 => V::Int(self.u64le()? as i64),
            10 => V::Int(self.u8()? as i8 as i64),
            11 => V::Float(f32::from_le_bytes(self.bytes(4)?.try_into().unwrap()) as f64),
            14 => V::UInt(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()) as u64),
            15 => V::UInt(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()) as u64),
            16 => V::UInt(self.u64le()?),
            17 => V::Null,
            18 => V::Str(self.lps()?),
            _ => return bad("unknown primitive type"),
        })
    }

    fn prim_size(t: u8) -> Option<usize> {
        Some(match t {
            1 | 2 | 10 => 1,
            7 | 14 => 2,
            8 | 11 | 15 => 4,
            6 | 9 | 12 | 13 | 16 => 8,
            _ => return None,
        })
    }

    fn class_info(&mut self) -> R<(i32, String, Vec<String>)> {
        let id = self.i32()?;
        let name = self.lps()?;
        let n = self.count()?;
        let mut names = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            names.push(self.lps()?);
        }
        Ok((id, name, names))
    }

    fn member_types(&mut self, n: usize) -> R<Vec<(u8, u8)>> {
        let bts: Vec<u8> = self.bytes(n)?.to_vec();
        let mut out = Vec::with_capacity(n);
        for bt in bts {
            let (extra, desc) = self.additional_info(bt)?;
            if self.trace.is_some() {
                self.log(format!("  type bt={bt} {desc}"));
            }
            out.push((bt, extra));
        }
        Ok(out)
    }

    /// Read the AdditionalInfo for a BinaryTypeEnum; returns the primitive type (or 0).
    fn additional_info(&mut self, bt: u8) -> R<(u8, String)> {
        Ok(match bt {
            0 | 7 => {
                let p = self.u8()?;
                (p, format!("prim={p}"))
            }
            3 => (0, format!("system={}", self.lps()?)),
            4 => {
                let n = self.lps()?;
                let lib = self.i32()?;
                (0, format!("class={n} lib={lib}"))
            }
            1 | 2 | 5 | 6 => (0, String::new()),
            _ => return bad("unknown member type"),
        })
    }

    fn class_body(&mut self, id: i32, meta: Meta) -> R<V> {
        self.depth += 1;
        if self.depth > 256 {
            return bad("nesting too deep");
        }
        let mut members = Vec::with_capacity(meta.names.len());
        for (i, name) in meta.names.iter().enumerate() {
            let v = match meta.types.as_ref().map(|t| t[i]) {
                Some((0, p)) => self.prim(p)?,
                _ => match self.record()? {
                    Item::V(v) => v,
                    Item::Nulls(_) => V::Null,
                    Item::End => return bad("early end"),
                },
            };
            if self.trace.is_some() {
                let s = format!("{name} = {v:?}");
                self.log(s);
            }
            members.push((name.clone(), v));
        }
        self.depth -= 1;
        self.objs.insert(id, Entry::Obj(Obj { class: meta.class.clone(), members }));
        self.metas.insert(id, meta);
        Ok(V::Ref(id))
    }

    fn elements(&mut self, n: usize) -> R<Vec<V>> {
        let mut out = Vec::with_capacity(n.min(1 << 16));
        while out.len() < n {
            match self.record()? {
                Item::V(v) => out.push(v),
                Item::Nulls(k) => out.extend(std::iter::repeat(V::Null).take(k.min(n - out.len()))),
                Item::End => return bad("early end in array"),
            }
        }
        Ok(out)
    }

    fn prim_array(&mut self, id: i32, n: usize, t: u8) -> R<V> {
        if let Some(sz) = Self::prim_size(t) {
            self.need(n.saturating_mul(sz))?;
        }
        let e = if t == 2 {
            Entry::Bytes(self.bytes(n)?.to_vec())
        } else {
            let mut v = Vec::with_capacity(n.min(1 << 16));
            for _ in 0..n {
                v.push(self.prim(t)?);
            }
            Entry::Arr(v)
        };
        self.objs.insert(id, e);
        Ok(V::Ref(id))
    }

    fn record(&mut self) -> R<Item> {
        loop {
            let tag = self.u8()?;
            return Ok(match tag {
                0 => {
                    self.root = self.i32()?;
                    let hb = self.bytes(12)?;
                    self.log(format!("[0 Header] root={} rest={hb:?}", self.root));
                    continue;
                }
                12 => {
                    let id = self.i32()?;
                    let n = self.lps()?;
                    self.log(format!("[12 Library] id={id} {n}"));
                    continue;
                }
                1 => {
                    let id = self.i32()?;
                    let mid = self.i32()?;
                    let Some(meta) = self.metas.get(&mid).cloned() else { return bad("missing class metadata") };
                    self.log(format!("[1 ClassWithId] id={id} meta={mid} {}", meta.class));
                    Item::V(self.class_body(id, meta)?)
                }
                2 | 3 => {
                    let (id, class, names) = self.class_info()?;
                    let lib = if tag == 3 { self.i32()? } else { -1 };
                    self.log(format!("[{tag}] id={id} {class} lib={lib} {names:?}"));
                    Item::V(self.class_body(id, Meta { class, names, types: None })?)
                }
                4 | 5 => {
                    let (id, class, names) = self.class_info()?;
                    self.log(format!("[{tag}] id={id} {class} {names:?}"));
                    let types = self.member_types(names.len())?;
                    let lib = if tag == 5 { self.i32()? } else { -1 };
                    self.log(format!("  lib={lib}"));
                    Item::V(self.class_body(id, Meta { class, names, types: Some(types) })?)
                }
                6 => {
                    let id = self.i32()?;
                    let s = self.lps()?;
                    self.log(format!("[6 String] id={id} {s:?}"));
                    self.objs.insert(id, Entry::Str(s.clone()));
                    Item::V(V::Str(s))
                }
                7 => {
                    let id = self.i32()?;
                    let kind = self.u8()?;
                    let rank = self.count()?;
                    let mut total = 1usize;
                    for _ in 0..rank {
                        total = total.saturating_mul(self.count()?);
                    }
                    if matches!(kind, 3 | 4 | 5) {
                        for _ in 0..rank {
                            self.i32()?;
                        }
                    }
                    let bt = self.u8()?;
                    let (p, desc) = self.additional_info(bt)?;
                    self.log(format!("[7 BinaryArray] id={id} kind={kind} rank={rank} total={total} bt={bt} {desc}"));
                    if bt == 0 {
                        Item::V(self.prim_array(id, total, p)?)
                    } else {
                        let v = self.elements(total)?;
                        self.objs.insert(id, Entry::Arr(v));
                        Item::V(V::Ref(id))
                    }
                }
                8 => {
                    let t = self.u8()?;
                    Item::V(self.prim(t)?)
                }
                9 => Item::V(V::Ref(self.i32()?)),
                10 => Item::V(V::Null),
                11 => {
                    self.log("[11 MessageEnd]".into());
                    Item::End
                }
                13 => Item::Nulls(self.u8()? as usize),
                14 => Item::Nulls(self.count()?),
                15 => {
                    let id = self.i32()?;
                    let n = self.count()?;
                    let t = self.u8()?;
                    self.log(format!("[15 ArraySinglePrimitive] id={id} n={n} prim={t}"));
                    Item::V(self.prim_array(id, n, t)?)
                }
                16 | 17 => {
                    let id = self.i32()?;
                    let n = self.count()?;
                    self.log(format!("[{tag} Array] id={id} n={n}"));
                    let v = self.elements(n)?;
                    self.objs.insert(id, Entry::Arr(v));
                    Item::V(V::Ref(id))
                }
                _ => return bad(&format!("unknown record type {tag}")),
            });
        }
    }

    // ---- graph access helpers

    fn obj(&self, v: &V) -> Option<&Obj> {
        match v {
            V::Ref(id) => match self.objs.get(id) {
                Some(Entry::Obj(o)) => Some(o),
                _ => None,
            },
            _ => None,
        }
    }
    fn arr(&self, v: &V) -> Option<&Vec<V>> {
        match v {
            V::Ref(id) => match self.objs.get(id) {
                Some(Entry::Arr(a)) => Some(a),
                _ => None,
            },
            _ => None,
        }
    }
    fn int(&self, v: &V) -> Option<i64> {
        match v {
            V::Int(i) => Some(*i),
            V::UInt(u) => Some(*u as i64),
            V::Bool(b) => Some(*b as i64),
            V::Float(f) => Some(*f as i64),
            // enums are serialised as an object with a `value__` member
            V::Ref(_) => self.obj(v).and_then(|o| get(o, "value__")).and_then(|x| self.int(x)),
            _ => None,
        }
    }
    fn string(&self, v: &V) -> Option<String> {
        match v {
            V::Str(s) => Some(s.clone()),
            V::Ref(id) => match self.objs.get(id) {
                Some(Entry::Str(s)) => Some(s.clone()),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Find a member by field name. Inherited fields are serialised as `Base+field`.
fn get<'o>(o: &'o Obj, key: &str) -> Option<&'o V> {
    o.members
        .iter()
        .find(|(n, _)| n == key)
        .or_else(|| o.members.iter().find(|(n, _)| n.rsplit('+').next() == Some(key)))
        .map(|(_, v)| v)
}

/// Paint.NET's layer blend modes, in the order of the serialised enum documented by pypdn.
fn blend_from_index(i: i64) -> LayerBlend {
    match i {
        1 => LayerBlend::Multiply,
        2 => LayerBlend::Additive,
        3 => LayerBlend::ColorBurn,
        4 => LayerBlend::ColorDodge,
        5 => LayerBlend::Reflect,
        6 => LayerBlend::Glow,
        7 => LayerBlend::Overlay,
        8 => LayerBlend::Difference,
        9 => LayerBlend::Negation,
        10 => LayerBlend::Lighten,
        11 => LayerBlend::Darken,
        12 => LayerBlend::Screen,
        13 => LayerBlend::Xor,
        _ => LayerBlend::Normal,
    }
}

/// Paint.NET 3.x stores a blend-operation object instead; map it by class name.
fn blend_from_class(name: &str) -> LayerBlend {
    let n = name.rsplit(['.', '+']).next().unwrap_or(name);
    match n.trim_end_matches("BlendOp") {
        "Multiply" => LayerBlend::Multiply,
        "Additive" => LayerBlend::Additive,
        "ColorBurn" => LayerBlend::ColorBurn,
        "ColorDodge" => LayerBlend::ColorDodge,
        "Reflect" => LayerBlend::Reflect,
        "Glow" => LayerBlend::Glow,
        "Overlay" => LayerBlend::Overlay,
        "Difference" => LayerBlend::Difference,
        "Negation" => LayerBlend::Negation,
        "Lighten" => LayerBlend::Lighten,
        "Darken" => LayerBlend::Darken,
        "Screen" => LayerBlend::Screen,
        "Xor" => LayerBlend::Xor,
        _ => LayerBlend::Normal,
    }
}

struct LayerInfo {
    name: String,
    visible: bool,
    opacity: u8,
    blend: LayerBlend,
    stride: usize,
    length: usize,
}

/// Read a .pdn file into a layered document.
pub fn load(bytes: &[u8], doc_name: String) -> Result<Document, String> {
    if bytes.len() < 9 || &bytes[0..4] != b"PDN3" {
        return Err("This is not a Paint.NET image this version of Ferrum Paint can read (expected a PDN3 file).".into());
    }
    let hlen = bytes[4] as usize | (bytes[5] as usize) << 8 | (bytes[6] as usize) << 16;
    let start = 7 + hlen;
    if bytes.len() < start + 2 || bytes[start] != 0 || bytes[start + 1] != 1 {
        return bad("bad header");
    }
    let mut nr = Nrbf { b: bytes, pos: start + 2, objs: HashMap::new(), metas: HashMap::new(), root: 1, depth: 0, trace: None };
    loop {
        match nr.record()? {
            Item::End => break,
            _ => {}
        }
    }
    let root = V::Ref(nr.root);
    let Some(d) = nr.obj(&root) else { return bad("no document") };
    let w = get(d, "width").and_then(|v| nr.int(v)).unwrap_or(0);
    let h = get(d, "height").and_then(|v| nr.int(v)).unwrap_or(0);
    if w <= 0 || h <= 0 || w > 65535 || h > 65535 || (w * h) > 1 << 29 {
        return bad("bad image size");
    }
    let (w, h) = (w as u32, h as u32);
    let Some(list) = get(d, "layers").and_then(|v| nr.obj(v)) else { return bad("no layer list") };
    let size = get(list, "_size").and_then(|v| nr.int(v)).unwrap_or(0).max(0) as usize;
    let Some(items) = get(list, "_items").and_then(|v| nr.arr(v)) else { return bad("no layers") };

    let mut infos = Vec::new();
    for item in items.iter().take(size) {
        let Some(layer) = nr.obj(item) else { return bad("bad layer") };
        // Layer properties: the member (usually `Layer+properties`) whose object has a `name`.
        let props = layer
            .members
            .iter()
            .filter(|(n, _)| n.rsplit('+').next() == Some("properties"))
            .filter_map(|(_, v)| nr.obj(v))
            .find(|o| get(o, "name").is_some());
        let Some(props) = props else { return bad("missing layer properties") };
        let name = get(props, "name").and_then(|v| nr.string(v)).unwrap_or_else(|| format!("Layer {}", infos.len() + 1));
        let visible = get(props, "visible").and_then(|v| nr.int(v)).map(|v| v != 0).unwrap_or(true);
        let opacity = get(props, "opacity").and_then(|v| nr.int(v)).unwrap_or(255).clamp(0, 255) as u8;
        let mut blend = get(props, "blendMode").and_then(|v| nr.int(v)).map(blend_from_index);
        if blend.is_none() {
            // 3.x: BitmapLayer.properties.blendOp is an object whose class names the mode
            blend = layer
                .members
                .iter()
                .filter(|(n, _)| n.rsplit('+').next() == Some("properties"))
                .filter_map(|(_, v)| nr.obj(v))
                .find_map(|o| get(o, "blendOp").and_then(|b| nr.obj(b)).map(|b| blend_from_class(&b.class)));
        }
        let Some(surface) = get(layer, "surface").and_then(|v| nr.obj(v)) else { return bad("missing layer pixels") };
        let stride = get(surface, "stride").and_then(|v| nr.int(v)).unwrap_or(w as i64 * 4).max(0) as usize;
        let length = get(surface, "scan0")
            .and_then(|v| nr.obj(v))
            .and_then(|m| get(m, "length64").or_else(|| get(m, "length")).and_then(|v| nr.int(v)))
            .unwrap_or((stride * h as usize) as i64)
            .max(0) as usize;
        infos.push(LayerInfo { name, visible, opacity, blend: blend.unwrap_or(LayerBlend::Normal), stride, length });
    }
    if infos.is_empty() {
        return bad("no layers");
    }

    // Pixel blocks follow the object graph, one per layer in order.
    let mut pos = nr.pos;
    let mut layers = Vec::with_capacity(infos.len());
    for info in infos {
        let bpp = if w > 0 { info.stride * 8 / w as usize } else { 0 };
        if bpp != 32 && bpp != 24 {
            return bad("unsupported pixel format");
        }
        if info.length < info.stride * h as usize {
            return bad("layer data too short");
        }
        let mut pm = Pixmap::new(w, h).ok_or("out of memory")?;
        // Decode straight into the pixmap buffer when the layout matches (the usual 32-bit case).
        let direct = bpp == 32 && info.stride == w as usize * 4 && info.length == pm.data().len();
        let mut tmp = if direct { Vec::new() } else { vec![0u8; info.length] };
        {
            let dst: &mut [u8] = if direct { pm.data_mut() } else { &mut tmp };
            pos = read_block(bytes, pos, dst)?;
        }
        let data = pm.data_mut();
        if direct {
            for px in data.chunks_exact_mut(4) {
                let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
                px[0] = premul(r, a);
                px[1] = premul(g, a);
                px[2] = premul(b, a);
                px[3] = a;
            }
        } else {
            let bp = bpp / 8;
            for y in 0..h as usize {
                let row = &tmp[y * info.stride..];
                for x in 0..w as usize {
                    let s = &row[x * bp..x * bp + bp];
                    let a = if bp == 4 { s[3] } else { 255 };
                    let o = (y * w as usize + x) * 4;
                    data[o] = premul(s[2], a);
                    data[o + 1] = premul(s[1], a);
                    data[o + 2] = premul(s[0], a);
                    data[o + 3] = a;
                }
            }
        }
        layers.push(Layer { name: info.name, pixmap: pm, visible: info.visible, opacity: info.opacity, blend: info.blend });
    }
    Ok(Document::from_layers(doc_name, w, h, layers, "Open Image"))
}

fn premul(c: u8, a: u8) -> u8 {
    ((c as u32 * a as u32 + 127) / 255) as u8
}

/// Read one chunked pixel block into `dst`; returns the position after it.
fn read_block(b: &[u8], mut pos: usize, dst: &mut [u8]) -> Result<usize, String> {
    let be = |p: usize| -> R<u32> {
        if p + 4 > b.len() {
            return bad("unexpected end of pixel data");
        }
        Ok(u32::from_be_bytes(b[p..p + 4].try_into().unwrap()))
    };
    if pos + 5 > b.len() {
        return bad("unexpected end of pixel data");
    }
    let format = b[pos];
    let chunk = be(pos + 1)? as usize;
    pos += 5;
    if chunk == 0 {
        return bad("bad chunk size");
    }
    let len = dst.len();
    let count = len.div_ceil(chunk);
    let mut seen = vec![false; count];
    for _ in 0..count {
        let idx = be(pos)? as usize;
        let size = be(pos + 4)? as usize;
        pos += 8;
        if idx >= count || seen[idx] || pos + size > b.len() {
            return bad("corrupt pixel chunk");
        }
        seen[idx] = true;
        let off = idx * chunk;
        let out = &mut dst[off..(off + chunk).min(len)];
        let raw = &b[pos..pos + size];
        if format == 0 {
            let mut dec = flate2::read::GzDecoder::new(raw);
            dec.read_exact(out).map_err(|_| "This .pdn file could not be read (bad compressed pixel data).".to_string())?;
        } else {
            let n = out.len().min(raw.len());
            out[..n].copy_from_slice(&raw[..n]);
        }
        pos += size;
    }
    Ok(pos)
}

/// Debug helper: a record-by-record listing of a .pdn file's header and object graph.
#[cfg(test)]
pub fn dump(bytes: &[u8]) -> String {
    let hlen = bytes[4] as usize | (bytes[5] as usize) << 8 | (bytes[6] as usize) << 16;
    let header = String::from_utf8_lossy(&bytes[7..7 + hlen]);
    let header: String = header.chars().take(600).collect();
    let mut nr = Nrbf { b: bytes, pos: 7 + hlen + 2, objs: HashMap::new(), metas: HashMap::new(), root: 1, depth: 0, trace: Some(vec![]) };
    let mut err = String::new();
    loop {
        match nr.record() {
            Ok(Item::End) => break,
            Ok(_) => {}
            Err(e) => {
                err = e;
                break;
            }
        }
    }
    let after = &bytes[nr.pos..(nr.pos + 32).min(bytes.len())];
    format!("XML: {header}\n{}\n{err}\nend at {} of {}; next bytes {after:?}\n", nr.trace.unwrap().join("\n"), nr.pos, bytes.len())
}
// ===================================================================== export

/// Assembly version written into the file (Paint.NET 5.2, which opens older-version files too).
const PDN_VERSION: &str = "5.200.9810.35277";
const KVP: &str = "System.Collections.Generic.KeyValuePair`2[[System.String, mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089],[System.String, mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089]]";
const CHUNK: usize = 256 * 1024;

/// Paint.NET's blend-mode index and blend-op class for a layer blend, if Paint.NET has it.
fn blend_to_pdn(b: LayerBlend) -> Option<(i32, &'static str)> {
    Some(match b {
        LayerBlend::Normal => (0, "Normal"),
        LayerBlend::Multiply => (1, "Multiply"),
        LayerBlend::Additive => (2, "Additive"),
        LayerBlend::ColorBurn => (3, "ColorBurn"),
        LayerBlend::ColorDodge => (4, "ColorDodge"),
        LayerBlend::Reflect => (5, "Reflect"),
        LayerBlend::Glow => (6, "Glow"),
        LayerBlend::Overlay => (7, "Overlay"),
        LayerBlend::Difference => (8, "Difference"),
        LayerBlend::Negation => (9, "Negation"),
        LayerBlend::Lighten => (10, "Lighten"),
        LayerBlend::Darken => (11, "Darken"),
        LayerBlend::Screen => (12, "Screen"),
        LayerBlend::Xor => (13, "Xor"),
        _ => return None,
    })
}

/// Names of layers whose blend mode Paint.NET doesn't have (they are saved as Normal).
pub fn unsupported_blends(doc: &Document) -> Vec<String> {
    doc.layers.iter().filter(|l| blend_to_pdn(l.blend).is_none()).map(|l| format!("{} ({})", l.name, l.blend.name())).collect()
}

/// Member type info for class metadata.
enum T<'s> {
    Prim(u8),
    Str,
    Sys(&'s str),
    Class(&'s str, i32),
    ObjArr,
}

struct W {
    out: Vec<u8>,
    metas: HashMap<String, i32>,
}

impl W {
    fn u8(&mut self, v: u8) {
        self.out.push(v);
    }
    fn i32(&mut self, v: i32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn lps(&mut self, s: &str) {
        let mut n = s.len();
        loop {
            let b = (n & 0x7f) as u8;
            n >>= 7;
            if n == 0 {
                self.u8(b);
                break;
            }
            self.u8(b | 0x80);
        }
        self.out.extend_from_slice(s.as_bytes());
    }
    fn reference(&mut self, id: i32) {
        self.u8(9);
        self.i32(id);
    }
    fn string(&mut self, id: i32, s: &str) {
        self.u8(6);
        self.i32(id);
        self.lps(s);
    }
    /// Start an object: full metadata the first time a class shape is seen, ClassWithId after.
    /// The caller then writes the member values in order.
    fn class(&mut self, id: i32, name: &str, lib: Option<i32>, members: &[(&str, T)]) {
        let mut key = name.to_string();
        for (_, t) in members {
            if let T::Class(c, _) = t {
                key.push('|');
                key.push_str(c);
            }
        }
        if let Some(&meta) = self.metas.get(&key) {
            self.u8(1);
            self.i32(id);
            self.i32(meta);
            return;
        }
        self.metas.insert(key, id);
        self.u8(if lib.is_some() { 5 } else { 4 });
        self.i32(id);
        self.lps(name);
        self.i32(members.len() as i32);
        for (n, _) in members {
            self.lps(n);
        }
        for (_, t) in members {
            self.u8(match t {
                T::Prim(_) => 0,
                T::Str => 1,
                T::Sys(_) => 3,
                T::Class(..) => 4,
                T::ObjArr => 5,
            });
        }
        for (_, t) in members {
            match t {
                T::Prim(p) => self.u8(*p),
                T::Sys(s) => self.lps(s),
                T::Class(c, l) => {
                    self.lps(c);
                    self.i32(*l);
                }
                _ => {}
            }
        }
        if let Some(l) = lib {
            self.i32(l);
        }
    }
    /// Empty `KeyValuePair<string,string>[]` (user metadata).
    fn empty_kvp_array(&mut self, id: i32) {
        self.u8(7);
        self.i32(id);
        self.u8(0); // single-dimensional
        self.i32(1); // rank
        self.i32(0); // length
        self.u8(3); // SystemClass element type
        self.lps(KVP);
    }
}

fn unpremul(c: u8, a: u8) -> u8 {
    if a == 0 {
        0
    } else {
        ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8
    }
}

fn base64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// PNG thumbnail (max 256 px), as shown by Explorer and Paint.NET's open dialog.
fn thumbnail(doc: &Document) -> String {
    let flat = doc.composite();
    let (w, h) = (flat.width(), flat.height());
    let mut img = image::RgbaImage::new(w, h);
    for (d, s) in img.as_mut().chunks_exact_mut(4).zip(flat.data().chunks_exact(4)) {
        d.copy_from_slice(&[unpremul(s[0], s[3]), unpremul(s[1], s[3]), unpremul(s[2], s[3]), s[3]]);
    }
    let scale = (256.0 / w.max(h) as f32).min(1.0);
    let (tw, th) = (((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1));
    let small = image::imageops::thumbnail(&img, tw, th);
    let mut png = Vec::new();
    if small.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).is_err() {
        return String::new();
    }
    base64(&png)
}

/// One layer's pixels as gzip chunks, compressed on all cores.
fn pixel_block(layer: &Layer, out: &mut Vec<u8>) {
    use std::io::Write;
    let src = layer.pixmap.data();
    let mut bgra = vec![0u8; src.len()];
    for (d, s) in bgra.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        let a = s[3];
        d.copy_from_slice(&[unpremul(s[2], a), unpremul(s[1], a), unpremul(s[0], a), a]);
    }
    let chunks: Vec<&[u8]> = bgra.chunks(CHUNK).collect();
    let mut packed: Vec<Vec<u8>> = vec![Vec::new(); chunks.len()];
    let next = std::sync::atomic::AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(chunks.len().max(1));
    let results = std::sync::Mutex::new(&mut packed);
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= chunks.len() {
                    break;
                }
                let mut e = flate2::write::GzEncoder::new(Vec::with_capacity(chunks[i].len() / 8), flate2::Compression::new(5));
                e.write_all(chunks[i]).unwrap();
                let z = e.finish().unwrap();
                results.lock().unwrap()[i] = z;
            });
        }
    });
    out.push(0); // gzip
    out.extend_from_slice(&(CHUNK as u32).to_be_bytes());
    for (i, z) in packed.iter().enumerate() {
        out.extend_from_slice(&(i as u32).to_be_bytes());
        out.extend_from_slice(&(z.len() as u32).to_be_bytes());
        out.extend_from_slice(z);
    }
}

/// Write a layered document as a Paint.NET .pdn file. The result is read back and checked
/// before it is returned.
pub fn save(doc: &Document) -> Result<Vec<u8>, String> {
    let (w, h) = (doc.width as i32, doc.height as i32);
    let n = doc.layers.len();
    if n == 0 {
        return Err("Nothing to save.".into());
    }
    let data_lib = format!("PaintDotNet.Data, Version={PDN_VERSION}, Culture=neutral, PublicKeyToken=null");
    let core_lib = format!("PaintDotNet.Core, Version={PDN_VERSION}, Culture=neutral, PublicKeyToken=null");
    let kvp_arr = format!("{KVP}[]");

    // ids: 1 document, 2 Data library, 3 Core library, then allocated in order
    let mut next = 4;
    let mut id = || {
        next += 1;
        next - 1
    };
    let (list_id, ver_id, meta_id, items_id, empty_id) = (id(), id(), id(), id(), id());
    struct Ids {
        layer: i32,
        bprops: i32,
        surface: i32,
        props: i32,
        name: i32,
        op: i32,
        mem: i32,
    }
    let ids: Vec<Ids> = (0..n).map(|_| Ids { layer: id(), bprops: id(), surface: id(), props: id(), name: id(), op: id(), mem: id() }).collect();

    let mut wr = W { out: Vec::with_capacity(4096 + n * 512), metas: HashMap::new() };
    wr.u8(0); // header: root 1, header id -1, version 1.0
    wr.i32(1);
    wr.i32(-1);
    wr.i32(1);
    wr.i32(0);
    for (lid, name) in [(2, &data_lib), (3, &core_lib)] {
        wr.u8(12);
        wr.i32(lid);
        wr.lps(name);
    }
    wr.class(
        1,
        "PaintDotNet.Document",
        Some(2),
        &[("isDisposed", T::Prim(1)), ("layers", T::Class("PaintDotNet.LayerList", 2)), ("width", T::Prim(8)), ("height", T::Prim(8)), ("savedWith", T::Sys("System.Version")), ("userMetadataItems", T::Sys(&kvp_arr))],
    );
    wr.u8(0);
    wr.reference(list_id);
    wr.i32(w);
    wr.i32(h);
    wr.reference(ver_id);
    wr.reference(meta_id);

    wr.class(
        list_id,
        "PaintDotNet.LayerList",
        Some(2),
        &[("parent", T::Class("PaintDotNet.Document", 2)), ("ArrayList+_items", T::ObjArr), ("ArrayList+_size", T::Prim(8)), ("ArrayList+_version", T::Prim(8))],
    );
    wr.reference(1);
    wr.reference(items_id);
    wr.i32(n as i32);
    wr.i32(n as i32);

    wr.class(ver_id, "System.Version", None, &[("_Major", T::Prim(8)), ("_Minor", T::Prim(8)), ("_Build", T::Prim(8)), ("_Revision", T::Prim(8))]);
    for v in PDN_VERSION.split('.') {
        wr.i32(v.parse().unwrap_or(0));
    }
    wr.empty_kvp_array(meta_id);

    // ArrayList backing store: capacity 4, 8, 16, ... with trailing nulls
    let cap = n.max(4).next_power_of_two();
    wr.u8(16);
    wr.i32(items_id);
    wr.i32(cap as i32);
    for l in &ids {
        wr.reference(l.layer);
    }
    let nulls = cap - n;
    if nulls == 1 {
        wr.u8(10);
    } else if nulls > 1 && nulls < 256 {
        wr.u8(13);
        wr.u8(nulls as u8);
    } else if nulls >= 256 {
        wr.u8(14);
        wr.i32(nulls as i32);
    }

    for l in &ids {
        wr.class(
            l.layer,
            "PaintDotNet.BitmapLayer",
            Some(2),
            &[
                ("properties", T::Class("PaintDotNet.BitmapLayer+BitmapLayerProperties", 2)),
                ("surface", T::Class("PaintDotNet.Surface", 3)),
                ("Layer+isDisposed", T::Prim(1)),
                ("Layer+width", T::Prim(8)),
                ("Layer+height", T::Prim(8)),
                ("Layer+properties", T::Class("PaintDotNet.Layer+LayerProperties", 2)),
            ],
        );
        wr.reference(l.bprops);
        wr.reference(l.surface);
        wr.u8(0);
        wr.i32(w);
        wr.i32(h);
        wr.reference(l.props);
    }

    let mut enum_id = -1_000_000;
    for (layer, l) in doc.layers.iter().zip(&ids) {
        let (mode, op) = blend_to_pdn(layer.blend).unwrap_or((0, "Normal"));
        let op_class = format!("PaintDotNet.UserBlendOps+{op}BlendOp");
        wr.class(l.bprops, "PaintDotNet.BitmapLayer+BitmapLayerProperties", Some(2), &[("blendOp", T::Class(&op_class, 2))]);
        wr.reference(l.op);

        wr.class(l.surface, "PaintDotNet.Surface", Some(3), &[("width", T::Prim(8)), ("height", T::Prim(8)), ("stride", T::Prim(8)), ("scan0", T::Class("PaintDotNet.MemoryBlock", 3))]);
        wr.i32(w);
        wr.i32(h);
        wr.i32(w * 4);
        wr.reference(l.mem);

        wr.class(
            l.props,
            "PaintDotNet.Layer+LayerProperties",
            Some(2),
            &[
                ("name", T::Str),
                ("userMetadataItems", T::Sys(&kvp_arr)),
                ("visible", T::Prim(1)),
                ("isBackground", T::Prim(1)),
                ("opacity", T::Prim(2)),
                ("blendMode", T::Class("PaintDotNet.LayerBlendMode", 3)),
            ],
        );
        wr.string(l.name, &layer.name);
        wr.reference(empty_id);
        wr.u8(layer.visible as u8);
        wr.u8(0);
        wr.u8(layer.opacity);
        // the enum is a boxed value type, written inline
        enum_id -= 1;
        wr.class(enum_id, "PaintDotNet.LayerBlendMode", Some(3), &[("value__", T::Prim(8))]);
        wr.i32(mode);
    }
    for (layer, l) in doc.layers.iter().zip(&ids) {
        let (_, op) = blend_to_pdn(layer.blend).unwrap_or((0, "Normal"));
        wr.class(l.op, &format!("PaintDotNet.UserBlendOps+{op}BlendOp"), Some(2), &[]);
        wr.class(l.mem, "PaintDotNet.MemoryBlock", Some(3), &[("length64", T::Prim(9)), ("hasParent", T::Prim(1)), ("deferred", T::Prim(1))]);
        wr.out.extend_from_slice(&(w as i64 * h as i64 * 4).to_le_bytes());
        wr.u8(0);
        wr.u8(1);
    }
    wr.empty_kvp_array(empty_id);
    wr.u8(11); // MessageEnd

    let xml = format!(
        "<pdnImage width=\"{w}\" height=\"{h}\" layers=\"{n}\" savedWithVersion=\"{PDN_VERSION}\"><custom><thumb png=\"{}\" /></custom></pdnImage>",
        thumbnail(doc)
    );
    if xml.len() >= 1 << 24 {
        return Err("Thumbnail too large.".into());
    }
    let mut out = Vec::with_capacity(xml.len() + wr.out.len() + doc.layers.iter().map(|l| l.pixmap.data().len() / 4).sum::<usize>());
    out.extend_from_slice(b"PDN3");
    out.extend_from_slice(&(xml.len() as u32).to_le_bytes()[..3]);
    out.extend_from_slice(xml.as_bytes());
    out.extend_from_slice(&[0, 1]);
    out.extend_from_slice(&wr.out);
    for layer in &doc.layers {
        pixel_block(layer, &mut out);
    }

    // read it back before anything touches the disk
    let check = load(&out, doc.name.clone()).map_err(|e| format!("Internal error writing .pdn: {e}"))?;
    if check.width != doc.width || check.height != doc.height || check.layers.len() != n {
        return Err("Internal error writing .pdn (verification failed).".into());
    }
    Ok(out)
}