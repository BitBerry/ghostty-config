//! Draws the app icon: the preview's three ink circles on paper, with a
//! small terminal window over them. `cargo run --example icon -- out.png`

use image::{Rgba, RgbaImage};

const S: u32 = 1024;

fn rgb(v: u32) -> [f32; 3] {
    [((v >> 16) & 255) as f32 / 255., ((v >> 8) & 255) as f32 / 255., (v & 255) as f32 / 255.]
}

/// Coverage of a rounded rectangle at a pixel center.
fn rrect(x: f32, y: f32, (l, t, r, b): (f32, f32, f32, f32), rad: f32) -> f32 {
    let cx = x.clamp(l + rad, r - rad);
    let cy = y.clamp(t + rad, b - rad);
    let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
    (rad - d + 0.5).clamp(0., 1.)
}

fn circle(x: f32, y: f32, (cx, cy, r): (f32, f32, f32)) -> f32 {
    (r - ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() + 0.5).clamp(0., 1.)
}

/// Distance from a point to a segment.
fn seg(x: f32, y: f32, (ax, ay): (f32, f32), (bx, by): (f32, f32)) -> f32 {
    let (dx, dy) = (bx - ax, by - ay);
    let t = (((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)).clamp(0., 1.);
    ((x - ax - t * dx).powi(2) + (y - ay - t * dy).powi(2)).sqrt()
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "icon.png".into());
    let mut img = RgbaImage::new(S, S);
    // macOS icon grid: an 824 square with ~185 corner radius.
    let tile = (100., 100., 924., 924.);
    let paper = rgb(0xf3f2f2);
    let circles = [((560., 330., 230.), 0x0088b0), ((660., 600., 210.), 0xd6006c), ((380., 640., 190.), 0xedbb00)];
    let term = (230., 300., 800., 760.);
    let term_bg = rgb(0x1a1b26);
    for y in 0..S {
        for x in 0..S {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let a = rrect(fx, fy, tile, 185.);
            if a <= 0. {
                continue;
            }
            let mut c = paper;
            for ((cx, cy, r), col) in circles {
                let k = circle(fx, fy, (cx, cy, r)) * 0.9;
                let ic = rgb(col);
                for i in 0..3 {
                    c[i] = c[i] * (1. - k) + c[i] * ic[i] * k;
                }
            }
            // The terminal: translucent dark glass over the inks.
            let t = rrect(fx, fy, term, 36.) * 0.88;
            for i in 0..3 {
                c[i] = c[i] * (1. - t) + term_bg[i] * t;
            }
            if t > 0. {
                // Title bar dots.
                for (i, col) in [0xff5f57u32, 0xfebc2e, 0x28c840].iter().enumerate() {
                    let k = circle(fx, fy, (280. + i as f32 * 44., 345., 13.));
                    let dc = rgb(*col);
                    for j in 0..3 {
                        c[j] = c[j] * (1. - k) + dc[j] * k;
                    }
                }
                // The prompt chevron and a block cursor.
                let d = seg(fx, fy, (300., 470.), (380., 540.)).min(seg(fx, fy, (380., 540.), (300., 610.)));
                let k = (17. - d + 0.5).clamp(0., 1.);
                let acc = rgb(0x62c5ee);
                for j in 0..3 {
                    c[j] = c[j] * (1. - k) + acc[j] * k;
                }
                let k = rrect(fx, fy, (430., 500., 500., 620.), 6.);
                for j in 0..3 {
                    c[j] = c[j] * (1. - k) + paper[j] * k;
                }
            }
            img.put_pixel(x, y, Rgba([(c[0] * 255.) as u8, (c[1] * 255.) as u8, (c[2] * 255.) as u8, (a * 255.) as u8]));
        }
    }
    img.save(&out).expect("save icon");
    eprintln!("icon {out}");
}
