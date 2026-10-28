//! The editor window: the file on disk, the values being edited, and the
//! three columns of the design — categories, settings, preview + file.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::backdrop::Backdrops;
use crate::config::{self, Line, Loaded, Values};
use crate::ghostty::{self, Install, ThemeCache};
use crate::kit::{Btn, Tag, button, tag};
use crate::schema::{self, CATS, OPTS, Opt, Rgb, Val};
use crate::theme::{MONO, PalExt as _, RADIUS_MD};

actions!(ghostty_config, [Save, Discard, FocusSearch, ClearSearch, ToggleKeys, ToggleHelp, RevealConfig, Quit, CloseWindow]);

/// How long a toast stays.
const TOAST: Duration = Duration::from_millis(3200);
const TOAST_LONG: Duration = Duration::from_millis(6000);
/// How often the file is checked for outside edits.
const POLL: Duration = Duration::from_millis(1500);

pub struct Toast {
    pub text: String,
    pub error: bool,
    seq: u64,
}

pub struct Editor {
    pub install: Install,
    pub path: PathBuf,
    /// The file as last read from or written to disk.
    pub disk_text: String,
    disk_mtime: Option<SystemTime>,
    pub lines: Vec<Line>,
    pub loaded: Loaded,
    pub values: Values,
    pub legacy: BTreeSet<&'static str>,
    /// A category id, or "new" for 1.4 新增与变化.
    pub cat: &'static str,
    pub search: Entity<InputState>,
    pub theme_search: Entity<InputState>,
    /// The theme group shown: dark or light; `None` follows the current theme.
    pub theme_dark: Option<bool>,
    /// The option under the pointer.
    pub focus: Option<&'static str>,
    pub split: bool,
    pub blink_on: bool,
    pub toast: Option<Toast>,
    toast_seq: u64,
    pub themes: Vec<String>,
    pub theme_cache: ThemeCache,
    /// Monospaced families Ghostty can find; `None` until known.
    pub fonts: Option<HashSet<String>>,
    /// Other config files Ghostty loads that hold settings.
    pub other_files: Vec<String>,
    /// The running Ghostty can be told to reload with SIGUSR2.
    reload_signal: bool,
    /// The previous file was backed up this session.
    backed_up: bool,
    pub backdrops: Backdrops,
    pub list_scroll: ScrollHandle,
    pub cfg_scroll: ScrollHandle,
    pub show_keys: bool,
    pub show_help: bool,
    /// The slider being dragged.
    pub drag: Option<&'static str>,
    pub slider_bounds: Rc<RefCell<HashMap<&'static str, Bounds<Pixels>>>>,
    pub pickers: HashMap<&'static str, Entity<ColorPickerState>>,
    /// Inputs of the free-text options.
    pub texts: HashMap<&'static str, Entity<InputState>>,
    /// The color last pushed into each picker.
    pub picker_synced: HashMap<&'static str, Option<Rgb>>,
    /// The config pane follows the focused option to its line.
    pub cfg_follow: Option<&'static str>,
    /// Snapshots set the focus themselves; the real pointer must not move it.
    pub hover_locked: bool,
    pub dark: bool,
    focus_handle: FocusHandle,
    _subs: Vec<Subscription>,
}

fn read_disk(path: &PathBuf) -> (String, Option<SystemTime>) {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    (text, mtime)
}

impl Editor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let install = ghostty::detect();
        let path = std::env::var_os("GHOSTTY_CONFIG_FILE").map(PathBuf::from).unwrap_or_else(ghostty::config_path);
        log::info!("ghostty {:?} at {:?}; config {}", install.version, install.bin, path.display());
        let themes = ghostty::theme_names(&install);
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索设置或配置键…"));
        let theme_search = cx.new(|cx| InputState::new(window, cx).placeholder(format!("在全部 {} 套主题中搜索…", themes.len())));
        let mut subs = vec![
            cx.subscribe(&search, |_, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe(&theme_search, |_, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                this.dark = is_dark(window);
                cx.notify();
            }),
        ];
        let mut pickers = HashMap::new();
        for o in OPTS.iter().filter(|o| matches!(o.kind, schema::Kind::Color)) {
            let key = o.key;
            let picker = cx.new(|cx| ColorPickerState::new(window, cx));
            subs.push(cx.subscribe(&picker, move |this, _, ev: &ColorPickerEvent, cx| {
                let ColorPickerEvent::Change(Some(c)) = ev else { return };
                let rgb = to_rgb(*c);
                this.picker_synced.insert(key, Some(rgb));
                this.set(key, Val::Color(Some(rgb)), cx);
            }));
            pickers.insert(key, picker);
        }
        let mut texts = HashMap::new();
        for o in OPTS.iter() {
            let schema::Kind::Text { placeholder } = o.kind else { continue };
            let key = o.key;
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            subs.push(cx.subscribe(&input, move |this, input, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    let text = input.read(cx).value().trim().to_string();
                    if this.value(key).text() != text {
                        this.set(key, Val::Text(text), cx);
                    }
                }
            }));
            texts.insert(key, input);
        }
        let (disk_text, disk_mtime) = read_disk(&path);
        let mut this = Self {
            install,
            path,
            disk_text: String::new(),
            disk_mtime,
            lines: Vec::new(),
            loaded: Loaded::default(),
            values: Values::new(),
            legacy: BTreeSet::new(),
            cat: std::env::var("GHOSTTY_CONFIG_CAT").ok().and_then(|c| cat_id(&c)).unwrap_or("theme"),
            search,
            theme_search,
            theme_dark: None,
            focus: None,
            split: false,
            blink_on: true,
            toast: None,
            toast_seq: 0,
            themes,
            theme_cache: ThemeCache::default(),
            fonts: None,
            other_files: Vec::new(),
            reload_signal: false,
            backed_up: false,
            backdrops: Backdrops::default(),
            list_scroll: ScrollHandle::new(),
            cfg_scroll: ScrollHandle::new(),
            show_keys: true,
            show_help: true,
            drag: None,
            slider_bounds: Rc::default(),
            pickers,
            texts,
            picker_synced: HashMap::new(),
            cfg_follow: None,
            hover_locked: false,
            dark: is_dark(window),
            focus_handle: cx.focus_handle(),
            _subs: subs,
        };
        this.split = this.cat == "split";
        this.other_files = ghostty::config_files()
            .into_iter()
            .filter(|f| *f != this.path && ghostty::settings_count(f) > 0)
            .map(|f| ghostty::tilde(&f))
            .collect();
        this.load(disk_text);
        window.focus(&this.focus_handle, cx);

        // The cursor blinks at Ghostty's 600 ms.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(600)).await;
                if this.update(cx, |e, cx| e.tick_blink(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                if this.update(cx, |e, cx| e.poll_disk(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let install = this.install.clone();
        cx.spawn(async move |this, cx| {
            let (fonts, reload_signal) = cx
                .background_executor()
                .spawn(async move { (ghostty::installed_fonts(&install), ghostty::reloads_on_sigusr2(&install)) })
                .await;
            let _ = this.update(cx, |e, cx| {
                e.fonts = fonts;
                e.reload_signal = reload_signal;
                cx.notify();
            });
        })
        .detach();
        this
    }

    fn tick_blink(&mut self, cx: &mut Context<Self>) {
        let blinks = self.value("cursor-style-blink").text() != "false";
        let next = !blinks || !self.blink_on;
        if next != self.blink_on {
            self.blink_on = next;
            cx.notify();
        }
    }

    /// Take `text` as the file on disk and reset the editor to it.
    fn load(&mut self, text: String) {
        self.lines = config::parse(&text);
        self.loaded = config::read(&self.lines);
        self.values = self.loaded.values.clone();
        self.legacy = self.loaded.legacy.clone();
        self.disk_text = text;
    }

    /// The file changed underneath: read it again, keeping unsaved edits on top.
    fn rebase(&mut self, text: String) {
        let edits: Vec<(&'static str, Val)> =
            self.dirty().iter().filter_map(|o| self.values.get(o.key).map(|v| (o.key, v.clone()))).collect();
        let migrated: Vec<&'static str> = self.loaded.legacy.difference(&self.legacy).copied().collect();
        self.load(text);
        for (k, v) in edits {
            if self.values.get(k) != Some(&v) {
                self.values.insert(k, v);
                self.legacy.remove(k);
            }
        }
        for k in migrated {
            self.legacy.remove(k);
        }
    }

    fn poll_disk(&mut self, cx: &mut Context<Self>) {
        let mtime = std::fs::metadata(&self.path).and_then(|m| m.modified()).ok();
        if mtime == self.disk_mtime {
            return;
        }
        self.disk_mtime = mtime;
        let (text, _) = read_disk(&self.path);
        if text == self.disk_text {
            return;
        }
        let had_edits = !self.dirty().is_empty();
        self.rebase(text);
        self.show_toast(
            if had_edits {
                "配置文件在外部被修改过，已重新读取，你未保存的改动仍保留"
            } else {
                "配置文件在外部被修改过，已重新读取"
            },
            false,
            cx,
        );
        cx.notify();
    }

    pub fn dirty(&self) -> Vec<&'static Opt> {
        config::dirty(&self.loaded, &self.values, &self.legacy)
    }

    pub fn set(&mut self, key: &'static str, v: Val, cx: &mut Context<Self>) {
        // A split setting is only visible with two panes.
        if schema::opt(key).is_some_and(|o| o.cat == "split") {
            self.split = true;
        }
        self.values.insert(key, v);
        self.legacy.remove(key);
        self.cfg_follow = Some(key);
        cx.notify();
    }

    pub fn value(&self, key: &str) -> Val {
        self.values.get(key).cloned().or_else(|| schema::opt(key).map(|o| o.default())).unwrap_or(Val::Raw(String::new()))
    }

    /// The file as it would be written now.
    pub fn rendered(&self) -> Vec<config::OutLine> {
        config::render(&self.lines, &self.dirty(), &self.values, &self.loaded.fallbacks)
    }

    pub fn show_toast(&mut self, text: impl Into<String>, error: bool, cx: &mut Context<Self>) {
        self.toast_seq += 1;
        let seq = self.toast_seq;
        let text = text.into();
        let wait = if error || text.chars().count() > 40 { TOAST_LONG } else { TOAST };
        self.toast = Some(Toast { text, error, seq });
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |e, cx| {
                if e.toast.as_ref().is_some_and(|t| t.seq == seq) {
                    e.toast = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub fn save(&mut self, cx: &mut Context<Self>) {
        if self.dirty().is_empty() {
            return;
        }
        // Someone else wrote the file since we read it: build on theirs.
        let (now, _) = read_disk(&self.path);
        let merged = now != self.disk_text;
        if merged {
            self.rebase(now);
        }
        let dirty = self.dirty();
        let text = config::text_of(&self.rendered());
        let stamp = (!self.backed_up && self.path.is_file()).then(ghostty::stamp);
        match ghostty::write_config(&self.install, &self.path, &text, stamp.as_deref()) {
            Ok(backup) => {
                self.backed_up |= backup.is_some();
                let (text, mtime) = read_disk(&self.path);
                self.disk_mtime = mtime;
                self.load(text);
                let running = ghostty::running_pids();
                let reloaded = if self.reload_signal && std::env::var_os("GHOSTTY_CONFIG_NO_RELOAD").is_none() {
                    ghostty::reload(&running)
                } else {
                    0
                };
                let mut msg = if reloaded > 0 {
                    "已写入配置，Ghostty 已重新加载".to_string()
                } else if !running.is_empty() {
                    "已写入配置，在 Ghostty 里按 ⌘ + Shift + , 重新加载后生效".to_string()
                } else {
                    "已写入配置，下次打开 Ghostty 时生效".to_string()
                };
                let restart: Vec<&str> = dirty.iter().filter(|o| o.tag == Some("需重启")).map(|o| o.label).collect();
                let new_win: Vec<&str> = dirty.iter().filter(|o| o.tag == Some("新窗口生效")).map(|o| o.label).collect();
                let mut notes = Vec::new();
                if reloaded > 0 && !restart.is_empty() {
                    notes.push(format!("{}需重启 Ghostty 生效", restart.join("、")));
                }
                if reloaded > 0 && !new_win.is_empty() {
                    notes.push(format!("{}在新窗口生效", new_win.join("、")));
                }
                if merged {
                    notes.push("已合并文件在外部的修改".into());
                }
                if let Some(b) = backup {
                    notes.push(format!("原文件备份为 {}", b.file_name().and_then(|n| n.to_str()).unwrap_or("")));
                }
                if !notes.is_empty() {
                    msg = format!("{msg}（{}）", notes.join("；"));
                }
                self.show_toast(msg, false, cx);
            }
            Err(e) => {
                log::error!("save: {e:#}");
                self.show_toast(format!("没有保存：{e:#}"), true, cx);
            }
        }
        cx.notify();
    }

    pub fn discard(&mut self, cx: &mut Context<Self>) {
        self.values = self.loaded.values.clone();
        self.legacy = self.loaded.legacy.clone();
        cx.notify();
    }

    pub fn query(&self, cx: &App) -> String {
        self.search.read(cx).value().trim().to_lowercase()
    }

    fn pick_cat(&mut self, id: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.cat = id;
        if id == "split" {
            self.split = true;
        }
        self.search.update(cx, |s, cx| s.set_value("", window, cx));
        self.list_scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    /// Whether `o` is shown in the current list.
    pub fn listed(&self, o: &Opt, q: &str) -> bool {
        if !q.is_empty() {
            return schema::haystack(o).contains(q);
        }
        if self.cat == "new" { schema::is_news(o) } else { o.cat == self.cat }
    }

    fn render_topbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.pal().clone();
        let n = self.dirty().len();
        let drag = |d: Div| {
            d.on_mouse_down(MouseButton::Left, |ev: &MouseDownEvent, window, _| {
                if ev.click_count == 2 {
                    window.titlebar_double_click();
                } else {
                    window.start_window_move();
                }
            })
        };
        div()
            .h(px(54.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(16.))
            .pl(px(18.))
            .pr(px(16.))
            .bg(p.surface)
            // The traffic lights are the window's own, drawn over this gap.
            .child(drag(div().w(px(192.)).h_full()))
            .child(
                drag(div().flex().items_baseline().gap(px(10.)).h_full().items_center())
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_size(px(16.)).child("Ghostty 配置"))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.n(700))
                            .child(self.install.version.clone().unwrap_or_else(|| "未检测到 Ghostty".into())),
                    ),
            )
            .child(drag(div().flex_1().h_full()))
            .child(div().w(px(260.)).child(Input::new(&self.search).text_size(px(13.)).h(px(32.)).bg(p.bg).cleanable(true)))
            .child(div().text_size(px(13.)).text_color(p.n(700)).min_w(px(76.)).flex().justify_end().child(if n > 0 {
                format!("{n} 项未保存")
            } else {
                "已保存".into()
            }))
            .child(
                button("discard", "放弃修改", Btn::Secondary, n == 0, &p)
                    .px(px(12.))
                    .py(px(6.))
                    .when(n > 0, |b| b.on_click(cx.listener(|this, _, _, cx| this.discard(cx)))),
            )
            .child(
                button("save", "保存并重新加载 ⌘S", Btn::Primary, n == 0, &p)
                    .px(px(14.))
                    .py(px(7.))
                    .when(n > 0, |b| b.on_click(cx.listener(|this, _, _, cx| this.save(cx)))),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.pal().clone();
        let q = self.query(cx);
        let item = |id: &'static str, active: bool| {
            let hover = p.ink(0.06);
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .w_full()
                .px(px(12.))
                .py(px(7.))
                .rounded(px(RADIUS_MD))
                .text_size(px(14.))
                .line_height(relative(1.35))
                .cursor_pointer()
                .when(active, |d| d.bg(p.a(100)).text_color(p.a(800)))
                .when(!active, |d| d.text_color(p.text).hover(move |s| s.bg(hover)))
        };
        let mut col = div().w(px(210.)).flex_none().flex().flex_col().gap(px(2.)).bg(p.surface).px(px(12.)).py(px(18.)).min_h_0();
        for c in CATS {
            let modified = OPTS.iter().filter(|o| o.cat == c.id && !o.is_default(&self.value(o.key))).count();
            let id = c.id;
            col = col.child(
                item(c.id, q.is_empty() && self.cat == c.id)
                    .on_click(cx.listener(move |this, _, window, cx| this.pick_cat(id, window, cx)))
                    .child(c.name)
                    .child(div().text_size(px(12.)).text_color(p.n(700)).child(if modified > 0 {
                        modified.to_string()
                    } else {
                        String::new()
                    })),
            );
        }
        let news = OPTS.iter().filter(|o| schema::is_news(o)).count();
        col.child(div().h(px(22.)))
            .child(
                item("new", q.is_empty() && self.cat == "new")
                    .on_click(cx.listener(|this, _, window, cx| this.pick_cat("new", window, cx)))
                    .child("1.4 新增与变化")
                    .child(tag(news.to_string(), Tag::Accent2, &p).px(px(7.)).py(px(1.))),
            )
            .child(div().flex_1())
            .child(
                div()
                    .px(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .text_size(px(12.))
                    .text_color(p.n(700))
                    .child(match &self.install.version {
                        Some(v) => format!("检测到 Ghostty {v}"),
                        None => "没有找到 Ghostty.app".into(),
                    })
                    .child(div().font_family(MONO).text_size(px(11.)).child(ghostty::tilde(&self.path)))
                    .when(!self.path.exists(), |d| d.child("文件尚不存在，保存时创建"))
                    .children(
                        self.other_files
                            .iter()
                            .map(|f| div().mt(px(6.)).text_color(p.s(700)).child(format!("Ghostty 也会读取 {f}，其中的设置会覆盖这里"))),
                    ),
            )
    }

    /// Keep each color picker showing the color in effect.
    fn sync_pickers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let look = self.look();
        for (key, picker) in self.pickers.clone() {
            let shown = self.value(key).color().or(match key {
                "background" => Some(look.theme_bg),
                "foreground" => Some(look.theme_fg),
                "cursor-color" => Some(look.theme_cursor),
                "cursor-text" => Some(look.theme_cursor_text),
                "selection-background" => Some(look.theme_sel_bg),
                "selection-foreground" => Some(look.theme_sel_fg),
                "split-divider-color" => Some(look.divider_default),
                _ => None,
            });
            if self.picker_synced.get(key) != Some(&shown) {
                self.picker_synced.insert(key, shown);
                if let Some(c) = shown {
                    picker.update(cx, |p, cx| p.set_value(crate::theme::rgbc(c), window, cx));
                }
            }
        }
        // Text inputs follow resets and reloads, but not while being typed in.
        for (key, input) in self.texts.clone() {
            let want = self.value(key).text().to_string();
            let (now, focused) = {
                let st = input.read(cx);
                (st.value().trim().to_string(), st.focus_handle(cx).is_focused(window))
            };
            if now != want && !focused {
                input.update(cx, |s, cx| s.set_value(want, window, cx));
            }
        }
    }
}

pub fn to_rgb(c: Hsla) -> Rgb {
    let c = Rgba::from(c);
    Rgb::from_rgb([c.r, c.g, c.b].map(|v| (v.clamp(0., 1.) * 255.).round() as u8))
}

fn is_dark(window: &Window) -> bool {
    matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark)
}

fn cat_id(name: &str) -> Option<&'static str> {
    if name == "new" {
        return Some("new");
    }
    CATS.iter().find(|c| c.id == name).map(|c| c.id)
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_pickers(window, cx);
        let p = cx.pal().clone();
        let topbar = self.render_topbar(cx).into_any_element();
        let sidebar = self.render_sidebar(cx).into_any_element();
        let list = self.render_list(window, cx).into_any_element();
        let right = self.render_right(window, cx).into_any_element();
        div()
            .id("editor")
            .key_context("Editor")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &Save, _, cx| this.save(cx)))
            .on_action(cx.listener(|this, _: &Discard, _, cx| this.discard(cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.search.update(cx, |s, cx| s.focus(window, cx));
            }))
            .on_action(cx.listener(|this, _: &ClearSearch, window, cx| {
                this.search.update(cx, |s, cx| s.set_value("", window, cx));
                window.focus(&this.focus_handle, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleKeys, _, cx| {
                this.show_keys = !this.show_keys;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleHelp, _, cx| {
                this.show_help = !this.show_help;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &RevealConfig, _, _| {
                let target = if this.path.exists() { this.path.clone() } else { this.path.parent().map(PathBuf::from).unwrap_or_default() };
                let _ = std::process::Command::new("open").arg("-R").arg(target).spawn();
            }))
            // Slider drags follow the pointer anywhere in the window.
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if let Some(key) = this.drag {
                    if ev.pressed_button == Some(MouseButton::Left) {
                        this.slide(key, ev.position.x, cx);
                    } else {
                        this.drag = None;
                    }
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.drag = None))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.text)
            .font_family(p.font.clone())
            .text_size(px(15.))
            .line_height(relative(1.55))
            .child(topbar)
            .child(div().flex_1().min_h_0().flex().child(sidebar).child(list).child(right))
            .when_some(self.toast.as_ref(), |d, t| {
                d.child(
                    div().absolute().bottom(px(26.)).left_0().right_0().flex().justify_center().child(
                        div()
                            .px(px(18.))
                            .py(px(10.))
                            .max_w(px(900.))
                            .rounded(px(RADIUS_MD))
                            .bg(if t.error { p.s(800) } else { p.text })
                            .text_color(p.bg)
                            .text_size(px(14.))
                            .line_height(relative(1.45))
                            .shadow(p.shadow_md())
                            .child(t.text.clone()),
                    ),
                )
            })
    }
}
