//! Offscreen snapshots for visual checks (`--features snapshot`).
//!
//! `GHOSTTY_CONFIG_SNAPSHOT=<dir>` renders each view in
//! `GHOSTTY_CONFIG_SNAPSHOT_VIEWS` (default: every category) to
//! `<dir>/<view>.png`, then quits. A view is a category id, optionally with
//! `+` steps: `window+focus=window-padding-x`, `font+set=font-size:18`,
//! `theme+migrate`, `clip+split`, `theme+save`.

use std::time::Duration;

use gpui_kit::*;

use crate::app::Editor;
use crate::schema::{self, Val};

pub fn run(dir: String, window: AnyWindowHandle, view: Entity<Editor>, cx: &mut App) {
    let views =
        std::env::var("GHOSTTY_CONFIG_SNAPSHOT_VIEWS").unwrap_or_else(|_| "theme,font,cursor,window,split,clip,scroll,kbd,sec,new".into());
    let views: Vec<String> = views.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_millis(1200)).await;
        for name in views {
            let _ = window.update(cx, |_, window, cx| view.update(cx, |e, cx| setup(e, &name, window, cx)));
            cx.background_executor().timer(Duration::from_millis(600)).await;
            let _ = window.update(cx, |_, window, cx| {
                // An occluded window gets no frame callbacks; draw explicitly.
                window.draw(cx).clear(cx);
            });
            let _ = window.update(cx, |_, window, _| match window.render_to_image() {
                Ok(img) => {
                    let path = format!("{dir}/{name}.png");
                    match img.save(&path) {
                        Ok(()) => eprintln!("snapshot {path}"),
                        Err(e) => eprintln!("snapshot {path}: {e}"),
                    }
                }
                Err(e) => eprintln!("render_to_image: {e:#}"),
            });
        }
        cx.update(|cx| cx.quit());
    })
    .detach();
}

fn setup(e: &mut Editor, name: &str, window: &mut Window, cx: &mut Context<Editor>) {
    let mut steps = name.split('+');
    let cat = steps.next().unwrap_or("theme");
    e.snapshot_cat(cat, window, cx);
    e.hover_locked = true;
    e.focus = None;
    for step in steps {
        match step.split_once('=') {
            Some(("focus", k)) => e.focus = schema::opt(k).map(|o| o.key),
            Some(("set", kv)) => {
                if let Some((k, v)) = kv.split_once(':')
                    && let Some(o) = schema::opt(k)
                {
                    let val = match o.parse(v) {
                        Val::Raw(r) if r == "default" => o.default(),
                        v => v,
                    };
                    e.set(o.key, val, cx);
                }
            }
            Some(("search", q)) => e.search.update(cx, |s, cx| s.set_value(q.to_string(), window, cx)),
            Some(("themes", q)) => e.theme_search.update(cx, |s, cx| s.set_value(q.to_string(), window, cx)),
            _ if step == "migrate" => e.legacy.clear(),
            _ if step == "split" => e.split = true,
            _ if step == "single" => e.split = false,
            _ if step == "save" => e.save(cx),
            _ if step == "toast" => e.show_toast("已写入配置，Ghostty 已重新加载", false, cx),
            _ => eprintln!("snapshot: unknown step {step}"),
        }
    }
    cx.notify();
}

impl Editor {
    pub fn snapshot_cat(&mut self, cat: &str, window: &mut Window, cx: &mut Context<Self>) {
        let id = if cat == "new" { Some("new") } else { schema::cat(cat).map(|c| c.id) };
        if let Some(id) = id {
            self.cat = id;
            self.split = id == "split";
        }
        self.search.update(cx, |s, cx| s.set_value("", window, cx));
        self.list_scroll.set_offset(point(px(0.), px(0.)));
    }
}
