//! Ghostty 配置: a visual editor for Ghostty's config file. It edits the
//! file in place, keeps everything it doesn't manage, checks the result
//! with Ghostty and asks the running Ghostty to reload.

mod app;
mod backdrop;
mod config;
mod ghostty;
mod kit;
mod preview;
mod rows;
mod schema;
#[cfg(feature = "snapshot")]
mod snapshot;
mod theme;

use gpui_kit::*;

use crate::app::{ClearSearch, CloseWindow, Discard, Editor, FocusSearch, Quit, RevealConfig, Save, ToggleHelp, ToggleKeys};

fn main() {
    if std::env::var_os("RUST_LOG").is_none() {
        // SAFETY: before any other thread starts.
        unsafe { std::env::set_var("RUST_LOG", "ghostty_config=info") };
    }
    env_logger();
    let app = gpui_kit::application();
    app.run(|cx| {
        gpui_kit::init(cx);
        theme::load_fonts(cx);
        theme::apply(cx);
        cx.bind_keys([
            KeyBinding::new("cmd-s", Save, None),
            KeyBinding::new("cmd-f", FocusSearch, None),
            KeyBinding::new("escape", ClearSearch, Some("Editor")),
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-w", CloseWindow, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &CloseWindow, cx| cx.quit());
        set_menus(cx);
        let bounds = Bounds::centered(None, size(px(1360.), px(860.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Ghostty 配置".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(18.), px(20.))),
            }),
            window_min_size: Some(size(px(1200.), px(700.))),
            app_id: Some("dev.ghostty-config".into()),
            ..Default::default()
        };
        match gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Editor::new(window, cx))) {
            Ok((_handle, _view)) =>
            {
                #[cfg(feature = "snapshot")]
                if let Ok(dir) = std::env::var("GHOSTTY_CONFIG_SNAPSHOT") {
                    snapshot::run(dir, _handle, _view, cx);
                }
            }
            Err(e) => {
                log::error!("open window: {e:#}");
                cx.quit();
            }
        }
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}

fn set_menus(cx: &mut App) {
    cx.set_menus(vec![
        Menu { name: "Ghostty 配置".into(), items: vec![MenuItem::action("退出 Ghostty 配置", Quit)], disabled: false },
        Menu {
            name: "文件".into(),
            items: vec![
                MenuItem::action("保存并重新加载", Save),
                MenuItem::action("放弃修改", Discard),
                MenuItem::separator(),
                MenuItem::action("在访达中显示配置文件", RevealConfig),
                MenuItem::separator(),
                MenuItem::action("关闭窗口", CloseWindow),
            ],
            disabled: false,
        },
        Menu {
            name: "显示".into(),
            items: vec![
                MenuItem::action("搜索设置", FocusSearch),
                MenuItem::separator(),
                MenuItem::action("显示 / 隐藏配置键", ToggleKeys),
                MenuItem::action("显示 / 隐藏说明", ToggleHelp),
            ],
            disabled: false,
        },
    ]);
}

/// Log lines to stderr: `level target: message`.
fn env_logger() {
    struct Stderr;
    impl log::Log for Stderr {
        fn enabled(&self, m: &log::Metadata) -> bool {
            m.target().starts_with("ghostty_config") && m.level() <= log::Level::Info || m.level() <= log::Level::Warn
        }
        fn log(&self, r: &log::Record) {
            if self.enabled(r.metadata()) {
                eprintln!("{} {}: {}", r.level(), r.target(), r.args());
            }
        }
        fn flush(&self) {}
    }
    let _ = log::set_boxed_logger(Box::new(Stderr)).map(|_| log::set_max_level(log::LevelFilter::Info));
}
