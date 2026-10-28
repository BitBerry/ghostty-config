//! The right column: a terminal window drawn with the values being edited,
//! over the design's ink-circle backdrop, and the config file as it would be
//! written.

use std::sync::Arc;

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::app::Editor;
use crate::backdrop;
use crate::config::{self, Mark};
use crate::ghostty;
use crate::kit::{SegItem, seg};
use crate::rows::FONTS;
use crate::schema::{self, Rgb, Val};
use crate::theme::{MONO, Pal, PalExt as _, RADIUS_LG, RADIUS_MD, rgba, rgbc};

/// The colors in effect.
pub struct Look {
    pub bg: Rgb,
    pub fg: Rgb,
    pub cursor: Rgb,
    pub palette: [Rgb; 16],
    pub theme_bg: Rgb,
    pub theme_fg: Rgb,
    pub theme_cursor: Rgb,
    pub divider_default: Rgb,
}

/// One run of terminal text.
struct Seg {
    t: String,
    c: Hsla,
    b: Option<Hsla>,
    bold: bool,
}

fn styled(segs: &[Seg], backgrounds: bool) -> StyledText {
    let mut text = String::new();
    let mut runs = Vec::new();
    for s in segs {
        let start = text.len();
        text.push_str(&s.t);
        runs.push((
            start..text.len(),
            HighlightStyle {
                color: Some(s.c),
                background_color: if backgrounds { s.b } else { None },
                font_weight: s.bold.then_some(FontWeight::BOLD),
                ..Default::default()
            },
        ));
    }
    StyledText::new(text).with_highlights(runs)
}

impl Editor {
    pub fn look(&mut self) -> Look {
        let name = ghostty::pick_theme(self.value("theme").text(), self.dark);
        let th = self.theme_cache.get(&self.install, &name).unwrap_or_else(ghostty::default_colors);
        let mut palette = th.palette;
        if self.value("palette").flag() == Some(true) {
            for (i, c) in &self.loaded.palette {
                palette[*i] = *c;
            }
        }
        let bg = self.value("background").color().unwrap_or(th.bg);
        let fg = self.value("foreground").color().unwrap_or(th.fg);
        let theme_cursor = th.cursor.unwrap_or(fg);
        let cursor = self.value("cursor-color").color().unwrap_or(theme_cursor);
        Look { bg, fg, cursor, palette, theme_bg: th.bg, theme_fg: th.fg, theme_cursor, divider_default: bg.mix(fg, 0.22) }
    }

    pub fn palette_now(&mut self) -> Vec<Rgb> {
        let look = self.look();
        let mut out = vec![look.bg, look.fg];
        out.extend(look.palette);
        out
    }

    pub fn render_right(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.pal().clone();
        let q = self.query(cx);
        let note =
            (q.is_empty() && schema::cat(self.cat).is_some_and(|c| !c.visual)).then_some("这一类设置不改变外观，效果见左侧每项的说明。");
        let split = self.split;
        let modes = vec![
            SegItem {
                label: "单窗格".into(),
                checked: !split,
                new: false,
                on_click: Box::new(cx.listener(|this, _, _, cx| {
                    this.split = false;
                    cx.notify();
                })),
            },
            SegItem {
                label: "分屏".into(),
                checked: split,
                new: false,
                on_click: Box::new(cx.listener(|this, _, _, cx| {
                    this.split = true;
                    cx.notify();
                })),
            },
        ];
        let preview = self.render_preview(&p);
        let file = self.render_file(&p);
        div()
            .w(px(600.))
            .flex_none()
            .bg(p.surface)
            .pt(px(22.))
            .px(px(24.))
            .pb(px(24.))
            .flex()
            .flex_col()
            .gap(px(14.))
            .min_h_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .min_h(px(34.))
                    .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child("实时预览"))
                    .child(seg("pv-mode", modes, true, &p))
                    .child(div().flex_1())
                    .child(div().text_size(px(12.)).text_color(p.n(700)).font_family(MONO).child(self.focus.unwrap_or(""))),
            )
            .child(preview)
            .when_some(note, |d, n| d.child(div().mt(px(-4.)).text_size(px(13.)).text_color(p.n(700)).child(n)))
            .child(file)
    }

    fn num(&self, key: &str) -> f32 {
        let o = schema::opt(key);
        self.value(key).num().or_else(|| o.and_then(|o| o.default().num())).unwrap_or(0.) as f32
    }

    fn render_preview(&mut self, p: &Pal) -> impl IntoElement + use<> {
        let look = self.look();
        let (bg, fg, pal) = (look.bg, look.fg, look.palette);
        let c = |i: usize| rgbc(pal[i]);
        let fgc = rgbc(fg);
        let dark = bg.lum() < 0.5;
        let op = self.num("background-opacity").clamp(0., 1.);
        let pane_bg = rgba(bg, op);
        let tbs = match self.value("macos-titlebar-style") {
            Val::Text(s) => s,
            _ => "transparent".into(),
        };
        let blur: Option<&'static str> = match self.value("background-blur") {
            Val::Text(s) if s == "true" => Some("true"),
            Val::Text(s) if s == "macos-glass-regular" => Some("macos-glass-regular"),
            Val::Text(s) if s == "macos-glass-clear" => Some("macos-glass-clear"),
            Val::Raw(s) if s.parse::<u32>().is_ok_and(|n| n > 0) => Some("true"),
            _ => None,
        };
        let fs = self.num("font-size").clamp(6., 40.);
        let lh = 1.32 * (1. + self.num("adjust-cell-height") / 100.);
        let line_h = px((fs * lh).max(fs * 0.8));
        let family = self.value("font-family").text().to_string();
        let font: SharedString = if family.is_empty() { FONTS[0].into() } else { family.into() };
        let lig = self.value("font-feature").flag() != Some(false);
        let thick = self.value("font-thicken").flag() == Some(true);
        let (padx, pady) = (self.num("window-padding-x"), self.num("window-padding-y"));
        let cs = self.value("cursor-style").text().to_string();
        let blinks = self.value("cursor-style-blink").text() != "false";
        let cc = rgba(look.cursor, self.num("cursor-opacity").clamp(0., 1.));
        let dh = self.value("drag-handle").text().to_string();
        let handle = self.split && dh != "never";
        let handle_op = if dh == "always" { 0.55 } else { 0.2 };
        let divider = self.value("split-divider-color").color().unwrap_or(look.divider_default);
        let dim = 1. - self.num("unfocused-split-opacity").clamp(0., 1.);
        let guide = self.focus.is_some_and(|k| k.starts_with("window-padding"));
        let version =
            self.install.version.as_deref().map(|v| v.split('.').take(2).collect::<Vec<_>>().join(".")).unwrap_or_else(|| "1.4".into());
        let sharp = self.backdrops.get("sharp");
        let blurred = blur.filter(|_| op < 1.).map(|k| self.backdrops.get(k));

        let seg = |t: &str, ci: Option<usize>, bold: bool| Seg { t: t.into(), c: ci.map(c).unwrap_or(fgc), b: None, bold };
        let s = |t: &str| seg(t, None, false);
        let prompt = || vec![seg("~/code/ghostty-config", Some(4), false), seg(" main", Some(5), false), seg(" ❯ ", Some(2), false)];
        let with = |mut a: Vec<Seg>, b: Vec<Seg>| {
            a.extend(b);
            a
        };
        let blocks = |a: usize, b: usize| (a..b).map(|i| Seg { t: "   ".into(), c: fgc, b: Some(c(i)), bold: false }).collect::<Vec<_>>();
        let lines: Vec<Vec<Seg>> = vec![
            with(prompt(), vec![s("ls")]),
            vec![s("Cargo.toml  README.md  "), seg("assets/  src/  target/", Some(4), true)],
            with(prompt(), vec![s("git status -s")]),
            vec![seg(" M ", Some(3), false), s("src/app.rs")],
            vec![seg(" M ", Some(3), false), s("src/preview.rs")],
            vec![seg("?? ", Some(1), false), s("src/schema.rs")],
            with(prompt(), vec![s("head -2 src/preview.rs")]),
            vec![seg("fn ", Some(5), false), seg("render", Some(4), false), s("(cfg: &Config) -> Element {")],
            vec![
                seg("    if ", Some(5), false),
                s("cfg.opacity != "),
                seg("1.0", Some(3), false),
                s(" && cfg.blur >= "),
                seg("20", Some(3), false),
                s(" {"),
            ],
            vec![s(" ")],
            blocks(0, 8),
            blocks(8, 16),
            vec![s(" ")],
        ];
        let lines2: Vec<Vec<Seg>> = vec![
            vec![seg("~/code", Some(4), false), seg(" ❯ ", Some(2), false), s("cargo run")],
            vec![seg("   Compiling ", Some(2), true), s("gpui v0.2.0")],
            vec![seg("    Finished ", Some(2), true), s("dev in 4.21s")],
            vec![seg("     Running ", Some(2), true), s("ghostty-config")],
        ];
        let last_prompt = prompt();

        // The cursor: one cell, shaped by cursor-style.
        let cell_w = px(fs * 0.6);
        let cursor = div().w(cell_w).h(line_h).flex_none().when(!blinks || self.blink_on, |d| match cs.as_str() {
            "bar" => d.child(div().w(px(2.)).h_full().bg(cc)),
            "underline" => d.flex().flex_col().justify_end().child(div().h(px(2.)).w_full().bg(cc)),
            "block_hollow" => d.border_1().border_color(cc),
            _ => d.bg(cc),
        });

        let text_block = |lines: &[Vec<Seg>], backgrounds: bool| {
            let mut col = div().flex().flex_col();
            for l in lines {
                col = col.child(div().whitespace_nowrap().h(line_h).child(styled(l, backgrounds)));
            }
            col
        };
        let pane = || {
            div()
                .flex_1()
                .min_w_0()
                .relative()
                .overflow_hidden()
                .bg(pane_bg)
                .px(px(padx))
                .py(px(pady))
                .font_family(font.clone())
                .text_size(px(fs))
                .line_height(line_h)
                .text_color(fgc)
                .when(!lig, |d| d.font_features(FontFeatures(Arc::new(vec![("calt".into(), 0), ("liga".into(), 0), ("dlig".into(), 0)]))))
                .when(handle, |d| {
                    d.child(
                        div()
                            .absolute()
                            .top(px(4.))
                            .left(relative(0.5))
                            .ml(px(-17.))
                            .w(px(34.))
                            .h(px(4.))
                            .rounded(px(2.))
                            .bg(fgc)
                            .opacity(handle_op),
                    )
                })
        };
        // font-thicken: the text again, nudged right — GPUI has no text stroke.
        let thicken = |lines: &[Vec<Seg>]| div().absolute().top_0().left(px(0.45)).child(text_block(lines, false));

        let mut body = div()
            .relative()
            .child(text_block(&lines, true))
            .child(div().flex().h(line_h).child(div().whitespace_nowrap().child(styled(&last_prompt, true))).child(cursor));
        if thick {
            body = body.child(thicken(&lines));
        }
        if guide {
            body = body.child(div().absolute().top_0().left_0().right_0().bottom_0().border_1().border_dashed().border_color(p.a(500)));
        }
        let mut panes = div().flex_1().min_h_0().flex().child(pane().child(body));
        if self.split {
            let mut body2 = div().relative().child(text_block(&lines2, true));
            if thick {
                body2 = body2.child(thicken(&lines2));
            }
            panes = panes
                .child(div().w(px(1.)).flex_none().bg(rgbc(divider)))
                .child(pane().child(body2).child(div().absolute().top_0().left_0().right_0().bottom_0().bg(rgbc(bg)).opacity(dim)));
        }

        let dots = || {
            div()
                .flex()
                .flex_none()
                .gap(px(7.))
                .children([0xff5f57, 0xfebc2e, 0x28c840].map(|c| div().size(px(11.)).rounded_full().bg(rgb(c))))
        };
        let (tb_bg, tb_fg) = match tbs.as_str() {
            "native" => (
                if dark { rgb(0x2e2c2c).into() } else { rgb(0xe4e2e1).into() },
                if dark { rgb(0xd6d4d3).into() } else { rgb(0x3b3939).into() },
            ),
            "tabs" => (rgba(bg.mix(if dark { Rgb(0) } else { Rgb(0xffffff) }, 0.3), op), rgba(fg, 0.75)),
            _ => (pane_bg, rgba(fg, 0.75)),
        };
        let titlebar = (tbs != "hidden").then(|| {
            let bar = div()
                .h(px(30.))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(10.))
                .bg(tb_bg)
                .text_color(tb_fg)
                .font_family(MONO)
                .text_size(px(11.))
                .child(dots());
            if tbs == "tabs" {
                bar.child(
                    div()
                        .flex_1()
                        .h_full()
                        .flex()
                        .gap(px(2.))
                        .pt(px(4.))
                        .child(div().flex_1().flex().items_center().justify_center().rounded_t(px(6.)).bg(pane_bg).child("zsh"))
                        .child(div().flex_1().flex().items_center().justify_center().opacity(0.7).child("cargo run")),
                )
            } else {
                bar.child(div().flex_1().flex().justify_center().pr(px(47.)).child("~/code/ghostty-config"))
            }
        });

        let big = |d: Div| {
            d.font_weight(FontWeight::SEMIBOLD)
                .text_size(px(104.))
                .line_height(px(98.8))
                .child(div().child("Ghostty"))
                .child(div().child(version.clone()))
        };
        let mut window = div()
            .absolute()
            .top(px(22.))
            .bottom(px(22.))
            .left(px(28.))
            .right(px(28.))
            .rounded(px(10.))
            .overflow_hidden()
            .shadow(p.shadow_lg());
        if let Some(img_b) = blurred {
            // The backdrop as seen through the glass: blurred, and the
            // headline smeared into soft copies.
            let mut layer = div()
                .absolute()
                .left(px(-28.))
                .top(px(-22.))
                .w(px(backdrop::W as f32))
                .h(px(backdrop::H as f32))
                .child(img(img_b).size_full());
            for (r, n) in [(5., 8), (11., 10)] {
                for k in 0..n {
                    let a = k as f32 / n as f32 * std::f32::consts::TAU;
                    layer = layer.child(big(div()
                        .absolute()
                        .left(px(34. + r * a.cos()))
                        .top(px(26. + r * a.sin()))
                        .text_color(p.text.opacity(0.07))));
                }
            }
            window = window.child(layer);
        }
        window = window.child(div().absolute().top_0().left_0().right_0().bottom_0().flex().flex_col().children(titlebar).child(panes));

        div()
            .relative()
            .h(px(backdrop::H as f32))
            .w(px(backdrop::W as f32))
            .flex_none()
            .rounded(px(RADIUS_LG))
            .overflow_hidden()
            .bg(p.n(200))
            .child(img(sharp).absolute().top_0().left_0().size_full())
            .child(big(div().absolute().left(px(34.)).top(px(26.)).text_color(p.text)))
            .child(window)
    }

    fn render_file(&mut self, p: &Pal) -> impl IntoElement + use<> {
        let out = self.rendered();
        let focus = self.focus.and_then(schema::opt);
        let follow = self.cfg_follow.take().and_then(schema::opt);
        let hits = |o: &schema::Opt, key: Option<&str>| {
            key.is_some_and(|k| k == o.key || (o.key == "scrollback-limit-bytes" && k == "scrollback-limit"))
        };
        if let Some(o) = follow
            && let Some(i) = out.iter().position(|l| hits(o, l.key.as_deref()))
        {
            self.cfg_scroll.scroll_to_item(i);
        }
        let legacy_sb = self.legacy.contains("scrollback-limit-bytes");
        let legacy_cos = self.legacy.contains("copy-on-select");
        let mut lines = div()
            .id("cfg")
            .flex_1()
            .min_h_0()
            .overflow_scroll()
            .track_scroll(&self.cfg_scroll)
            .py(px(12.))
            .rounded(px(RADIUS_MD))
            .bg(p.bg)
            .font_family(MONO)
            .text_size(px(12.))
            .line_height(px(21.));
        if out.is_empty() {
            lines = lines.child(div().px(px(14.)).text_color(p.n(600)).child("# 配置文件是空的，所有选项都用默认值"));
        }
        for l in &out {
            let legacy = l.mark != Mark::Removed
                && match l.key.as_deref() {
                    Some("scrollback-limit") => legacy_sb,
                    Some("copy-on-select") => legacy_cos && matches!(config::parse_line(&l.text).value.as_str(), "true" | "false"),
                    _ => false,
                };
            let mark = match l.mark {
                Mark::Same => " ",
                Mark::Changed => "~",
                Mark::Added => "+",
                Mark::Removed => "−",
            };
            let color = if l.mark == Mark::Removed || config::is_comment(&l.text) {
                p.n(600)
            } else if legacy {
                p.s(700)
            } else {
                p.text
            };
            let lit = focus.is_some_and(|o| hits(o, l.key.as_deref()));
            let text = if l.text.is_empty() { " ".to_string() } else { l.text.clone() };
            lines = lines.child(
                div()
                    .flex()
                    .gap(px(10.))
                    .px(px(14.))
                    .whitespace_nowrap()
                    .text_color(color)
                    .when(lit, |d| d.bg(p.a(100)))
                    .child(div().w(px(8.)).flex_none().text_color(p.accent).child(mark))
                    .child(div().when(l.mark == Mark::Removed, |d| d.line_through()).child(text))
                    .when(legacy, |d| d.child(div().text_color(p.s(700)).child("   # 1.4 已弃用"))),
            );
        }
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(12.))
                    .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child("配置文件"))
                    .child(
                        div().text_size(px(12.)).text_color(p.n(700)).child("只改动你调整过的行，其余原样保留 · 左侧符号表示未保存的改动"),
                    ),
            )
            .child(lines)
    }
}
