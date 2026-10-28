//! The Broadsheet design system's tokens (the design's `styles.css`), as a
//! GPUI global, projected onto GPUI Kit's theme so stock components (the
//! search inputs, the color picker) wear the same colors.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

fn hex(v: u32) -> Hsla {
    rgb(v).into()
}

#[derive(Clone)]
pub struct Pal {
    pub bg: Hsla,
    pub surface: Hsla,
    pub text: Hsla,
    pub accent: Hsla,
    pub divider: Hsla,
    n: [Hsla; 9],
    a: [Hsla; 9],
    s: [Hsla; 9],
    pub font: SharedString,
}

impl Global for Pal {}

impl Pal {
    pub fn broadsheet() -> Self {
        let text = hex(0x201e1d);
        Self {
            bg: hex(0xf3f2f2),
            surface: hex(0xeae9e9),
            text,
            accent: hex(0x0088b0),
            divider: text.opacity(0.16),
            n: [0xf8f4f4, 0xeae7e7, 0xd7d3d3, 0xbab6b6, 0x9b9797, 0x7d7979, 0x605d5d, 0x444141, 0x2d2b2b].map(hex),
            a: [0xe9f8ff, 0xcbeeff, 0x99e0ff, 0x62c5ee, 0x38a6cf, 0x1186ac, 0x006786, 0x004961, 0x0a303e].map(hex),
            s: [0xfff1f4, 0xffdee6, 0xffc0d0, 0xff90b1, 0xff458e, 0xd82071, 0xaa0b56, 0x790e3d, 0x4b1528].map(hex),
            font: "Source Serif 4".into(),
        }
    }

    /// Neutral ramp, `step` in 100..=900.
    pub fn n(&self, step: usize) -> Hsla {
        self.n[step / 100 - 1]
    }

    /// Accent (cyan) ramp.
    pub fn a(&self, step: usize) -> Hsla {
        self.a[step / 100 - 1]
    }

    /// Second accent (magenta) ramp.
    pub fn s(&self, step: usize) -> Hsla {
        self.s[step / 100 - 1]
    }

    /// `color-mix(in srgb, text N%, transparent)`.
    pub fn ink(&self, alpha: f32) -> Hsla {
        self.text.opacity(alpha)
    }

    fn shadow(&self, alpha: f32, y: f32, blur: f32) -> BoxShadow {
        BoxShadow {
            color: hex(0x2d2b2b).opacity(alpha),
            offset: point(px(0.), px(y)),
            blur_radius: px(blur),
            spread_radius: px(0.),
            inset: false,
        }
    }

    pub fn shadow_sm(&self) -> Vec<BoxShadow> {
        vec![self.shadow(0.14, 1., 2.)]
    }

    pub fn shadow_md(&self) -> Vec<BoxShadow> {
        vec![self.shadow(0.16, 3., 10.)]
    }

    pub fn shadow_lg(&self) -> Vec<BoxShadow> {
        vec![self.shadow(0.22, 12., 32.)]
    }
}

pub trait PalExt {
    fn pal(&self) -> &Pal;
}

impl PalExt for App {
    fn pal(&self) -> &Pal {
        self.global::<Pal>()
    }
}

pub const RADIUS_MD: f32 = 2.;
pub const RADIUS_LG: f32 = 4.;
pub const MONO: &str = "JetBrains Mono";

pub fn apply(cx: &mut App) {
    let p = Pal::broadsheet();
    Theme::change(ThemeMode::Light, None, cx);
    let pc = p.clone();
    Theme::update(cx, move |t| {
        let p = pc;
        t.font_family = p.font.clone();
        t.font_size = px(14.);
        t.mono_font_family = MONO.into();
        t.radius = px(RADIUS_MD);
        t.radius_lg = px(RADIUS_LG);
        t.shadow = true;
        let c = &mut t.colors;
        c.background = p.bg;
        c.foreground = p.text;
        c.border = p.divider;
        c.input = p.divider;
        c.ring = p.accent;
        c.caret = p.accent;
        c.selection = p.accent.opacity(0.3);
        c.primary = p.accent;
        c.primary_hover = p.a(600);
        c.primary_active = p.a(700);
        c.primary_foreground = p.bg;
        c.secondary = p.surface;
        c.secondary_hover = p.ink(0.07);
        c.secondary_active = p.ink(0.14);
        c.secondary_foreground = p.text;
        c.muted = p.surface;
        c.muted_foreground = p.ink(0.65);
        c.accent = p.a(100);
        c.accent_foreground = p.text;
        c.popover = p.bg;
        c.popover_foreground = p.text;
        c.list = p.bg;
        c.list_hover = p.a(100);
        c.list_active = p.a(100);
        c.list_active_border = p.accent;
        c.link = p.a(700);
        c.link_hover = p.a(800);
        c.link_active = p.a(800);
        c.button = p.surface;
        c.button_hover = p.ink(0.07);
        c.button_active = p.ink(0.14);
        c.button_foreground = p.text;
        c.button_primary = p.accent;
        c.button_primary_hover = p.a(600);
        c.button_primary_active = p.a(700);
        c.button_primary_foreground = p.bg;
        c.tab_bar_segmented = p.n(200);
        c.tab_active = p.bg;
        c.tab_active_foreground = p.text;
        c.scrollbar = p.bg.opacity(0.);
        c.scrollbar_thumb = p.ink(0.18);
        c.scrollbar_thumb_hover = p.ink(0.3);
    });
    cx.set_global(p);
}

/// The fonts the design and the preview use, bundled.
pub fn load_fonts(cx: &mut App) {
    let fonts: Vec<std::borrow::Cow<'static, [u8]>> = vec![
        include_bytes!("../assets/fonts/SourceSerif4-400.ttf").as_slice().into(),
        include_bytes!("../assets/fonts/SourceSerif4-600.ttf").as_slice().into(),
        include_bytes!("../assets/fonts/JetBrainsMono-400.ttf").as_slice().into(),
        include_bytes!("../assets/fonts/JetBrainsMono-700.ttf").as_slice().into(),
        include_bytes!("../assets/fonts/FiraCode-400.ttf").as_slice().into(),
        include_bytes!("../assets/fonts/FiraCode-700.ttf").as_slice().into(),
        include_bytes!("../assets/fonts/IBMPlexMono-400.ttf").as_slice().into(),
        include_bytes!("../assets/fonts/IBMPlexMono-700.ttf").as_slice().into(),
        include_bytes!("../assets/fonts/SourceCodePro-400.ttf").as_slice().into(),
        include_bytes!("../assets/fonts/SourceCodePro-700.ttf").as_slice().into(),
    ];
    if let Err(e) = cx.text_system().add_fonts(fonts) {
        log::warn!("font registration failed: {e}");
    }
}

/// Ghostty colors in GPUI terms.
pub fn rgbc(c: crate::schema::Rgb) -> Hsla {
    hex(c.0)
}

pub fn rgba(c: crate::schema::Rgb, alpha: f32) -> Hsla {
    hex(c.0).opacity(alpha)
}
