//! Ghostty's config file as lines. Reading pulls the editor's options out of
//! it; writing edits only the lines of options that changed, so comments,
//! ordering, keybinds and every option the editor doesn't know stay as the
//! user wrote them.

use std::collections::{BTreeMap, BTreeSet};

use crate::schema::{self, CATS, OPTS, Opt, Val};

#[derive(Clone, Debug)]
pub struct Line {
    pub text: String,
    pub key: Option<String>,
    pub value: String,
}

pub fn parse_line(text: &str) -> Line {
    let t = text.trim();
    let (key, value) = match t.split_once('=') {
        Some((k, v)) if !t.starts_with('#') && !k.trim().is_empty() => {
            let v = v.trim();
            let v = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(v);
            (Some(k.trim().to_string()), v.to_string())
        }
        _ => (None, String::new()),
    };
    Line { text: text.to_string(), key, value }
}

pub fn parse(text: &str) -> Vec<Line> {
    text.lines().map(parse_line).collect()
}

pub type Values = BTreeMap<&'static str, Val>;

/// What the file says, in the editor's terms.
#[derive(Clone, Debug, Default)]
pub struct Loaded {
    pub values: Values,
    /// Options written in a form 1.4 deprecates.
    pub legacy: BTreeSet<&'static str>,
    /// `font-family` lines after the first: fallbacks the editor keeps.
    pub fallbacks: Vec<String>,
    /// `palette = N=#rrggbb` overrides, for the preview.
    pub palette: BTreeMap<usize, schema::Rgb>,
}

/// Keys that also set an option: its deprecated names.
fn aliases(key: &str) -> &'static [&'static str] {
    match key {
        "scrollback-limit-bytes" => &["scrollback-limit-bytes", "scrollback-limit"],
        _ => &[],
    }
}

fn matches_key(o: &Opt, key: &str) -> bool {
    key == o.key || aliases(o.key).contains(&key)
}

/// `font-family`, `font-feature`: repeatable keys; an empty value clears
/// what came before.
fn list_values<'a>(lines: &'a [Line], key: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    for l in lines.iter().filter(|l| l.key.as_deref() == Some(key)) {
        if l.value.is_empty() {
            out.clear();
        } else {
            out.push(l.value.as_str());
        }
    }
    out
}

const LIG: &[&str] = &["calt", "liga", "dlig"];

/// A `font-feature` token that turns a ligature feature off.
fn is_lig_off(tok: &str) -> bool {
    let t = tok.trim();
    let name = t.trim_start_matches('-').split(['=', ' ', ':']).next().unwrap_or("");
    LIG.contains(&name) && (t.starts_with('-') || t.ends_with("=0") || t.ends_with(" off") || t.ends_with(":0"))
}

fn is_lig(tok: &str) -> bool {
    let name = tok.trim().trim_start_matches(['-', '+']).split(['=', ' ', ':']).next().unwrap_or("");
    LIG.contains(&name)
}

pub fn read(lines: &[Line]) -> Loaded {
    let mut out = Loaded::default();
    for o in OPTS {
        let v = match o.key {
            "font-family" => {
                let fams = list_values(lines, "font-family");
                out.fallbacks = fams.iter().skip(1).map(|s| s.to_string()).collect();
                fams.first().map(|f| Val::Text(f.to_string())).unwrap_or_else(|| o.default())
            }
            "palette" => Val::Bool(lines.iter().any(|l| l.key.as_deref() == Some("palette"))),
            "font-feature" => {
                let off = list_values(lines, "font-feature").iter().flat_map(|v| v.split(',')).any(is_lig_off);
                Val::Bool(!off)
            }
            _ => match lines.iter().rev().find(|l| l.key.as_deref().is_some_and(|k| matches_key(o, k))) {
                None => o.default(),
                Some(l) => {
                    if l.key.as_deref() != Some(o.key) {
                        out.legacy.insert(o.key);
                    }
                    match (o.key, l.value.as_str()) {
                        // Deprecated aliases: on macOS `true` is `clipboard`.
                        ("copy-on-select", "true") => {
                            out.legacy.insert(o.key);
                            Val::Text("clipboard".into())
                        }
                        ("copy-on-select", "false") => {
                            out.legacy.insert(o.key);
                            Val::Text("none".into())
                        }
                        ("background-blur", "0") => Val::Text("false".into()),
                        ("background-blur", "20") => Val::Text("true".into()),
                        _ => o.parse(&l.value),
                    }
                }
            },
        };
        out.values.insert(o.key, v);
    }
    for l in lines.iter().filter(|l| l.key.as_deref() == Some("palette")) {
        if let Some((i, c)) = l.value.split_once('=')
            && let (Ok(i), Some(c)) = (i.trim().parse::<usize>(), schema::Rgb::parse(c))
            && i < 16
        {
            out.palette.insert(i, c);
        }
    }
    out
}

/// How a line of the rewritten file relates to the file on disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Same,
    Changed,
    Added,
    /// Shown struck through; not written.
    Removed,
}

#[derive(Clone, Debug)]
pub struct OutLine {
    pub text: String,
    pub key: Option<String>,
    pub mark: Mark,
}

impl OutLine {
    fn live(&self) -> bool {
        self.mark != Mark::Removed
    }
}

/// The file as it would be written: the lines on disk with the changed
/// options edited in. `dirty` lists the options to rewrite.
pub fn render(lines: &[Line], dirty: &[&'static Opt], values: &Values, fallbacks: &[String]) -> Vec<OutLine> {
    let mut out: Vec<OutLine> = lines.iter().map(|l| OutLine { text: l.text.clone(), key: l.key.clone(), mark: Mark::Same }).collect();
    for o in dirty {
        let Some(v) = values.get(o.key) else { continue };
        match o.key {
            "font-family" => edit_font_family(&mut out, o, v, !fallbacks.is_empty()),
            "font-feature" => edit_font_feature(&mut out, o, v.flag().unwrap_or(true)),
            // Only ever switched off: every palette line goes.
            "palette" => {
                for i in positions(&out, |k| k == "palette").into_iter().rev() {
                    remove(&mut out, i);
                }
            }
            _ => {
                let at: Vec<usize> = positions(&out, |k| matches_key(o, k));
                if o.is_default(v) {
                    for i in at.into_iter().rev() {
                        remove(&mut out, i);
                    }
                } else {
                    let line = format!("{} = {}", o.key, o.ser(v));
                    match at.last() {
                        Some(&i) => replace(&mut out, i, line),
                        None => insert(&mut out, o, line),
                    }
                }
            }
        }
    }
    out
}

/// The text to write.
pub fn text_of(out: &[OutLine]) -> String {
    let mut s = String::new();
    for l in out.iter().filter(|l| l.live()) {
        s.push_str(&l.text);
        s.push('\n');
    }
    s
}

fn positions(out: &[OutLine], f: impl Fn(&str) -> bool) -> Vec<usize> {
    out.iter().enumerate().filter(|(_, l)| l.live() && l.key.as_deref().is_some_and(&f)).map(|(i, _)| i).collect()
}

fn replace(out: &mut [OutLine], i: usize, text: String) {
    let l = &mut out[i];
    if l.text == text {
        return;
    }
    if l.mark == Mark::Same {
        l.mark = Mark::Changed;
    }
    l.key = parse_line(&text).key;
    l.text = text;
}

fn remove(out: &mut Vec<OutLine>, i: usize) {
    if out[i].mark == Mark::Added {
        out.remove(i);
    } else {
        out[i].mark = Mark::Removed;
    }
}

fn added(text: String) -> OutLine {
    let key = parse_line(&text).key;
    OutLine { text, key, mark: Mark::Added }
}

/// A new line goes after the last line of the same category; failing that,
/// into a new section at the end of the file.
fn insert(out: &mut Vec<OutLine>, o: &Opt, text: String) {
    let near = out.iter().rposition(|l| l.live() && l.key.as_deref().and_then(schema::cat_of_key) == Some(o.cat));
    match near {
        Some(i) => out.insert(i + 1, added(text)),
        None => {
            if out.last().is_some_and(|l| !l.text.trim().is_empty()) {
                out.push(added(String::new()));
            }
            let name = CATS.iter().find(|c| c.id == o.cat).map(|c| c.name).unwrap_or(o.cat);
            out.push(added(format!("# {name}")));
            out.push(added(text));
        }
    }
}

/// The first family is the editor's; later ones are fallbacks and stay. With
/// fallbacks the primary is always written, or the first fallback would
/// take its place.
fn edit_font_family(out: &mut Vec<OutLine>, o: &Opt, v: &Val, has_fallbacks: bool) {
    let at = positions(out, |k| k == "font-family");
    // The primary: the first after the last clearing (empty) line.
    let start = at.iter().rposition(|&i| parse_line(&out[i].text).value.is_empty()).map(|p| p + 1).unwrap_or(0);
    let primary = at.get(start).copied();
    if o.is_default(v) && !has_fallbacks {
        if let Some(i) = primary {
            remove(out, i);
        }
        return;
    }
    let line = format!("font-family = {}", v.text());
    match primary {
        Some(i) => replace(out, i, line),
        None => insert(out, o, line),
    }
}

/// Ligatures off: one `-calt, -liga, -dlig` line. On: those tokens go,
/// and lines left empty with them; other features stay.
fn edit_font_feature(out: &mut Vec<OutLine>, o: &Opt, on: bool) {
    let at = positions(out, |k| k == "font-feature");
    let mut first_lig_only = None;
    for &i in at.iter().rev() {
        let value = parse_line(&out[i].text).value;
        let toks: Vec<&str> = value.split(',').map(str::trim).filter(|t| !t.is_empty()).collect();
        let keep: Vec<&str> = toks.iter().copied().filter(|t| if on { !is_lig_off(t) } else { !is_lig(t) }).collect();
        if keep.len() == toks.len() {
            continue;
        }
        if keep.is_empty() {
            first_lig_only = Some(i);
            remove(out, i);
        } else {
            replace(out, i, format!("font-feature = {}", keep.join(", ")));
        }
    }
    if on {
        return;
    }
    let line = "font-feature = -calt, -liga, -dlig".to_string();
    // Reuse the topmost ligature-only line, now struck out, for the new one.
    match first_lig_only {
        Some(i) => {
            out[i].mark = Mark::Same;
            replace(out, i, line);
        }
        None => insert(out, o, line),
    }
}

/// The display of a line: comment, blank or setting.
pub fn is_comment(text: &str) -> bool {
    text.trim_start().starts_with('#')
}

/// Options whose current value differs from the file, or whose legacy form
/// is being migrated.
pub fn dirty(loaded: &Loaded, values: &Values, legacy: &BTreeSet<&'static str>) -> Vec<&'static Opt> {
    OPTS.iter()
        .filter(|o| values.get(o.key) != loaded.values.get(o.key) || (loaded.legacy.contains(o.key) && !legacy.contains(o.key)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Rgb, opt};

    const SAMPLE: &str = "# Theme\ntheme = Cobalt2\nbackground = #122738\npalette = 1=#ff628c\n\n# Fonts\nfont-family = Menlo\nfont-family = BIZ UDGothic\nfont-size = 17\nfont-feature = calt\nfont-feature = liga\nadjust-cell-height = 12%\n\nscrollback-limit = 10000000\ncopy-on-select = true\nkeybind = super+t=new_tab\n";

    fn edit(f: impl FnOnce(&mut Values, &mut BTreeSet<&'static str>)) -> (Loaded, Vec<OutLine>) {
        let lines = parse(SAMPLE);
        let loaded = read(&lines);
        let mut values = loaded.values.clone();
        let mut legacy = loaded.legacy.clone();
        f(&mut values, &mut legacy);
        let d = dirty(&loaded, &values, &legacy);
        let out = render(&lines, &d, &values, &loaded.fallbacks);
        (loaded, out)
    }

    #[test]
    fn reads_values() {
        let l = read(&parse(SAMPLE));
        assert_eq!(l.values["theme"], Val::Text("Cobalt2".into()));
        assert_eq!(l.values["background"], Val::Color(Some(Rgb(0x122738))));
        assert_eq!(l.values["font-family"], Val::Text("Menlo".into()));
        assert_eq!(l.fallbacks, vec!["BIZ UDGothic".to_string()]);
        assert_eq!(l.values["font-feature"], Val::Bool(true));
        assert_eq!(l.values["adjust-cell-height"], Val::Num(12.));
        assert_eq!(l.values["scrollback-limit-bytes"], Val::Num(10.));
        assert_eq!(l.values["copy-on-select"], Val::Text("clipboard".into()));
        assert!(l.legacy.contains("scrollback-limit-bytes") && l.legacy.contains("copy-on-select"));
        assert_eq!(l.palette[&1], Rgb(0xff628c));
        assert_eq!(l.values["palette"], Val::Bool(true));
    }

    #[test]
    fn removes_palette_overrides() {
        let (_, out) = edit(|v, _| {
            v.insert("palette", Val::Bool(false));
        });
        assert!(!text_of(&out).contains("palette"));
    }

    #[test]
    fn untouched_file_is_identical() {
        let (_, out) = edit(|_, _| {});
        assert_eq!(text_of(&out), SAMPLE);
        assert!(out.iter().all(|l| l.mark == Mark::Same));
    }

    #[test]
    fn edits_in_place_and_removes_defaults() {
        let (_, out) = edit(|v, _| {
            v.insert("font-size", Val::Num(14.));
            v.insert("background", Val::Color(None));
        });
        let text = text_of(&out);
        assert!(text.contains("font-size = 14\n"));
        assert!(!text.contains("background ="));
        assert!(out.iter().any(|l| l.mark == Mark::Removed && l.text.starts_with("background")));
        assert!(out.iter().any(|l| l.mark == Mark::Changed && l.text == "font-size = 14"));
    }

    #[test]
    fn inserts_next_to_its_category() {
        let (_, out) = edit(|v, _| {
            v.insert("cursor-style", Val::Text("bar".into()));
            v.insert("font-thicken", Val::Bool(true));
        });
        let text = text_of(&out);
        // Fonts: after adjust-cell-height, the last font line.
        assert!(text.contains("adjust-cell-height = 12%\nfont-thicken = true\n"));
        // No cursor lines yet: a new section at the end.
        assert!(text.ends_with("keybind = super+t=new_tab\n\n# 光标\ncursor-style = bar\n"));
    }

    #[test]
    fn migrates_legacy_forms() {
        let (_, out) = edit(|_, legacy| legacy.clear());
        let text = text_of(&out);
        assert!(text.contains("scrollback-limit-bytes = 10000000\n"));
        assert!(text.contains("copy-on-select = clipboard\n"));
        assert!(!text.contains("scrollback-limit ="));
    }

    #[test]
    fn font_family_keeps_fallbacks() {
        let (_, out) = edit(|v, _| {
            v.insert("font-family", opt("font-family").unwrap().default());
        });
        let text = text_of(&out);
        assert!(text.contains("font-family = JetBrains Mono\nfont-family = BIZ UDGothic\n"));
    }

    #[test]
    fn ligatures_toggle() {
        let (_, out) = edit(|v, _| {
            v.insert("font-feature", Val::Bool(false));
        });
        let text = text_of(&out);
        assert_eq!(text.matches("font-feature").count(), 1);
        assert!(text.contains("font-feature = -calt, -liga, -dlig\n"));
        let lines = parse(&text);
        let back = read(&lines);
        assert_eq!(back.values["font-feature"], Val::Bool(false));
        let mut values = back.values.clone();
        values.insert("font-feature", Val::Bool(true));
        let d = dirty(&back, &values, &back.legacy);
        let text = text_of(&render(&lines, &d, &values, &back.fallbacks));
        assert!(!text.contains("font-feature"));
    }
}
