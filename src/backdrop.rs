//! What sits behind the preview's terminal window: the design's three
//! process-ink circles multiplied onto paper. GPUI has neither blend modes
//! nor backdrop filters, so the backdrop is drawn once into an image here —
//! sharp, and blurred for each `background-blur` setting.

use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::{Image, ImageFormat};
use image::{Rgba, RgbaImage};

/// The preview box, in points.
pub const W: u32 = 552;
pub const H: u32 = 360;

/// (x, y, diameter, color) in points.
const CIRCLES: &[(f32, f32, f32, u32)] = &[(300., 30., 190., 0x0088b0), (390., 150., 170., 0xd6006c), (120., 210., 150., 0xedbb00)];
const PAPER: u32 = 0xeae7e7;
const OPACITY: f32 = 0.9;

fn rgb(v: u32) -> [f32; 3] {
    [((v >> 16) & 255) as f32 / 255., ((v >> 8) & 255) as f32 / 255., (v & 255) as f32 / 255.]
}

/// The paper with its circles at `scale` pixels per point.
fn paint(scale: u32) -> Vec<[f32; 3]> {
    let (w, h) = (W * scale, H * scale);
    let paper = rgb(PAPER);
    let mut px = vec![paper; (w * h) as usize];
    for &(x, y, d, color) in CIRCLES {
        let ink = rgb(color);
        let (r, cx, cy) = (d / 2. * scale as f32, (x + d / 2.) * scale as f32, (y + d / 2.) * scale as f32);
        let (x0, x1) = (((cx - r).floor().max(0.)) as u32, ((cx + r).ceil() as u32).min(w));
        let (y0, y1) = (((cy - r).floor().max(0.)) as u32, ((cy + r).ceil() as u32).min(h));
        for yy in y0..y1 {
            for xx in x0..x1 {
                let dist = ((xx as f32 + 0.5 - cx).powi(2) + (yy as f32 + 0.5 - cy).powi(2)).sqrt();
                let cover = (r - dist + 0.5).clamp(0., 1.) * OPACITY;
                if cover <= 0. {
                    continue;
                }
                let p = &mut px[(yy * w + xx) as usize];
                // mix-blend-mode: multiply, at the circle's opacity.
                for i in 0..3 {
                    p[i] = p[i] * (1. - cover) + p[i] * ink[i] * cover;
                }
            }
        }
    }
    px
}

/// Three box blurs ≈ a gaussian of `sigma`.
fn blur(px: &mut [[f32; 3]], w: usize, h: usize, sigma: f32) {
    let r = ((sigma * sigma * 4. + 1.).sqrt() / 2.).round().max(1.) as usize;
    let mut tmp = px.to_vec();
    for _ in 0..3 {
        box_pass(px, &mut tmp, w, h, r, true);
        box_pass(&tmp, px, w, h, r, false);
    }
}

fn box_pass(src: &[[f32; 3]], dst: &mut [[f32; 3]], w: usize, h: usize, r: usize, horizontal: bool) {
    let (len, lines) = if horizontal { (w, h) } else { (h, w) };
    let at = |line: usize, i: usize| if horizontal { line * w + i } else { i * w + line };
    let span = (2 * r + 1) as f32;
    for line in 0..lines {
        let get = |i: isize| src[at(line, i.clamp(0, len as isize - 1) as usize)];
        let mut acc = [0f32; 3];
        for i in -(r as isize)..=(r as isize) {
            let p = get(i);
            for c in 0..3 {
                acc[c] += p[c];
            }
        }
        for i in 0..len {
            dst[at(line, i)] = acc.map(|a| a / span);
            let (add, sub) = (get(i as isize + r as isize + 1), get(i as isize - r as isize));
            for c in 0..3 {
                acc[c] += add[c] - sub[c];
            }
        }
    }
}

fn saturate(px: &mut [[f32; 3]], s: f32) {
    for p in px {
        let l = 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
        for c in p.iter_mut() {
            *c = (l + (*c - l) * s).clamp(0., 1.);
        }
    }
}

fn encode(px: &[[f32; 3]], w: u32, h: u32) -> Arc<Image> {
    let mut img = RgbaImage::new(w, h);
    for (i, p) in img.pixels_mut().enumerate() {
        let c = px[i];
        *p = Rgba([(c[0] * 255.).round() as u8, (c[1] * 255.).round() as u8, (c[2] * 255.).round() as u8, 255]);
    }
    let mut bytes = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).expect("png encode");
    Arc::new(Image::from_bytes(ImageFormat::Png, bytes))
}

/// The backdrop images, made on first use.
#[derive(Default)]
pub struct Backdrops(HashMap<&'static str, Arc<Image>>);

impl Backdrops {
    /// `kind`: "sharp", or a `background-blur` value.
    pub fn get(&mut self, kind: &'static str) -> Arc<Image> {
        self.0
            .entry(kind)
            .or_insert_with(|| {
                if kind == "sharp" {
                    return encode(&paint(2), W * 2, H * 2);
                }
                // CSS blur(N) is a gaussian of sigma N.
                let (sigma, sat) = match kind {
                    "macos-glass-regular" => (28., 1.8),
                    "macos-glass-clear" => (10., 1.5),
                    _ => (20., 1.),
                };
                let mut px = paint(1);
                blur(&mut px, W as usize, H as usize, sigma);
                if sat != 1. {
                    saturate(&mut px, sat);
                }
                encode(&px, W, H)
            })
            .clone()
    }
}
