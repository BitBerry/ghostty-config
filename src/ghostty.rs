//! The installed Ghostty: where it and its config live, its themes and
//! fonts, and asking it to check and reload a config.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, Result, bail};

use crate::schema::Rgb;

pub const BUNDLE_ID: &str = "com.mitchellh.ghostty";

#[derive(Clone, Debug, Default)]
pub struct Install {
    pub bin: Option<PathBuf>,
    pub version: Option<String>,
    /// `Contents/Resources/ghostty` of the app bundle: themes live here.
    pub resources: Option<PathBuf>,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

fn xdg_config() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(".config"))
}

pub fn detect() -> Install {
    let has_bin = |a: &PathBuf| a.join("Contents/MacOS/ghostty").is_file();
    let app =
        [PathBuf::from("/Applications/Ghostty.app"), home().join("Applications/Ghostty.app")].into_iter().find(has_bin).or_else(|| {
            // Wherever Spotlight knows it from.
            let out = Command::new("mdfind").arg(format!("kMDItemCFBundleIdentifier == '{BUNDLE_ID}'")).output().ok()?;
            String::from_utf8_lossy(&out.stdout).lines().map(PathBuf::from).find(has_bin)
        });
    let bin = app.as_ref().map(|a| a.join("Contents/MacOS/ghostty")).or_else(|| which("ghostty"));
    let resources = app.map(|a| a.join("Contents/Resources/ghostty")).filter(|p| p.is_dir());
    let version = bin.as_ref().and_then(|b| {
        let out = Command::new(b).arg("+version").output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        // "Ghostty 1.4.0"
        text.lines().next()?.strip_prefix("Ghostty ").map(|v| v.trim().to_string())
    });
    Install { bin, version, resources }
}

fn which(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?.to_str()?.split(':').map(|d| Path::new(d).join(name)).find(|p| p.is_file())
}

/// The config files Ghostty loads, in order: XDG first, then (macOS) the
/// Application Support location, whose settings win. `config.ghostty` is the
/// 1.2+ name, `config` the older one. Ghostty reads every one that exists.
pub fn config_files() -> Vec<PathBuf> {
    let support = home().join("Library/Application Support").join(BUNDLE_ID);
    let xdg = xdg_config().join("ghostty");
    [xdg.join("config"), xdg.join("config.ghostty"), support.join("config"), support.join("config.ghostty")]
        .into_iter()
        .filter(|p| p.is_file())
        .collect()
}

/// Settings lines in a config file.
pub fn settings_count(p: &Path) -> usize {
    std::fs::read_to_string(p).map(|t| crate::config::parse(&t).iter().filter(|l| l.key.is_some()).count()).unwrap_or(0)
}

/// The file to edit: the one holding the settings (an empty file Ghostty
/// also reads is not it). With none present, a new file goes to the XDG
/// location.
pub fn config_path() -> PathBuf {
    let files = config_files();
    files.iter().max_by_key(|p| settings_count(p)).cloned().unwrap_or_else(|| xdg_config().join("ghostty/config.ghostty"))
}

/// `~/.config/ghostty/config` for display.
pub fn tilde(p: &Path) -> String {
    let h = home();
    match p.strip_prefix(&h) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThemeColors {
    pub bg: Rgb,
    pub fg: Rgb,
    pub cursor: Option<Rgb>,
    pub palette: [Rgb; 16],
}

/// Ghostty's colors with no theme set.
pub fn default_colors() -> ThemeColors {
    const P: [u32; 16] = [
        0x1d1f21, 0xcc6666, 0xb5bd68, 0xf0c674, 0x81a2be, 0xb294bb, 0x8abeb7, 0xc5c8c6, 0x666666, 0xd54e53, 0xb9ca4a, 0xe7c547, 0x7aa6da,
        0xc397d8, 0x70c0b1, 0xeaeaea,
    ];
    ThemeColors { bg: Rgb(0x282c34), fg: Rgb(0xffffff), cursor: None, palette: P.map(Rgb) }
}

fn user_themes() -> PathBuf {
    xdg_config().join("ghostty/themes")
}

/// Every theme name: the built-in ones and the user's, sorted, deduplicated.
pub fn theme_names(install: &Install) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for dir in [install.resources.as_ref().map(|r| r.join("themes")), Some(user_themes())].into_iter().flatten() {
        if let Ok(rd) = std::fs::read_dir(dir) {
            names.extend(
                rd.flatten()
                    .filter(|e| e.path().is_file())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .filter(|n| !n.starts_with('.')),
            );
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    names
}

/// A theme's colors. `name` is a theme name or an absolute path; the
/// user's themes directory shadows the built-in one, as in Ghostty.
pub fn load_theme(install: &Install, name: &str) -> Option<ThemeColors> {
    let name = name.trim();
    let path = if name.starts_with('/') || name.starts_with('~') {
        PathBuf::from(name.replacen('~', &home().to_string_lossy(), 1))
    } else {
        let user = user_themes().join(name);
        if user.is_file() { user } else { install.resources.as_ref()?.join("themes").join(name) }
    };
    let text = std::fs::read_to_string(path).ok()?;
    let mut t = default_colors();
    for l in crate::config::parse(&text) {
        match l.key.as_deref() {
            Some("background") => t.bg = Rgb::parse(&l.value).unwrap_or(t.bg),
            Some("foreground") => t.fg = Rgb::parse(&l.value).unwrap_or(t.fg),
            Some("cursor-color") => t.cursor = Rgb::parse(&l.value).or(t.cursor),
            Some("palette") => {
                if let Some((i, c)) = l.value.split_once('=')
                    && let (Ok(i), Some(c)) = (i.trim().parse::<usize>(), Rgb::parse(c))
                    && i < 16
                {
                    t.palette[i] = c;
                }
            }
            _ => {}
        }
    }
    Some(t)
}

/// `light:A,dark:B` → the one for the current appearance; a plain name as is.
pub fn pick_theme(value: &str, dark: bool) -> String {
    if !value.contains(':') || value.starts_with('/') {
        return value.to_string();
    }
    let mut pick = None;
    for part in value.split(',') {
        match part.trim().split_once(':') {
            Some(("light", n)) if !dark => pick = Some(n.trim()),
            Some(("dark", n)) if dark => pick = Some(n.trim()),
            Some((_, n)) if pick.is_none() => pick = Some(n.trim()),
            _ => {}
        }
    }
    pick.unwrap_or(value).to_string()
}

/// The monospaced families Ghostty can find on this system.
pub fn installed_fonts(install: &Install) -> Option<HashSet<String>> {
    let out = Command::new(install.bin.as_ref()?).arg("+list-fonts").output().ok()?;
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with(' '))
            .map(|l| l.trim().to_string())
            .collect(),
    )
}

/// Check a config file with `ghostty +validate-config`. The errors come back
/// as `path:line:key: message`; the path is stripped.
pub fn validate(install: &Install, path: &Path) -> Result<(), Vec<String>> {
    let Some(bin) = &install.bin else { return Ok(()) };
    let Ok(out) = Command::new(bin).arg("+validate-config").arg(format!("--config-file={}", path.display())).output() else {
        return Ok(());
    };
    if out.status.success() {
        return Ok(());
    }
    let prefix = format!("{}:", path.display());
    let errs: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .chain(String::from_utf8_lossy(&out.stderr).lines())
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.strip_prefix(&prefix).unwrap_or(l).to_string())
        .collect();
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

/// Write `text` to the config at `path`: validated first through a sibling
/// temp file (so relative `config-file` includes resolve the same), then
/// moved into place. A symlinked config (dotfiles) is written through to
/// its target. `backup` keeps the previous file next to it.
pub fn write_config(install: &Install, path: &Path, text: &str, backup: Option<&str>) -> Result<Option<PathBuf>> {
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir = target.parent().context("config path has no directory")?;
    std::fs::create_dir_all(dir).with_context(|| format!("创建 {}", dir.display()))?;
    let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("config");
    let tmp = dir.join(format!(".{name}.ghostty-config.tmp"));
    std::fs::write(&tmp, text).with_context(|| format!("写入 {}", tmp.display()))?;
    if let Err(errs) = validate(install, &tmp) {
        let _ = std::fs::remove_file(&tmp);
        bail!("Ghostty 拒绝了这份配置：{}", errs.join("；"));
    }
    let mut saved = None;
    if let Some(stamp) = backup
        && target.is_file()
    {
        let b = dir.join(format!("{name}.bak-{stamp}"));
        std::fs::copy(&target, &b).with_context(|| format!("备份到 {}", b.display()))?;
        saved = Some(b);
    }
    // Keep the old file's permissions.
    if let Ok(meta) = std::fs::metadata(&target) {
        let _ = std::fs::set_permissions(&tmp, meta.permissions());
    }
    std::fs::rename(&tmp, &target).with_context(|| format!("替换 {}", target.display()))?;
    Ok(saved)
}

/// Pids of the running Ghostty apps.
#[cfg(target_os = "macos")]
pub fn running_pids() -> Vec<i32> {
    use objc2_app_kit::NSRunningApplication;
    use objc2_foundation::NSString;
    let id = NSString::from_str(BUNDLE_ID);
    let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(&id);
    apps.iter().map(|a| a.processIdentifier()).filter(|&p| p > 0).collect()
}

#[cfg(not(target_os = "macos"))]
pub fn running_pids() -> Vec<i32> {
    Vec::new()
}

/// Ask these Ghostty processes to reload their config with SIGUSR2; only
/// for a Ghostty that [`reloads_on_sigusr2`]. Returns how many were asked.
pub fn reload(pids: &[i32]) -> usize {
    let mut n = 0;
    for &pid in pids {
        // SAFETY: plain kill(2) on a pid that belongs to a Ghostty app.
        if unsafe { libc::kill(pid, libc::SIGUSR2) } == 0 {
            n += 1;
        }
    }
    n
}

/// Whether this Ghostty reloads on SIGUSR2 rather than dying of it: the
/// macOS app logs "reloading configuration in response to SIGUSR2" from its
/// handler, so the binary carries that text (1.4 does; 1.2's handler was
/// GTK-only).
pub fn reloads_on_sigusr2(install: &Install) -> bool {
    const NEEDLE: &[u8] = b"in response to SIGUSR2";
    let Some(bin) = &install.bin else { return false };
    let Ok(bytes) = std::fs::read(bin) else { return false };
    bytes.windows(NEEDLE.len()).any(|w| w == NEEDLE)
}

/// The theme colors for every name asked so far.
#[derive(Default)]
pub struct ThemeCache(BTreeMap<String, Option<ThemeColors>>);

impl ThemeCache {
    pub fn get(&mut self, install: &Install, name: &str) -> Option<ThemeColors> {
        if name.is_empty() {
            return Some(default_colors());
        }
        self.0.entry(name.to_string()).or_insert_with(|| load_theme(install, name)).clone()
    }
}

/// `20261029-013012` from the local clock.
pub fn stamp() -> String {
    // SAFETY: localtime_r into a zeroed tm.
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        format!("{:04}{:02}{:02}-{:02}{:02}{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_light_or_dark() {
        assert_eq!(pick_theme("light:Rose Pine Dawn,dark:Rose Pine", true), "Rose Pine");
        assert_eq!(pick_theme("light:Rose Pine Dawn,dark:Rose Pine", false), "Rose Pine Dawn");
        assert_eq!(pick_theme("Nord", true), "Nord");
    }
}
