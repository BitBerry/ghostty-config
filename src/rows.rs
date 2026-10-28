//! The middle column: the legacy banner, the heading and one row per option,
//! each with the control its kind calls for.

use gpui_kit::component::color_picker::ColorPicker;
use gpui_kit::component::input::Input;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::app::Editor;
use crate::ghostty;
use crate::kit::{Btn, SegItem, Tag, button, radio_dot, seg, tag};
use crate::schema::{self, Kind, Num, OPTS, Opt, Val};
use crate::theme::{MONO, Pal, PalExt as _, RADIUS_LG, RADIUS_MD, rgbc};

/// The design's eight: (value, name).
const FEATURED: &[(&str, &str)] = &[
    ("", "Ghostty 默认"),
    ("TokyoNight", "TokyoNight"),
    ("Catppuccin Mocha", "Catppuccin Mocha"),
    ("Gruvbox Dark", "Gruvbox Dark"),
    ("Dracula", "Dracula"),
    ("Nord", "Nord"),
    ("Catppuccin Latte", "Catppuccin Latte"),
    ("Rose Pine Dawn", "Rose Pine Dawn"),
];

pub const FONTS: &[&str] = &["JetBrains Mono", "Fira Code", "IBM Plex Mono", "Source Code Pro", "Menlo"];

/// Width of the text column; controls stay this narrow.
const MEASURE: f32 = 440.;

impl Editor {
    pub fn render_list(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.pal().clone();
        let q = self.query(cx);
        let opts: Vec<&'static Opt> = OPTS.iter().filter(|o| self.listed(o, &q)).collect();
        let show_cat = !q.is_empty() || self.cat == "new";
        let (heading, intro) = if !q.is_empty() {
            (format!("搜索“{}”", self.search.read(cx).value().trim()), format!("找到 {} 项", opts.len()))
        } else if self.cat == "new" {
            ("1.4 新增与变化".to_string(), "这些选项在 Ghostty 1.4 中首次出现，或行为发生了变化。".to_string())
        } else {
            let c = schema::cat(self.cat).unwrap_or(&schema::CATS[0]);
            (c.name.to_string(), c.intro.to_string())
        };
        // The column's inner width, for the theme grid.
        let width = f32::from(window.viewport_size().width) - 210. - 600. - 80.;
        let mut col = div()
            .id("list")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.list_scroll)
            .pt(px(30.))
            .px(px(40.))
            .pb(px(80.))
            .flex()
            .flex_col()
            .gap(px(6.));
        if let Some(banner) = self.render_legacy(&p, cx) {
            col = col.child(banner);
        }
        col = col
            .child(div().font_weight(FontWeight::SEMIBOLD).text_size(px(25.)).line_height(relative(1.12)).child(heading))
            .child(div().mb(px(18.)).text_size(px(14.)).text_color(p.n(700)).child(intro));
        if opts.is_empty() {
            col = col.child(div().text_size(px(14.)).text_color(p.n(700)).child("没有匹配的设置。试试配置键，例如 font-size。"));
        }
        for o in opts {
            col = col.child(self.render_row(o, show_cat, width, &p, cx));
        }
        col
    }

    fn render_legacy(&self, p: &Pal, cx: &mut Context<Self>) -> Option<AnyElement> {
        let mut items = Vec::new();
        if self.legacy.contains("scrollback-limit-bytes") {
            items.push("scrollback-limit → scrollback-limit-bytes".to_string());
        }
        if self.legacy.contains("copy-on-select") {
            let raw =
                self.lines.iter().rev().find(|l| l.key.as_deref() == Some("copy-on-select")).map(|l| l.value.clone()).unwrap_or_default();
            let to = if raw == "false" { "none" } else { "clipboard" };
            items.push(format!("copy-on-select = {raw} → {to}"));
        }
        if items.is_empty() {
            return None;
        }
        let mut list = div().flex_1().flex().flex_col().gap(px(4.)).child(
            div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(format!("你的配置里有 {} 处 1.4 已弃用的写法", items.len())),
        );
        for it in items {
            list = list.child(div().text_size(px(12.)).font_family(MONO).child(it));
        }
        Some(
            div()
                .flex()
                .items_center()
                .gap(px(16.))
                .px(px(16.))
                .py(px(14.))
                .mb(px(18.))
                .rounded(px(RADIUS_MD))
                .bg(p.s(100))
                .text_color(p.s(800))
                .child(list)
                .child(
                    button("migrate", "一键迁移", Btn::Secondary, false, p)
                        .px(px(18.))
                        .py(px(10.))
                        .border_color(p.s(300))
                        .text_color(p.s(800))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.legacy.clear();
                            this.cfg_follow = Some("scrollback-limit-bytes");
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }

    fn render_row(&mut self, o: &'static Opt, show_cat: bool, width: f32, p: &Pal, cx: &mut Context<Self>) -> AnyElement {
        let v = self.value(o.key);
        let key = o.key;
        let focused = self.focus == Some(key);
        let modified = !o.is_default(&v);
        let mut head = div().flex().items_center().gap(px(8.)).flex_wrap().min_h(px(26.));
        if show_cat && let Some(c) = schema::cat(o.cat) {
            head = head.child(div().text_size(px(12.)).text_color(p.n(700)).child(format!("{} ·", c.name)));
        }
        head = head
            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(o.label))
            .when(o.new, |d| d.child(tag("1.4 新增", Tag::Accent2, p)))
            .when(o.changed, |d| d.child(tag("1.4 行为变化", Tag::Accent2, p)))
            .when_some(o.tag, |d, t| d.child(tag(t, Tag::Neutral, p)))
            .when(self.legacy.contains(key), |d| d.child(tag("弃用写法", Tag::Outline2, p)))
            .child(div().flex_1());
        if modified {
            head = head.child(
                button(SharedString::from(format!("reset-{key}")), "恢复默认", Btn::Ghost, false, p)
                    .text_size(px(13.))
                    .py(px(2.))
                    .px(px(6.))
                    .on_click(cx.listener(move |this, _, _, cx| this.set(key, o.default(), cx))),
            );
        }
        let mut row = div()
            .id(SharedString::from(format!("row-{key}")))
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(18.))
            .py(px(16.))
            .mx(px(-18.))
            .mb(px(6.))
            .rounded(px(RADIUS_MD))
            .when(focused, |d| d.bg(p.ink(0.04)))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered && !this.hover_locked && this.focus != Some(key) {
                    this.focus = Some(key);
                    this.cfg_follow = Some(key);
                    cx.notify();
                }
            }))
            .child(head);
        if self.show_keys {
            row = row.child(
                div()
                    .flex()
                    .gap(px(12.))
                    .mt(px(-4.))
                    .text_size(px(12.))
                    .text_color(p.n(700))
                    .child(div().font_family(MONO).text_color(p.a(700)).child(key))
                    .child(format!("默认：{}", o.describe(&o.default()))),
            );
        }
        if self.show_help && !o.help.is_empty() {
            row = row.child(div().max_w(px(MEASURE)).text_size(px(13.)).line_height(relative(1.55)).text_color(p.n(800)).child(o.help));
        }
        if let Val::Raw(raw) = &v {
            row = row.child(
                div()
                    .max_w(px(MEASURE))
                    .text_size(px(12.))
                    .text_color(p.s(700))
                    .child(format!("文件里写的是 {key} = {raw}，这个控件无法表示；调整后会替换它。")),
            );
        }
        let control = match &o.kind {
            Kind::Enum(_) | Kind::Bool { .. } => self.render_choices(o, &v, p, cx).into_any_element(),
            Kind::Num(n) => self.render_slider(o, n, &v, p, cx).into_any_element(),
            Kind::Color => self.render_color(o, &v, p).into_any_element(),
            Kind::Theme => self.render_themes(&v, width, p, cx).into_any_element(),
            Kind::Font => self.render_fonts(&v, p, cx).into_any_element(),
            Kind::Palette => self.render_palette(&v, p).into_any_element(),
        };
        row.child(control).into_any_element()
    }

    fn render_choices(&self, o: &'static Opt, v: &Val, p: &Pal, cx: &mut Context<Self>) -> impl IntoElement {
        let key = o.key;
        // (value, label, description, new)
        let choices: Vec<(Val, &str, &str, bool)> = match &o.kind {
            Kind::Bool { on, off } => vec![(Val::Bool(true), "开", *on, false), (Val::Bool(false), "关", *off, false)],
            Kind::Enum(chs) => chs.iter().map(|c| (Val::Text(c.v.into()), c.label, c.desc, c.new)).collect(),
            _ => Vec::new(),
        };
        let desc = choices.iter().find(|c| c.0 == *v).map(|c| c.2).unwrap_or("");
        let items = choices
            .iter()
            .map(|(val, label, _, new)| {
                let val = val.clone();
                SegItem {
                    label: SharedString::from(label.to_string()),
                    checked: *v == val,
                    new: *new,
                    on_click: Box::new(cx.listener(move |this, _, _, cx| this.set(key, val.clone(), cx))),
                }
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .items_start()
            .mt(px(4.))
            .child(seg(key, items, false, p))
            .when(!desc.is_empty(), |d| d.child(div().max_w(px(MEASURE)).text_size(px(13.)).text_color(p.a(700)).child(desc)))
    }

    fn render_slider(&self, o: &'static Opt, n: &'static Num, v: &Val, p: &Pal, cx: &mut Context<Self>) -> impl IntoElement {
        let key = o.key;
        let x = v.num().or_else(|| o.default().num()).unwrap_or(n.min);
        let t = (((x - n.min) / (n.max - n.min)).clamp(0., 1.)) as f32;
        let bounds = self.slider_bounds.clone();
        let accent = p.accent;
        let track = div()
            .id(SharedString::from(format!("slider-{key}")))
            .relative()
            .flex_1()
            .h(px(18.))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    this.drag = Some(key);
                    this.slide(key, ev.position.x, cx);
                }),
            )
            .child(div().absolute().left_0().right_0().top(px(7.)).h(px(4.)).rounded_full().bg(p.n(300)))
            // The thumb travels between the ends, inset by its radius.
            .child(
                div()
                    .absolute()
                    .left(px(8.))
                    .right(px(8.))
                    .top_0()
                    .bottom_0()
                    .child(div().absolute().left(px(-8.)).right(relative(1. - t)).top(px(7.)).h(px(4.)).rounded_full().bg(accent))
                    .child(
                        div()
                            .absolute()
                            .top(px(1.))
                            .left(relative(t))
                            .ml(px(-8.))
                            .size(px(16.))
                            .rounded_full()
                            .bg(accent)
                            .border_2()
                            .border_color(p.bg)
                            .shadow(p.shadow_sm()),
                    )
                    .child(
                        canvas(
                            move |b, _, _| {
                                bounds.borrow_mut().insert(key, b);
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    ),
            );
        let text = match v {
            Val::Num(x) => (n.fmt)(*x),
            Val::Raw(r) => r.clone(),
            _ => String::new(),
        };
        div()
            .flex()
            .items_center()
            .gap(px(14.))
            .max_w(px(MEASURE))
            .mt(px(4.))
            .child(track)
            .child(div().min_w(px(84.)).flex().justify_end().font_family(MONO).text_size(px(13.)).child(text))
    }

    /// Move the dragged slider to the pointer.
    pub fn slide(&mut self, key: &'static str, x: Pixels, cx: &mut Context<Self>) {
        let Some(b) = self.slider_bounds.borrow().get(key).copied() else { return };
        let Some(o) = schema::opt(key) else { return };
        let Kind::Num(n) = &o.kind else { return };
        let w = f32::from(b.size.width).max(1.);
        let t = ((f32::from(x) - f32::from(b.origin.x)) / w).clamp(0., 1.) as f64;
        let v = Val::Num(n.snap(n.min + t * (n.max - n.min)));
        if self.value(key) != v {
            self.set(key, v, cx);
        }
    }

    fn render_color(&mut self, o: &'static Opt, v: &Val, p: &Pal) -> impl IntoElement + use<> {
        let featured = self.palette_now().iter().map(|c| rgbc(*c)).collect::<Vec<_>>();
        let look_hex = self.picker_synced.get(o.key).copied().flatten().map(|c| c.hex()).unwrap_or_default();
        let picker = self.pickers.get(o.key).cloned();
        let set = v.color().is_some();
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .mt(px(4.))
            .when_some(picker, |d, picker| d.child(ColorPicker::new(&picker).featured_colors(featured)))
            .child(div().font_family(MONO).text_size(px(13.)).child(look_hex))
            .when(!set && !matches!(v, Val::Raw(_)), |d| d.child(div().text_size(px(13.)).text_color(p.n(700)).child("跟随主题")))
    }

    fn render_themes(&mut self, v: &Val, width: f32, p: &Pal, cx: &mut Context<Self>) -> impl IntoElement {
        let q = self.theme_search.read(cx).value().trim().to_lowercase();
        let cur = v.text().to_string();
        let picked = ghostty::pick_theme(&cur, self.dark);
        let pair = picked != cur;
        let tiles: Vec<(String, String)> = if q.is_empty() {
            let mut t: Vec<(String, String)> = FEATURED.iter().map(|(v, n)| (v.to_string(), n.to_string())).collect();
            if !FEATURED.iter().any(|(v, _)| *v == cur) {
                let name = if pair { format!("{picked}（随外观）") } else { cur.clone() };
                t.insert(0, (cur.clone(), name));
                t.truncate(8);
            }
            t
        } else {
            self.themes.iter().filter(|n| n.to_lowercase().contains(&q)).take(12).map(|n| (n.clone(), n.clone())).collect()
        };
        let matches = if q.is_empty() { 0 } else { self.themes.iter().filter(|n| n.to_lowercase().contains(&q)).count() };
        let overridden = self.value("palette").flag() == Some(true) && !self.loaded.palette.is_empty();
        let tile_w = ((width - 3. * 12.) / 4.).max(80.);
        let mut grid = div().flex().flex_wrap().gap_x(px(12.)).gap_y(px(14.)).mt(px(6.));
        for (i, (val, name)) in tiles.into_iter().enumerate() {
            let colors = if val.is_empty() {
                ghostty::default_colors()
            } else {
                let n = ghostty::pick_theme(&val, self.dark);
                self.theme_cache.get(&self.install, &n).unwrap_or_else(ghostty::default_colors)
            };
            let on = val == cur;
            let mut sw = div().flex().gap(px(3.));
            for c in &colors.palette[1..=6] {
                sw = sw.child(div().size(px(9.)).rounded(px(1.)).bg(rgbc(*c)));
            }
            let pick = val.clone();
            grid = grid.child(
                div()
                    .id(SharedString::from(format!("theme-{i}")))
                    .w(px(tile_w))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.set("theme", Val::Text(pick.clone()), cx)))
                    .child(
                        div()
                            .rounded(px(RADIUS_LG + 3.))
                            .when(on, |d| d.border_2().border_color(p.accent).p(px(2.)))
                            .when(!on, |d| d.border_1().border_color(p.divider).p(px(3.)))
                            .child(
                                div()
                                    .h(px(58.))
                                    .flex()
                                    .flex_col()
                                    .justify_between()
                                    .px(px(10.))
                                    .py(px(8.))
                                    .rounded(px(RADIUS_LG))
                                    .bg(rgbc(colors.bg))
                                    .child(
                                        div()
                                            .font_family(MONO)
                                            .text_size(px(11.))
                                            .line_height(relative(1.2))
                                            .text_color(rgbc(colors.fg))
                                            .child("~ ❯ ls"),
                                    )
                                    .child(sw),
                            ),
                    )
                    .child(
                        div().text_size(px(12.)).line_height(relative(1.3)).when(on, |d| d.font_weight(FontWeight::SEMIBOLD)).child(name),
                    ),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(grid)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(div().w(px(260.)).child(Input::new(&self.theme_search).text_size(px(13.)).h(px(32.)).bg(p.bg).cleanable(true)))
                    .when(!q.is_empty(), |d| {
                        d.child(div().text_size(px(13.)).text_color(p.n(700)).child(match matches {
                            0 => "没有匹配的主题".to_string(),
                            n if n > 12 => format!("找到 {n} 套，显示前 12 套"),
                            n => format!("找到 {n} 套"),
                        }))
                    }),
            )
            .when(pair, |d| {
                d.child(
                    div().text_size(px(13.)).text_color(p.n(700)).child(format!("当前写的是 theme = {cur}，预览按系统外观显示 {picked}。")),
                )
            })
            .when(overridden, |d| {
                d.child(div().max_w(px(MEASURE)).text_size(px(13.)).text_color(p.s(700)).child(format!(
                    "配置里的 {} 条 palette 覆盖仍然生效，换主题时这些颜色不会变；可在下方「调色板覆盖」里删除。",
                    self.loaded.palette.len()
                )))
            })
    }

    fn render_fonts(&self, v: &Val, p: &Pal, cx: &mut Context<Self>) -> impl IntoElement {
        let cur = v.text().to_string();
        let mut names: Vec<String> = FONTS.iter().map(|s| s.to_string()).collect();
        if !cur.is_empty() && !names.contains(&cur) {
            names.push(cur.clone());
        }
        let mut list = div().flex().flex_col().gap(px(10.)).mt(px(6.));
        for (i, name) in names.into_iter().enumerate() {
            let checked = name == cur;
            let missing = name != "JetBrains Mono" && self.fonts.as_ref().is_some_and(|f| !f.contains(&name));
            let pick = name.clone();
            list = list.child(
                div()
                    .id(SharedString::from(format!("font-{i}")))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .text_size(px(14.))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.set("font-family", Val::Text(pick.clone()), cx)))
                    .child(radio_dot(checked, p))
                    .child(div().w(px(128.)).child(name.clone()))
                    .child(
                        div()
                            .font_family(SharedString::from(name.clone()))
                            .text_size(px(14.))
                            .text_color(p.n(800))
                            .child("fn main() -> Ok != 0x1F"),
                    )
                    .when(name == "JetBrains Mono", |d| d.child(tag("内置", Tag::Neutral, p)))
                    .when(missing, |d| d.child(tag("未安装", Tag::Accent2, p))),
            );
        }
        if !self.loaded.fallbacks.is_empty() {
            list = list.child(
                div().text_size(px(12.)).text_color(p.n(700)).child(format!("后备字体：{}（保持不变）", self.loaded.fallbacks.join("、"))),
            );
        }
        list
    }

    fn render_palette(&self, v: &Val, p: &Pal) -> impl IntoElement {
        let on = v.flag() == Some(true);
        let mut row = div().flex().items_center().gap(px(3.)).mt(px(4.));
        if on {
            for i in 0..16 {
                let c = self.loaded.palette.get(&i).copied();
                row = row.child(
                    div()
                        .size(px(18.))
                        .rounded(px(1.))
                        .when_some(c, |d, c| d.bg(rgbc(c)))
                        .when(c.is_none(), |d| d.border_1().border_color(p.divider)),
                );
            }
            row =
                row.child(div().ml(px(9.)).text_size(px(13.)).text_color(p.n(700)).child(format!("{} 条覆盖", self.loaded.palette.len())));
        } else {
            let pending = !self.loaded.palette.is_empty();
            row = row.child(div().text_size(px(13.)).text_color(p.n(700)).child(if pending {
                "保存后删除这些覆盖，改用主题的调色板。"
            } else {
                "没有覆盖，完全使用主题的调色板。"
            }));
        }
        row
    }
}
