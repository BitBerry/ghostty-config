//! What the editor knows about Ghostty's options: the sidebar's categories
//! and, for each option, its control, default and help text. Keys, defaults
//! and value sets follow `ghostty +show-config --default --docs` of Ghostty
//! 1.4.0.

use std::fmt::Write as _;

pub struct Cat {
    pub id: &'static str,
    pub name: &'static str,
    pub intro: &'static str,
    /// Whether these options change how the preview looks.
    pub visual: bool,
}

pub const CATS: &[Cat] = &[
    Cat { id: "theme", name: "主题与颜色", intro: "先挑一套配色，再按需覆盖个别颜色。", visual: true },
    Cat { id: "font", name: "字体", intro: "字形、字号与行距。", visual: true },
    Cat { id: "cursor", name: "光标", intro: "光标的形状、颜色与闪烁。", visual: true },
    Cat { id: "window", name: "窗口", intro: "标题栏、留白与透明度。", visual: true },
    Cat { id: "split", name: "分屏", intro: "一个窗口拆成多个终端时的样子。预览已切换到分屏。", visual: true },
    Cat {
        id: "clip", name: "剪贴板与鼠标", intro: "选中、复制与粘贴时发生什么。1.4 对这里改动最大。", visual: false
    },
    Cat { id: "scroll", name: "滚动历史", intro: "每个终端保留多少历史输出。", visual: false },
    Cat { id: "kbd", name: "键盘", intro: "按键与鼠标指针。", visual: false },
    Cat { id: "shell", name: "Shell 与目录", intro: "Shell 集成、TERM，以及新终端从哪个目录开始。", visual: false },
    Cat {
        id: "sec", name: "安全", intro: "终端里运行的程序——包括 SSH 远端的程序——被允许做哪些事。", visual: false
    },
];

pub fn cat(id: &str) -> Option<&'static Cat> {
    CATS.iter().find(|c| c.id == id)
}

/// `0xRRGGBB`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb(pub u32);

impl Rgb {
    /// `#rrggbb`, `rrggbb`, `#rgb`.
    pub fn parse(s: &str) -> Option<Rgb> {
        let h = s.trim().trim_start_matches('#');
        let full = match h.len() {
            6 => h.to_string(),
            3 => h.chars().flat_map(|c| [c, c]).collect(),
            _ => return None,
        };
        u32::from_str_radix(&full, 16).ok().map(Rgb)
    }

    pub fn hex(self) -> String {
        format!("#{:06x}", self.0)
    }

    pub fn rgb(self) -> [u8; 3] {
        [(self.0 >> 16) as u8, (self.0 >> 8) as u8, self.0 as u8]
    }

    pub fn from_rgb([r, g, b]: [u8; 3]) -> Rgb {
        Rgb(((r as u32) << 16) | ((g as u32) << 8) | b as u32)
    }

    /// Perceived lightness, 0–1.
    pub fn lum(self) -> f32 {
        let [r, g, b] = self.rgb();
        (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.
    }

    /// `self` moved toward `other` by `t`.
    pub fn mix(self, other: Rgb, t: f32) -> Rgb {
        let (a, b) = (self.rgb(), other.rgb());
        Rgb::from_rgb([0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Text(String),
    Num(f64),
    Bool(bool),
    /// `None`: not set, so the theme decides.
    Color(Option<Rgb>),
    /// A value from the file that the control can't show, such as
    /// `adjust-cell-height = 2` (pixels) or `background-blur = 12`. It stays
    /// as written until the control is used.
    Raw(String),
}

impl Val {
    pub fn text(&self) -> &str {
        match self {
            Val::Text(s) | Val::Raw(s) => s,
            _ => "",
        }
    }

    pub fn num(&self) -> Option<f64> {
        if let Val::Num(v) = self { Some(*v) } else { None }
    }

    pub fn flag(&self) -> Option<bool> {
        if let Val::Bool(v) = self { Some(*v) } else { None }
    }

    pub fn color(&self) -> Option<Rgb> {
        if let Val::Color(c) = self { *c } else { None }
    }
}

pub struct Choice {
    pub v: &'static str,
    pub label: &'static str,
    pub desc: &'static str,
    /// The value itself is new in 1.4.
    pub new: bool,
}

const fn ch(v: &'static str, label: &'static str, desc: &'static str) -> Choice {
    Choice { v, label, desc, new: false }
}

const fn ch_new(v: &'static str, label: &'static str, desc: &'static str) -> Choice {
    Choice { v, label, desc, new: true }
}

pub struct Num {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// Shown beside the slider.
    pub fmt: fn(f64) -> String,
    /// Written to the file.
    pub ser: fn(f64) -> String,
    /// Read from the file; `None` keeps the value as [`Val::Raw`].
    pub de: fn(&str) -> Option<f64>,
}

impl Num {
    /// Snap to the slider's grid and clamp to its range.
    pub fn snap(&self, v: f64) -> f64 {
        let v = ((v - self.min) / self.step).round() * self.step + self.min;
        (v.clamp(self.min, self.max) * 10000.).round() / 10000.
    }
}

pub enum Kind {
    Theme,
    Color,
    Font,
    Num(Num),
    Bool {
        on: &'static str,
        off: &'static str,
    },
    Enum(&'static [Choice]),
    /// The file's `palette = N=#rrggbb` lines, as one on/off: they can only
    /// be removed here.
    Palette,
    /// A comma list of features, each on or `no-`off: (name, description).
    /// The value is the full list in this order.
    Flags(&'static [(&'static str, &'static str)]),
    /// Free text, such as a path or a font name.
    Text {
        placeholder: &'static str,
    },
}

pub enum Def {
    Text(&'static str),
    Num(f64),
    Bool(bool),
    Color,
}

pub struct Opt {
    pub key: &'static str,
    pub cat: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    pub def: Def,
    pub help: &'static str,
    /// A neutral tag such as 需重启.
    pub tag: Option<&'static str>,
    /// New in 1.4.
    pub new: bool,
    /// Changed meaning in 1.4.
    pub changed: bool,
}

impl Opt {
    pub fn default(&self) -> Val {
        match self.def {
            Def::Text(s) => Val::Text(s.into()),
            Def::Num(v) => Val::Num(v),
            Def::Bool(v) => Val::Bool(v),
            Def::Color => Val::Color(None),
        }
    }

    pub fn is_default(&self, v: &Val) -> bool {
        *v == self.default()
    }

    /// A value as written in the file. An empty value resets to the default.
    pub fn parse(&self, raw: &str) -> Val {
        let raw = raw.trim();
        if raw.is_empty() {
            return self.default();
        }
        let raw_val = || Val::Raw(raw.to_string());
        match &self.kind {
            Kind::Theme | Kind::Font => Val::Text(raw.to_string()),
            Kind::Palette => Val::Bool(true),
            Kind::Text { .. } => Val::Text(raw.to_string()),
            Kind::Flags(flags) => Val::Text(parse_flags(flags, self.default().text(), raw)),
            Kind::Color => Rgb::parse(raw).map(|c| Val::Color(Some(c))).unwrap_or_else(raw_val),
            Kind::Num(n) => (n.de)(raw).map(Val::Num).unwrap_or_else(raw_val),
            Kind::Bool { .. } => match raw {
                "true" => Val::Bool(true),
                "false" => Val::Bool(false),
                _ => raw_val(),
            },
            Kind::Enum(choices) => {
                if choices.iter().any(|c| c.v == raw) {
                    Val::Text(raw.to_string())
                } else {
                    raw_val()
                }
            }
        }
    }

    /// The value as it goes into the file.
    pub fn ser(&self, v: &Val) -> String {
        match (v, &self.kind) {
            (Val::Num(x), Kind::Num(n)) => (n.ser)(*x),
            (Val::Bool(b), _) => b.to_string(),
            (Val::Color(Some(c)), _) => c.hex(),
            (Val::Color(None), _) => String::new(),
            (Val::Text(s) | Val::Raw(s), _) => s.clone(),
            (Val::Num(x), _) => fnum(*x),
        }
    }

    /// A value in words, as in 默认：…
    pub fn describe(&self, v: &Val) -> String {
        match (v, &self.kind) {
            (Val::Raw(s), _) => s.clone(),
            (Val::Text(s), Kind::Theme) if s.is_empty() => "Ghostty 默认".into(),
            (Val::Color(None), _) => "跟随主题".into(),
            (Val::Color(Some(c)), _) => c.hex(),
            (Val::Bool(b), Kind::Palette) => if *b { "有覆盖" } else { "无覆盖" }.into(),
            (Val::Text(s), Kind::Flags(_)) => {
                let on: Vec<&str> = s.split(',').filter(|f| !f.starts_with("no-")).collect();
                if on.is_empty() { "全部关闭".into() } else { on.join("、") }
            }
            (Val::Text(s), Kind::Text { placeholder }) if s.is_empty() => placeholder.to_string(),
            (Val::Bool(b), _) => if *b { "开" } else { "关" }.into(),
            (Val::Text(s), Kind::Enum(choices)) => {
                choices.iter().find(|c| c.v == s).map(|c| c.label.to_string()).unwrap_or_else(|| s.clone())
            }
            (Val::Num(x), Kind::Num(n)) => (n.fmt)(*x),
            (Val::Text(s), _) => s.clone(),
            (Val::Num(x), _) => fnum(*x),
        }
    }
}

/// `cursor,no-sudo` → every flag, on or `no-`, in the table's order. Flags
/// not mentioned keep their default; `true` / `false` switch all.
pub fn parse_flags(flags: &[(&str, &str)], default: &str, raw: &str) -> String {
    let mut on: Vec<bool> = flags.iter().map(|(n, _)| default.split(',').any(|t| t == *n)).collect();
    for tok in raw.split(',').map(str::trim) {
        match tok {
            "true" => on.iter_mut().for_each(|b| *b = true),
            "false" => on.iter_mut().for_each(|b| *b = false),
            _ => {
                let (name, val) = tok.strip_prefix("no-").map(|n| (n, false)).unwrap_or((tok, true));
                if let Some(i) = flags.iter().position(|(n, _)| *n == name) {
                    on[i] = val;
                }
            }
        }
    }
    flags.iter().zip(on).map(|((n, _), b)| if b { n.to_string() } else { format!("no-{n}") }).collect::<Vec<_>>().join(",")
}

pub fn opt(key: &str) -> Option<&'static Opt> {
    OPTS.iter().find(|o| o.key == key)
}

/// A number without trailing zeros: 1, 0.9, 0.75.
pub fn fnum(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn de_plain(s: &str) -> Option<f64> {
    s.parse().ok()
}
fn de_percent(s: &str) -> Option<f64> {
    s.strip_suffix('%')?.trim().parse().ok()
}
fn de_mb(s: &str) -> Option<f64> {
    s.parse::<f64>().ok().map(|b| b / 1e6)
}
fn de_mib(s: &str) -> Option<f64> {
    s.parse::<f64>().ok().map(|b| b / 1_048_576.)
}
fn de_lines(s: &str) -> Option<f64> {
    if s == "unlimited" { Some(0.) } else { s.parse().ok() }
}

fn fmt_pt(v: f64) -> String {
    format!("{} pt", fnum(v))
}
fn fmt_pct(v: f64) -> String {
    format!("{}%", (v * 100.).round())
}
fn fmt_rel(v: f64) -> String {
    format!("{}{}%", if v > 0. { "+" } else { "" }, fnum(v))
}
fn fmt_mb(v: f64) -> String {
    format!("{} MB", fnum(v))
}
fn fmt_mib(v: f64) -> String {
    format!("{} MiB", fnum(v))
}
fn fmt_lines(v: f64) -> String {
    if v == 0. { "不限".into() } else { format!("{} 行", grouped(v as u64)) }
}

fn fmt_cursor_h(v: f64) -> String {
    if v == 0. { "与字同高".into() } else { fmt_rel(v) }
}
fn fmt_px_add(v: f64) -> String {
    if v == 0. { "默认".into() } else { format!("+{} px", fnum(v)) }
}
fn fmt_cols(v: f64) -> String {
    if v == 0. { "自动".into() } else { format!("{} 列", fnum(v)) }
}
fn fmt_rows(v: f64) -> String {
    if v == 0. { "自动".into() } else { format!("{} 行", fnum(v)) }
}

fn ser_plain(v: f64) -> String {
    fnum(v)
}
fn ser_percent(v: f64) -> String {
    format!("{}%", fnum(v))
}
fn ser_mb(v: f64) -> String {
    format!("{}", (v * 1e6).round() as u64)
}
fn ser_mib(v: f64) -> String {
    format!("{}", (v * 1_048_576.).round() as u64)
}
fn ser_lines(v: f64) -> String {
    if v == 0. { "unlimited".into() } else { format!("{}", v as u64) }
}

const fn num(min: f64, max: f64, step: f64, fmt: fn(f64) -> String, ser: fn(f64) -> String, de: fn(&str) -> Option<f64>) -> Kind {
    Kind::Num(Num { min, max, step, fmt, ser, de })
}

const fn o(key: &'static str, cat: &'static str, label: &'static str, kind: Kind, def: Def, help: &'static str) -> Opt {
    Opt { key, cat, label, kind, def, help, tag: None, new: false, changed: false }
}

const fn tagged(mut opt: Opt, tag: &'static str) -> Opt {
    opt.tag = Some(tag);
    opt
}

const fn new(mut opt: Opt) -> Opt {
    opt.new = true;
    opt
}

const fn changed(mut opt: Opt) -> Opt {
    opt.changed = true;
    opt
}

const NO: &str = "";

pub const OPTS: &[Opt] = &[
    // — 主题与颜色
    o(
        "theme",
        "theme",
        "主题",
        Kind::Theme,
        Def::Text(""),
        "Ghostty 内置数百套主题，这里按深色、浅色各挑了一组耐看的，其余可以在下方搜索。也可以写成 light:主题A,dark:主题B，随系统外观自动切换。",
    ),
    o("background", "theme", "背景色", Kind::Color, Def::Color, "覆盖主题自带的背景色。"),
    o("foreground", "theme", "文字颜色", Kind::Color, Def::Color, "覆盖主题的默认文字颜色。"),
    o(
        "window-theme",
        "theme",
        "窗口外观",
        Kind::Enum(&[
            ch("auto", "自动", "按终端背景的明暗决定标题栏、菜单等窗口元素是深色还是浅色。"),
            ch("system", "跟随系统", "跟随 macOS 的外观设置。"),
            ch("light", "浅色", "始终浅色。"),
            ch("dark", "深色", "始终深色。"),
        ]),
        Def::Text("auto"),
        "影响原生标题栏、标签栏和弹窗的颜色，不影响终端内容。",
    ),
    o(
        "selection-background",
        "theme",
        "选中背景色",
        Kind::Color,
        Def::Color,
        "选中文字时的底色。未设置时用主题的；主题也没有时把背景色和文字颜色对调。",
    ),
    o("selection-foreground", "theme", "选中文字颜色", Kind::Color, Def::Color, NO),
    o(
        "palette",
        "theme",
        "调色板覆盖",
        Kind::Palette,
        Def::Bool(false),
        "配置里逐条写下的 palette = N=#颜色 会盖过主题自带的 16 色调色板，换主题时这些颜色不会跟着变。恢复默认即删除这些行。",
    ),
    // — 字体
    o(
        "font-family",
        "font",
        "字体",
        Kind::Font,
        Def::Text("JetBrains Mono"),
        "Ghostty 内置 JetBrains Mono，无需安装；其他字体需要已安装在系统中。",
    ),
    o(
        "font-size",
        "font",
        "字号",
        num(8., 28., 1., fmt_pt, ser_plain, de_plain),
        Def::Num(13.),
        "运行中也可以用 ⌘+ / ⌘− 临时缩放，不会改写这里；缩放过的窗口在重新加载后仍保留手动字号，新开窗口才按这里的值。",
    ),
    o(
        "adjust-cell-height",
        "font",
        "行高",
        num(-20., 60., 5., fmt_rel, ser_percent, de_percent),
        Def::Num(0.),
        "在字体自身行高的基础上增减，正数让行距更松。",
    ),
    o(
        "adjust-cell-width",
        "font",
        "字距",
        num(-20., 40., 5., fmt_rel, ser_percent, de_percent),
        Def::Num(0.),
        "在字体自身字宽的基础上增减每个字符格的宽度，正数让字符之间更松。",
    ),
    o(
        "font-feature",
        "font",
        "编程连字",
        Kind::Bool {
            on: "-> != >= 等会合成一个字形（需字体支持，Menlo 没有连字）。",
            off: "每个字符单独显示，写入 font-feature = -calt, -liga, -dlig。",
        },
        Def::Bool(true),
        "把常见的符号组合显示成连写字形。",
    ),
    o(
        "font-thicken",
        "font",
        "加粗渲染",
        Kind::Bool { on: NO, off: NO },
        Def::Bool(false),
        "仅 macOS。笔画更厚实，在细字体或非 Retina 屏幕上更易读。",
    ),
    // — 光标
    o(
        "cursor-style",
        "cursor",
        "光标形状",
        Kind::Enum(&[
            ch("block", "方块", "实心方块，最醒目。"),
            ch("bar", "竖线", "细竖线，像文本编辑器。"),
            ch("underline", "下划线", "字符下方的一条横线。"),
            ch("block_hollow", "空心方块", "只保留方框轮廓。"),
        ]),
        Def::Text("block"),
        "程序（如 vim 的插入模式）可以临时改变它。开启 shell 集成时，提示符处会自动变成竖线，可用 shell-integration-features = no-cursor 关闭。",
    ),
    o(
        "cursor-style-blink",
        "cursor",
        "光标闪烁",
        Kind::Enum(&[
            ch("", "默认", "闪烁，并允许程序用 DEC 模式 12 开关闪烁。"),
            ch("true", "闪烁", "默认闪烁，程序仍可用 DECSCUSR 改变。"),
            ch("false", "常亮", "默认不闪烁，程序仍可用 DECSCUSR 改变。"),
        ]),
        Def::Text(""),
        NO,
    ),
    o("cursor-color", "cursor", "光标颜色", Kind::Color, Def::Color, NO),
    o(
        "cursor-text",
        "cursor",
        "光标处文字颜色",
        Kind::Color,
        Def::Color,
        "方块光标盖住的那个字符用什么颜色。未设置时用主题的；主题也没有时用背景色。",
    ),
    o("cursor-opacity", "cursor", "光标不透明度", num(0.1, 1., 0.05, fmt_pct, ser_plain, de_plain), Def::Num(1.), NO),
    // — 窗口
    o(
        "adjust-cursor-height",
        "cursor",
        "光标高度",
        num(-50., 100., 5., fmt_cursor_h, ser_percent, de_percent),
        Def::Num(0.),
        "在字体高度的基础上增减，不随行高变化。对竖线、方块和空心方块都有效。",
    ),
    o(
        "adjust-cursor-thickness",
        "cursor",
        "光标粗细",
        num(0., 12., 1., fmt_px_add, ser_plain, de_plain),
        Def::Num(0.),
        "竖线和空心方块光标的线宽，在默认粗细（约 1 px）上加。",
    ),
    o(
        "cursor-click-to-move",
        "cursor",
        "⌥ 点击移动光标",
        Kind::Bool { on: NO, off: NO },
        Def::Bool(true),
        "在提示符处按住 ⌥ 点击，光标移到点击的位置。需要 shell 集成。",
    ),
    tagged(
        o(
            "macos-titlebar-style",
            "window",
            "标题栏样式",
            Kind::Enum(&[
                ch("transparent", "透明", "标题栏与终端同色，窗口浑然一体。"),
                ch("native", "原生", "标准的 macOS 标题栏，跟随 window-theme。"),
                ch("tabs", "标签页", "标签页直接放进标题栏，更省空间。"),
                ch("hidden", "隐藏", "没有标题栏，只剩终端内容；按住 ⌥ 拖动窗口边缘来移动窗口。"),
            ]),
            Def::Text("transparent"),
            NO,
        ),
        "新窗口生效",
    ),
    tagged(
        o(
            "window-decoration",
            "window",
            "窗口装饰",
            Kind::Enum(&[ch("auto", "自动", "正常的标题栏和边框。"), ch("none", "无", "去掉标题栏和边框，标签页也一起去掉。")]),
            Def::Text("auto"),
            NO,
        ),
        "新窗口生效",
    ),
    o(
        "macos-titlebar-proxy-icon",
        "window",
        "标题栏文件夹图标",
        Kind::Enum(&[
            ch("visible", "显示", "原生标题栏里，标题前显示当前目录的文件夹图标，可以拖动。"),
            ch("hidden", "隐藏", "只显示标题文字。"),
        ]),
        Def::Text("visible"),
        "只在原生标题栏样式下可见。",
    ),
    o(
        "window-title-font-family",
        "window",
        "标题栏字体",
        Kind::Text { placeholder: "系统字体" },
        Def::Text(""),
        "标题栏和标签页标题用的字体，留空用系统字体。",
    ),
    o(
        "window-padding-x",
        "window",
        "左右内边距",
        num(0., 40., 1., fmt_pt, ser_plain, de_plain),
        Def::Num(2.),
        "文字与窗口左右边缘的距离。预览中虚线框是文字区域。",
    ),
    o(
        "window-padding-y",
        "window",
        "上下内边距",
        num(0., 40., 1., fmt_pt, ser_plain, de_plain),
        Def::Num(2.),
        "文字与窗口上下边缘的距离。",
    ),
    o(
        "window-padding-balance",
        "window",
        "内边距平衡",
        Kind::Enum(&[
            ch("false", "关闭", "窗口不是字符的整数倍时，多出的空间留在右下角。"),
            ch("true", "平衡", "多出的空间分到四周，顶部有上限。"),
            ch_new("equal", "完全均分", "四边完全相等，不限制顶部。"),
        ]),
        Def::Text("false"),
        NO,
    ),
    tagged(
        o(
            "background-opacity",
            "window",
            "背景不透明度",
            num(0.3, 1., 0.01, fmt_pct, ser_plain, de_plain),
            Def::Num(1.),
            "低于 100% 时可以透过终端看到后面的内容。macOS 上修改后需要完全退出并重新打开 Ghostty。",
        ),
        "需重启",
    ),
    o(
        "background-blur",
        "window",
        "背景模糊",
        Kind::Enum(&[
            ch("false", "关闭", "透出的内容保持清晰。"),
            ch("true", "模糊", "对后方内容做模糊处理（强度 20）。"),
            ch("macos-glass-regular", "玻璃", "macOS 26 起的液态玻璃效果。"),
            ch("macos-glass-clear", "通透玻璃", "更通透的玻璃效果。"),
        ]),
        Def::Text("false"),
        "只在背景不透明度低于 100% 时可见。",
    ),
    // — 分屏
    o(
        "window-width",
        "window",
        "新窗口列数",
        num(0., 300., 10., fmt_cols, ser_plain, de_plain),
        Def::Num(0.),
        "新窗口的初始宽度，按字符列数计。0 表示由系统决定。和行数要同时设置才生效。",
    ),
    o(
        "window-height",
        "window",
        "新窗口行数",
        num(0., 100., 5., fmt_rows, ser_plain, de_plain),
        Def::Num(0.),
        "新窗口的初始高度，按行数计。",
    ),
    o(
        "window-save-state",
        "window",
        "恢复窗口",
        Kind::Enum(&[
            ch("default", "跟随系统", "只在被强制退出、或系统设置要求时保存窗口。"),
            ch("always", "始终", "每次退出都保存，下次打开恢复窗口、标签页和分屏。"),
            ch("never", "从不", "每次都从一个新窗口开始。"),
        ]),
        Def::Text("default"),
        NO,
    ),
    o(
        "confirm-close-surface",
        "window",
        "关闭前确认",
        Kind::Enum(&[
            ch("true", "有程序运行时", "终端里有程序在运行时才确认；停在提示符时直接关闭（需要 shell 集成）。"),
            ch("always", "始终", "每次关闭都确认。"),
            ch("false", "从不", "直接关闭。"),
        ]),
        Def::Text("true"),
        NO,
    ),
    o(
        "window-colorspace",
        "window",
        "色彩空间",
        Kind::Enum(&[
            ch("srgb", "sRGB", "按 sRGB 解释配置里的颜色，和大多数主题的设计一致。"),
            ch("display-p3", "Display P3", "按 P3 解释，同样的色值在广色域屏幕上更鲜艳。"),
        ]),
        Def::Text("srgb"),
        NO,
    ),
    o(
        "window-vsync",
        "window",
        "垂直同步",
        Kind::Bool { on: NO, off: NO },
        Def::Bool(true),
        "与屏幕刷新同步绘制，避免画面撕裂。关闭能略降输入延迟，但在部分外接显示器上可能出问题。新终端生效。",
    ),
    o(
        "unfocused-split-opacity",
        "split",
        "非焦点分屏亮度",
        num(0.15, 1., 0.05, fmt_pct, ser_plain, de_plain),
        Def::Num(0.7),
        "没有焦点的分屏会变暗，一眼看出正在输入的是哪个。100% 表示不变暗。",
    ),
    o("split-divider-color", "split", "分隔线颜色", Kind::Color, Def::Color, NO),
    new(o(
        "drag-handle",
        "split",
        "拖动手柄",
        Kind::Enum(&[
            ch("auto", "自动", "鼠标移到分屏顶部时出现（预览中以淡色示意）。"),
            ch("always", "始终", "一直显示在每个分屏顶部，只有一个分屏时也显示。"),
            ch("never", "从不", "隐藏手柄，也就不能再用鼠标拖动重排分屏。"),
        ]),
        Def::Text("auto"),
        "拖动它可以重新排列分屏。",
    )),
    // — 剪贴板与鼠标
    changed(o(
        "copy-on-select",
        "clip",
        "选中即复制",
        Kind::Enum(&[
            ch("none", "不复制", "选中只是高亮，按 ⌘C 才复制。这是 macOS 上的新默认值。"),
            ch_new("primary", "选区剪贴板", "复制到 Linux 的选区剪贴板，在 macOS 上没有效果。"),
            ch("clipboard", "系统剪贴板", "选中即复制到系统剪贴板，可以直接 ⌘V。"),
            ch_new("both", "两者", "Linux 上两处都复制；macOS 上与“系统剪贴板”相同，即 1.3 中 clipboard 的含义。"),
        ]),
        Def::Text("none"),
        "1.4 起取值改为 none / primary / clipboard / both，clipboard 只复制到系统剪贴板。旧写法 true 在 macOS 上等同 clipboard，false 等同 none。",
    )),
    new(o(
        "middle-click-action",
        "clip",
        "鼠标中键",
        Kind::Enum(&[
            ch("primary-paste", "粘贴选区", "粘贴选区剪贴板，只在 Linux 上有效；macOS 上什么也不做。"),
            ch("clipboard-paste", "粘贴剪贴板", "粘贴系统剪贴板，等同 ⌘V。"),
            ch("ignore", "无操作", "中键什么也不做。"),
        ]),
        Def::Text("primary-paste"),
        NO,
    )),
    o(
        "clipboard-read",
        "clip",
        "程序读取剪贴板",
        Kind::Enum(&[
            ch("ask", "询问", "每次弹窗确认，并预览将被读取的内容。"),
            ch("allow", "允许", "不询问，直接允许。"),
            ch("deny", "拒绝", "一律拒绝。"),
        ]),
        Def::Text("ask"),
        "终端里的程序（包括 SSH 远端）想读取你的剪贴板时。",
    ),
    o(
        "clipboard-write",
        "clip",
        "程序写入剪贴板",
        Kind::Enum(&[
            ch("allow", "允许", "程序可以直接写入，例如 vim 的 \"+y。"),
            ch("ask", "询问", "每次写入前确认。"),
            ch("deny", "拒绝", "一律拒绝。"),
        ]),
        Def::Text("allow"),
        NO,
    ),
    new(o(
        "clipboard-write-limit-bytes",
        "clip",
        "单次写入上限",
        num(1., 256., 1., fmt_mib, ser_mib, de_mib),
        Def::Num(64.),
        "程序通过 Kitty 剪贴板协议（OSC 5522）单次写入的上限。超过上限的写入会失败，剪贴板保持原样；不影响 OSC 52。",
    )),
    o(
        "clipboard-trim-trailing-spaces",
        "clip",
        "复制时去掉行尾空格",
        Kind::Bool { on: NO, off: NO },
        Def::Bool(true),
        "复制到剪贴板时去掉每行末尾的空白。",
    ),
    o(
        "selection-clear-on-typing",
        "clip",
        "打字时取消选中",
        Kind::Bool { on: NO, off: NO },
        Def::Bool(true),
        "开始输入时取消当前选中。关闭后可以按 Esc 或单击取消。",
    ),
    o("selection-clear-on-copy", "clip", "复制后取消选中", Kind::Bool { on: NO, off: NO }, Def::Bool(false), NO),
    o(
        "clipboard-paste-protection",
        "clip",
        "粘贴保护",
        Kind::Bool { on: NO, off: NO },
        Def::Bool(true),
        "粘贴含换行的文本前先确认，避免命令被直接执行。1.4 起回车符也算作换行。",
    ),
    new(o(
        "link-osc8",
        "clip",
        "程序输出的超链接",
        Kind::Bool { on: NO, off: NO },
        Def::Bool(true),
        "OSC 8 超链接。关闭后不再高亮、预览、复制或打开；不影响普通网址的识别。",
    )),
    // — 滚动历史
    changed(o(
        "scrollback-limit-bytes",
        "scroll",
        "历史容量",
        num(0., 500., 10., fmt_mb, ser_mb, de_mb),
        Def::Num(50.),
        "按未压缩的大小计算。1.4 起由 scrollback-limit 改名而来，默认值从 10 MB 提高到 50 MB；旧名称暂时仍然有效。",
    )),
    new(o(
        "scrollback-limit-lines",
        "scroll",
        "历史行数上限",
        num(0., 200000., 5000., fmt_lines, ser_lines, de_lines),
        Def::Num(0.),
        "按行数限制，不含当前屏幕。与容量同时设置时，先达到的那个生效。",
    )),
    new(o(
        "scrollback-compression",
        "scroll",
        "压缩历史",
        Kind::Bool { on: NO, off: NO },
        Def::Bool(true),
        "闲置时压缩看不见的历史。以文字为主的历史通常能省下 70–90% 的物理内存，对速度没有可感知的影响。",
    )),
    // — 键盘
    o(
        "macos-option-as-alt",
        "kbd",
        "Option 作为 Alt",
        Kind::Enum(&[
            ch("", "默认", "跟随键盘布局：美式和美式国际键盘上两侧都当作 Alt，其他布局不当作 Alt。"),
            ch("false", "关闭", "⌥ 用于输入特殊字符，例如 ⌥E → é。"),
            ch("true", "两侧", "两个 ⌥ 都当作 Alt。"),
            ch("left", "仅左侧", "左 ⌥ 当 Alt，右 ⌥ 仍可输入特殊字符。"),
            ch("right", "仅右侧", "右 ⌥ 当 Alt，左 ⌥ 输入特殊字符。"),
        ]),
        Def::Text(""),
        "在 shell、vim、emacs 里使用 Alt 组合键时需要。",
    ),
    o(
        "mouse-hide-while-typing",
        "kbd",
        "输入时隐藏鼠标",
        Kind::Bool { on: NO, off: NO },
        Def::Bool(false),
        "开始打字时隐藏指针，移动鼠标后重新出现。",
    ),
    // — 安全
    o(
        "shell-integration",
        "shell",
        "Shell 集成",
        Kind::Enum(&[
            ch("detect", "自动", "按 shell 的文件名识别并注入。"),
            ch("zsh", "zsh", ""),
            ch("bash", "bash", ""),
            ch("fish", "fish", ""),
            ch("none", "关闭", "不自动注入；需要的话自己在 shell 配置里加载。"),
        ]),
        Def::Text("detect"),
        "让新标签继承目录、⌥ 点击移动光标、停在提示符时关窗不再确认。",
    ),
    o(
        "shell-integration-features",
        "shell",
        "Shell 集成功能",
        Kind::Flags(&[
            ("cursor", "提示符处把光标换成竖线"),
            ("sudo", "sudo 时保留 Ghostty 的 terminfo"),
            ("title", "用当前命令和目录设置窗口标题"),
            ("ssh-env", "SSH 时把 TERM 换成 xterm-256color 并传递相关环境变量"),
            ("ssh-terminfo", "SSH 时在远端自动安装 Ghostty 的 terminfo"),
            ("path", "把 ghostty 命令所在目录加入 PATH"),
        ]),
        Def::Text("cursor,no-sudo,title,no-ssh-env,no-ssh-terminfo,path"),
        NO,
    ),
    o(
        "term",
        "shell",
        "TERM",
        Kind::Enum(&[
            ch("xterm-ghostty", "xterm-ghostty", "Ghostty 自己的 terminfo，功能最全。"),
            ch("xterm-256color", "xterm-256color", "兼容性最好；远端机器或 zellij、tmux 等程序不认识 xterm-ghostty 时用。"),
        ]),
        Def::Text("xterm-ghostty"),
        NO,
    ),
    o(
        "working-directory",
        "shell",
        "起始目录",
        Kind::Text { placeholder: "home、inherit 或 ~/路径" },
        Def::Text(""),
        "新终端从哪个目录开始。home 是主目录，inherit 是启动 Ghostty 的进程所在目录；留空时从 Dock 或 open 打开用 home。",
    ),
    o(
        "window-inherit-working-directory",
        "shell",
        "新窗口继承目录",
        Kind::Bool { on: NO, off: NO },
        Def::Bool(true),
        "新窗口从上一个窗口的当前目录开始。",
    ),
    o("tab-inherit-working-directory", "shell", "新标签继承目录", Kind::Bool { on: NO, off: NO }, Def::Bool(true), NO),
    o("split-inherit-working-directory", "shell", "新分屏继承目录", Kind::Bool { on: NO, off: NO }, Def::Bool(true), NO),
    new(o(
        "vt-window-resize-allowed",
        "sec",
        "允许程序调整窗口大小",
        Kind::Bool { on: NO, off: NO },
        Def::Bool(false),
        "程序可通过转义序列改变窗口尺寸，远程机器上的程序也可以，因此默认关闭。在分屏、多标签页、全屏或最大化时始终忽略。",
    )),
    new(o(
        "vt-xt-checksum-report",
        "sec",
        "允许屏幕校验和查询",
        Kind::Bool { on: NO, off: NO },
        Def::Bool(false),
        "主要供终端测试套件使用。程序可以借此逐格推断屏幕内容，因此默认关闭。",
    )),
];

/// The 1.4 category: options that are new or changed meaning.
pub fn is_news(o: &Opt) -> bool {
    o.new || o.changed || matches!(o.kind, Kind::Enum(chs) if chs.iter().any(|c| c.new))
}

/// Which category an option-like key belongs to, for placing new lines next
/// to related ones. Keys this editor doesn't manage go by prefix.
pub fn cat_of_key(key: &str) -> Option<&'static str> {
    if let Some(o) = opt(key) {
        return Some(o.cat);
    }
    const PREFIXES: &[(&str, &str)] = &[
        ("scrollback-limit", "scroll"),
        ("palette", "theme"),
        ("selection-", "theme"),
        ("cursor-text", "theme"),
        ("window-theme", "theme"),
        ("minimum-contrast", "theme"),
        ("bold-color", "theme"),
        ("font-", "font"),
        ("adjust-cell", "font"),
        ("adjust-font", "font"),
        ("cursor-", "cursor"),
        ("adjust-cursor", "cursor"),
        ("window-", "window"),
        ("macos-titlebar", "window"),
        ("background-", "window"),
        ("split-", "split"),
        ("unfocused-split", "split"),
        ("clipboard-", "clip"),
        ("copy-on-select", "clip"),
        ("link-", "clip"),
        ("scrollback-", "scroll"),
        ("macos-option", "kbd"),
        ("mouse-", "kbd"),
        ("vt-", "sec"),
        ("shell-", "shell"),
        ("term", "shell"),
        ("working-directory", "shell"),
        ("window-inherit", "shell"),
        ("tab-inherit", "shell"),
        ("split-inherit", "shell"),
    ];
    PREFIXES.iter().find(|(p, _)| key.starts_with(p)).map(|(_, c)| *c)
}

/// The search text of an option: key, label and help.
pub fn haystack(o: &Opt) -> String {
    let mut s = String::new();
    let _ = write!(s, "{} {} {}", o.key, o.label, o.help);
    s.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_round_trip() {
        let size = opt("font-size").unwrap();
        assert_eq!(size.parse("14"), Val::Num(14.));
        assert_eq!(size.ser(&Val::Num(14.)), "14");
        let cell = opt("adjust-cell-height").unwrap();
        assert_eq!(cell.parse("12%"), Val::Num(12.));
        assert_eq!(cell.parse("2"), Val::Raw("2".into()));
        assert_eq!(cell.ser(&Val::Num(-5.)), "-5%");
        let sb = opt("scrollback-limit-bytes").unwrap();
        assert_eq!(sb.parse("10000000"), Val::Num(10.));
        assert_eq!(sb.ser(&Val::Num(50.)), "50000000");
        let lines = opt("scrollback-limit-lines").unwrap();
        assert_eq!(lines.parse("unlimited"), Val::Num(0.));
        assert_eq!(lines.ser(&Val::Num(0.)), "unlimited");
        let lim = opt("clipboard-write-limit-bytes").unwrap();
        assert_eq!(lim.parse("67108864"), lim.default());
        let op = opt("background-opacity").unwrap();
        assert_eq!(op.ser(&Val::Num(0.9)), "0.9");
        let Kind::Num(n) = &op.kind else { unreachable!() };
        assert_eq!(n.snap(0.8999999), 0.9);
    }

    #[test]
    fn enums_and_colors() {
        let blink = opt("cursor-style-blink").unwrap();
        assert_eq!(blink.parse(""), blink.default());
        assert_eq!(blink.parse("false"), Val::Text("false".into()));
        let blur = opt("background-blur").unwrap();
        assert_eq!(blur.parse("12"), Val::Raw("12".into()));
        let bg = opt("background").unwrap();
        assert_eq!(bg.parse("122738"), Val::Color(Some(Rgb(0x122738))));
        assert_eq!(bg.ser(&Val::Color(Some(Rgb(0x122738)))), "#122738");
        assert_eq!(opt("cursor-color").unwrap().parse("cell-foreground"), Val::Raw("cell-foreground".into()));
    }

    #[test]
    fn flags_and_text() {
        let f = opt("shell-integration-features").unwrap();
        assert_eq!(f.parse("cursor,title,sudo,path"), Val::Text("cursor,sudo,title,no-ssh-env,no-ssh-terminfo,path".into()));
        assert_eq!(f.parse("no-cursor"), Val::Text("no-cursor,no-sudo,title,no-ssh-env,no-ssh-terminfo,path".into()));
        assert_eq!(f.parse("false"), Val::Text("no-cursor,no-sudo,no-title,no-ssh-env,no-ssh-terminfo,no-path".into()));
        assert!(f.is_default(&f.parse("cursor")));
        assert_eq!(opt("working-directory").unwrap().parse("~/code"), Val::Text("~/code".into()));
        assert_eq!(opt("adjust-cursor-height").unwrap().parse("80%"), Val::Num(80.));
        assert_eq!(opt("adjust-cursor-thickness").unwrap().parse("10"), Val::Num(10.));
    }

    #[test]
    fn news_category() {
        let news: Vec<_> = OPTS.iter().filter(|o| is_news(o)).map(|o| o.key).collect();
        assert!(news.contains(&"window-padding-balance"));
        assert!(news.contains(&"drag-handle"));
        assert!(!news.contains(&"font-size"));
    }
}
