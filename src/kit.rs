//! The design system's components (`.btn`, `.seg`, `.tag`, `.radio`) as
//! GPUI elements.

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::{Pal, RADIUS_MD};

pub type OnClick = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, PartialEq)]
pub enum Btn {
    Primary,
    Secondary,
    Ghost,
}

/// `.btn`: serif semibold 14px; the caller sets padding and the click.
pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, kind: Btn, disabled: bool, p: &Pal) -> Stateful<Div> {
    let d = div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(14.))
        .line_height(relative(1.2))
        .rounded(px(RADIUS_MD))
        .border_1()
        .border_color(transparent_black())
        .whitespace_nowrap()
        .child(label.into());
    let (accent, a600, a700, ink7, ink14) = (p.accent, p.a(600), p.a(700), p.ink(0.07), p.ink(0.14));
    let d = match kind {
        Btn::Primary => d.bg(accent).text_color(p.bg).when(!disabled, |d| d.hover(move |s| s.bg(a600)).active(move |s| s.bg(a700))),
        Btn::Secondary => {
            d.border_color(p.divider).text_color(p.text).when(!disabled, |d| d.hover(move |s| s.bg(ink7)).active(move |s| s.bg(ink14)))
        }
        Btn::Ghost => {
            let (h, a) = (accent.opacity(0.1), accent.opacity(0.18));
            d.text_color(accent).px(px(5.)).when(!disabled, |d| d.hover(move |s| s.bg(h)).active(move |s| s.bg(a)))
        }
    };
    if disabled { d.opacity(0.45).cursor_not_allowed() } else { d.cursor_pointer() }
}

pub struct SegItem {
    pub label: SharedString,
    pub checked: bool,
    /// The small 新 mark after the label.
    pub new: bool,
    pub on_click: OnClick,
}

/// `.seg`: a row of options in one hairline frame; the checked one is
/// filled with the accent.
pub fn seg(id: &str, items: Vec<SegItem>, small: bool, p: &Pal) -> Div {
    let mut row = div().flex().flex_none().overflow_hidden().border_1().border_color(p.divider).rounded(px(RADIUS_MD));
    for (i, it) in items.into_iter().enumerate() {
        let hover = p.ink(0.07);
        row = row.child(
            div()
                .id(SharedString::from(format!("{id}-{i}")))
                .flex()
                .items_center()
                .gap(px(6.))
                .when(small, |d| d.px(px(10.)).py(px(4.)).text_size(px(12.)))
                .when(!small, |d| d.px(px(12.)).py(px(7.)).text_size(px(13.)))
                .line_height(relative(1.3))
                .whitespace_nowrap()
                .cursor_pointer()
                .when(i > 0, |d| d.border_l_1().border_color(p.divider))
                .when(it.checked, |d| d.bg(p.accent).text_color(p.bg))
                .when(!it.checked, |d| d.hover(move |s| s.bg(hover)))
                .on_click(it.on_click)
                .child(it.label)
                .when(it.new, |d| d.child(div().text_size(px(10.)).opacity(0.85).child("新"))),
        );
    }
    row
}

#[derive(Clone, Copy)]
pub enum Tag {
    Accent2,
    Neutral,
    /// Outlined in the second accent: 弃用写法.
    Outline2,
}

/// `.tag`: 11px, tinted from a ramp.
pub fn tag(text: impl Into<SharedString>, kind: Tag, p: &Pal) -> Div {
    let d = div().flex_none().px(px(10.)).py(px(3.)).rounded(px(1.5)).text_size(px(11.)).line_height(relative(1.2)).whitespace_nowrap();
    match kind {
        Tag::Accent2 => d.bg(p.s(100)).text_color(p.s(800)),
        Tag::Neutral => d.bg(p.n(100)).text_color(p.n(800)),
        Tag::Outline2 => d.py(px(2.)).px(px(9.)).border_1().border_color(p.s(300)).text_color(p.s(700)),
    }
    .child(text.into())
}

/// `.radio .dot`: a ring, filled with a gap when checked.
pub fn radio_dot(checked: bool, p: &Pal) -> Div {
    let d = div().size(px(16.)).flex_none().rounded_full().flex().items_center().justify_center();
    if checked {
        d.border(px(1.5)).border_color(p.accent).child(div().size(px(5.)).rounded_full().bg(p.accent))
    } else {
        d.border(px(1.5)).border_color(p.divider)
    }
}

/// A square check box: accent-filled with a tick when on.
pub fn checkbox(checked: bool, p: &Pal) -> Div {
    let d = div().size(px(16.)).flex_none().rounded(px(3.)).flex().items_center().justify_center().text_size(px(11.)).line_height(px(16.));
    if checked { d.bg(p.accent).text_color(p.bg).child("✓") } else { d.border(px(1.5)).border_color(p.divider) }
}
