//! Omarchy 4 ("Quattro") theme adapter.
//!
//! Omarchy replaces `~/.local/state/omarchy/current/theme` as a directory on
//! every theme switch. Holdover watches the parent `current/` directory, then
//! reads `theme/holdover.json` (rendered from `holdover.json.tpl`) or, if that
//! file is absent, `theme/colors.toml`. A failed read keeps the last valid
//! palette, and a cold start with nothing to read uses the bundled palette.
//!
//! Colour mixing follows `omarchy-theme-set-templates` from Omarchy v4.0.4:
//! `amount` of the second colour is blended into the first, in straight RGB.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const TEMPLATE: &str = include_str!("../../../share/holdover/holdover.json.tpl");

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteSource {
    RenderedTemplate,
    ColorsToml,
    LastValid,
    Bundled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tokens {
    pub mode: String,
    pub background: String,
    pub surface: String,
    pub text: String,
    pub text_secondary: String,
    pub border: String,
    pub accent: String,
    pub focus: String,
    pub warning: String,
    pub error: String,
    pub chart: [String; 6],
    pub reticle: String,
    pub target: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub tokens: Tokens,
    pub source: PaletteSource,
    pub contrast_warnings: Vec<String>,
}

/// Paths for one Omarchy `current` directory.
#[derive(Clone, Debug)]
pub struct ThemePaths {
    pub current_dir: PathBuf,
}

impl ThemePaths {
    pub fn new(current_dir: impl Into<PathBuf>) -> Self {
        Self {
            current_dir: current_dir.into(),
        }
    }

    /// Watch this directory. Watching the theme file itself misses the rename
    /// Omarchy uses to publish a new theme.
    pub fn watch_dir(&self) -> &Path {
        &self.current_dir
    }

    pub fn theme_dir(&self) -> PathBuf {
        self.current_dir.join("theme")
    }

    pub fn next_theme_dir(&self) -> PathBuf {
        self.current_dir.join("next-theme")
    }

    pub fn holdover_json(&self) -> PathBuf {
        self.theme_dir().join("holdover.json")
    }

    pub fn colors_toml(&self) -> PathBuf {
        self.theme_dir().join("colors.toml")
    }
}

#[derive(Clone, Debug, Default)]
pub struct ThemeFollower {
    last_good: Option<Palette>,
}

impl ThemeFollower {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reload(&mut self, paths: &ThemePaths) -> Palette {
        if let Some(palette) = load_rendered(paths) {
            self.last_good = Some(palette.clone());
            return palette;
        }
        if let Some(palette) = load_colors_toml(paths) {
            self.last_good = Some(palette.clone());
            return palette;
        }
        if let Some(last) = &self.last_good {
            let mut kept = last.clone();
            kept.source = PaletteSource::LastValid;
            return kept;
        }
        let bundled = bundled_palette("dark");
        self.last_good = Some(bundled.clone());
        bundled
    }
}

pub fn holdover_template() -> &'static str {
    TEMPLATE
}

pub fn render_template(template: &str, colors: &BTreeMap<String, String>) -> String {
    let mut rendered = template.to_string();
    let mixes = mix_tokens(template);
    for token in mixes {
        if let Some(value) = eval_mix(&token, colors) {
            rendered = rendered.replace(&token, &value);
        }
    }
    let keys: Vec<String> = colors.keys().cloned().collect();
    for key in keys {
        let Some(value) = colors.get(&key) else {
            continue;
        };
        rendered = rendered.replace(
            &format!("{{{{ {key}_strip }}}}"),
            value.trim_start_matches('#'),
        );
        rendered = rendered.replace(
            &format!("{{{{ {key}_rgb }}}}"),
            &hex_to_rgb(value).unwrap_or_default(),
        );
        rendered = rendered.replace(&format!("{{{{ {key} }}}}"), value);
    }
    rendered
}

pub fn parse_colors_toml(text: &str) -> BTreeMap<String, String> {
    let mut raw = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key: String = key
            .chars()
            .filter(|c| !matches!(c, '"' | '\'' | ' '))
            .collect();
        if key.is_empty() {
            continue;
        }
        let value = value.trim();
        let value = if let Some(start) = value.find(['"', '\'']) {
            let quote = value.as_bytes()[start] as char;
            let rest = &value[start + 1..];
            rest.split(quote).next().unwrap_or("").to_string()
        } else {
            value.split('#').next().unwrap_or("").trim().to_string()
        };
        raw.insert(key, value);
    }
    resolve_colors(raw)
}

pub fn tokens_from_colors(colors: &BTreeMap<String, String>) -> Palette {
    let background = color(colors, "background", "#1a1916");
    let foreground = color(colors, "foreground", "#f3efe6");
    let accent = color(colors, "accent", &color(colors, "blue", "#d4763a"));
    let tokens = Tokens {
        mode: color(colors, "mode", "dark"),
        background: background.clone(),
        surface: mix_hex(&background, &foreground, 0.08),
        text: foreground.clone(),
        text_secondary: color(
            colors,
            "dark_foreground",
            &color(colors, "muted", &foreground),
        ),
        border: mix_hex(&background, &foreground, 0.16),
        accent: accent.clone(),
        focus: accent,
        warning: color(colors, "orange", &color(colors, "yellow", "#e0af68")),
        error: color(colors, "red", "#f7768e"),
        chart: [
            color(colors, "blue", "#7aa2f7"),
            color(colors, "green", "#9ece6a"),
            color(colors, "orange", "#eb927b"),
            color(colors, "magenta", "#ad8ee6"),
            color(colors, "cyan", "#449dab"),
            color(colors, "yellow", "#e0af68"),
        ],
        reticle: foreground.clone(),
        target: foreground,
    };
    let mut tokens = tokens;
    let backgrounds = [tokens.background.as_str(), tokens.surface.as_str()];
    let toward = tokens.text.clone();
    // Hints, warnings, and the focus ring have to stay readable on both
    // surfaces. A theme colour that fails is mixed toward the foreground.
    tokens.text_secondary = lift_contrast(&tokens.text_secondary, &backgrounds, 4.5, &toward);
    tokens.warning = lift_contrast(&tokens.warning, &backgrounds, 4.5, &toward);
    tokens.error = lift_contrast(&tokens.error, &backgrounds, 4.5, &toward);
    tokens.focus = lift_contrast(&tokens.focus, &backgrounds, 3.0, &toward);
    let mut contrast_warnings = Vec::new();
    let text_on_bg = contrast_ratio(&tokens.text, &tokens.background);
    if text_on_bg < 4.5 {
        contrast_warnings.push(format!(
            "text on background is {text_on_bg:.2}:1, below 4.5:1"
        ));
    }
    let text_on_surface = contrast_ratio(&tokens.text, &tokens.surface);
    if text_on_surface < 4.5 {
        contrast_warnings.push(format!(
            "text on surface is {text_on_surface:.2}:1, below 4.5:1"
        ));
    }
    Palette {
        tokens,
        source: PaletteSource::ColorsToml,
        contrast_warnings,
    }
}

pub fn bundled_palette(mode: &str) -> Palette {
    let colors = if mode == "light" {
        bundled_light()
    } else {
        bundled_dark()
    };
    let mut palette = tokens_from_colors(&colors);
    palette.source = PaletteSource::Bundled;
    palette
}

/// GTK text scale. `HOLDOVER_TEXT_SCALE` wins so tests and a missing desktop
/// stay deterministic. Whether WebKitGTK follows the GTK setting by itself was
/// not measured here; see docs/M0-SPIKE.md.
pub fn text_scale_factor() -> f64 {
    if let Ok(value) = std::env::var("HOLDOVER_TEXT_SCALE") {
        if let Ok(scale) = value.parse::<f64>() {
            if scale > 0.0 {
                return scale;
            }
        }
    }
    gsettings_text_scale().unwrap_or(1.0)
}

pub fn mix_hex(start: &str, end: &str, amount: f64) -> String {
    let amount = amount.clamp(0.0, 1.0);
    let (sr, sg, sb) = parse_hex(start).unwrap_or((0, 0, 0));
    let (er, eg, eb) = parse_hex(end).unwrap_or((0, 0, 0));
    let ch = |a: u8, b: u8| {
        (a as f64 * (1.0 - amount) + b as f64 * amount + 0.5).clamp(0.0, 255.0) as u8
    };
    format!("#{:02x}{:02x}{:02x}", ch(sr, er), ch(sg, eg), ch(sb, eb))
}

/// Move `color` toward `toward` until it clears `minimum` against every background.
fn lift_contrast(color: &str, backgrounds: &[&str], minimum: f64, toward: &str) -> String {
    let passes = |candidate: &str| {
        backgrounds
            .iter()
            .all(|background| contrast_ratio(candidate, background) + 1e-6 >= minimum)
    };
    if passes(color) {
        return color.to_string();
    }
    for step in 1..=20 {
        let candidate = mix_hex(color, toward, step as f64 / 20.0);
        if passes(&candidate) {
            return candidate;
        }
    }
    toward.to_string()
}

pub fn contrast_ratio(a: &str, b: &str) -> f64 {
    let l1 = relative_luminance(a);
    let l2 = relative_luminance(b);
    let (hi, lo) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}

fn load_rendered(paths: &ThemePaths) -> Option<Palette> {
    let text = fs::read_to_string(paths.holdover_json()).ok()?;
    let value: FlatJson = parse_flat_json(&text)?;
    let mut colors = BTreeMap::new();
    for (key, val) in value {
        colors.insert(key, val);
    }
    let mut palette = tokens_from_colors(&colors);
    // A rendered template already contains surface and border. Prefer those.
    if let Some(surface) = colors.get("surface") {
        palette.tokens.surface = surface.clone();
    }
    if let Some(border) = colors.get("border") {
        palette.tokens.border = border.clone();
    }
    palette.source = PaletteSource::RenderedTemplate;
    Some(palette)
}

fn load_colors_toml(paths: &ThemePaths) -> Option<Palette> {
    let text = fs::read_to_string(paths.colors_toml()).ok()?;
    let colors = parse_colors_toml(&text);
    if !colors.contains_key("background") {
        return None;
    }
    let rendered = render_template(TEMPLATE, &colors);
    let mut palette = tokens_from_colors(&colors);
    if let Some(flat) = parse_flat_json(&rendered) {
        if let Some(surface) = flat.get("surface") {
            palette.tokens.surface = surface.clone();
        }
        if let Some(border) = flat.get("border") {
            palette.tokens.border = border.clone();
        }
    }
    palette.source = PaletteSource::ColorsToml;
    Some(palette)
}

type FlatJson = BTreeMap<String, String>;

fn parse_flat_json(text: &str) -> Option<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let text = text.trim().trim_start_matches('{').trim_end_matches('}');
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, value) = part.split_once(':')?;
        let key = key.trim().trim_matches('"').to_string();
        let value = value.trim().trim_matches('"').to_string();
        if key.is_empty() {
            return None;
        }
        out.insert(key, value);
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn resolve_colors(mut colors: BTreeMap<String, String>) -> BTreeMap<String, String> {
    alias(&mut colors, "background", "bg");
    alias(&mut colors, "dark_background", "dark_bg");
    alias(&mut colors, "darker_background", "darker_bg");
    alias(&mut colors, "lighter_background", "lighter_bg");
    alias(&mut colors, "foreground", "fg");
    alias(&mut colors, "dark_foreground", "dark_fg");
    alias(&mut colors, "light_foreground", "light_fg");
    alias(&mut colors, "bright_foreground", "bright_fg");
    if !colors.contains_key("background") {
        if let Some(value) = colors.get("color0").cloned() {
            colors.insert("background".into(), value);
        }
    }
    if !colors.contains_key("foreground") {
        if let Some(value) = colors.get("color7").cloned() {
            colors.insert("foreground".into(), value);
        }
    }
    for (semantic, ansi) in [
        ("red", "color1"),
        ("green", "color2"),
        ("yellow", "color3"),
        ("blue", "color4"),
        ("magenta", "color5"),
        ("cyan", "color6"),
        ("bright_red", "color9"),
        ("bright_green", "color10"),
        ("bright_yellow", "color11"),
        ("bright_blue", "color12"),
        ("bright_magenta", "color13"),
        ("bright_cyan", "color14"),
    ] {
        alias(&mut colors, semantic, ansi);
    }
    alias(&mut colors, "magenta", "purple");
    alias(&mut colors, "bright_magenta", "bright_purple");
    fill(&mut colors, "light_foreground", &["color7", "foreground"]);
    fill(&mut colors, "bright_foreground", &["color15", "foreground"]);
    fill(&mut colors, "dark_foreground", &["color8", "foreground"]);
    fill(&mut colors, "muted", &["color8", "dark_foreground"]);
    fill(
        &mut colors,
        "selection",
        &["selection_background", "color8", "color0", "background"],
    );
    if let Some(selection) = colors.get("selection").cloned() {
        colors
            .entry("selection_background".into())
            .or_insert(selection);
    }
    if let Some(bright) = colors.get("bright_foreground").cloned() {
        colors
            .entry("selection_foreground".into())
            .or_insert(bright);
    }
    fill(&mut colors, "orange", &["yellow"]);
    if !colors.contains_key("brown") {
        if let Some(orange) = colors.get("orange").cloned() {
            colors.insert("brown".into(), mix_hex(&orange, "#000000", 0.5));
        }
    }
    if !colors.contains_key("dark_background") {
        if let Some(bg) = colors.get("background").cloned() {
            colors.insert("dark_background".into(), mix_hex(&bg, "#000000", 0.25));
        }
    }
    if !colors.contains_key("darker_background") {
        if let Some(bg) = colors.get("background").cloned() {
            colors.insert("darker_background".into(), mix_hex(&bg, "#000000", 0.5));
        }
    }
    for (bright, base) in [
        ("bright_red", "red"),
        ("bright_yellow", "yellow"),
        ("bright_green", "green"),
        ("bright_cyan", "cyan"),
        ("bright_blue", "blue"),
        ("bright_magenta", "magenta"),
    ] {
        if !colors.contains_key(bright) {
            if let Some(base_color) = colors.get(base).cloned() {
                colors.insert(bright.into(), mix_hex(&base_color, "#ffffff", 0.2));
            }
        }
    }
    alias(&mut colors, "purple", "magenta");
    alias(&mut colors, "bright_purple", "bright_magenta");
    for (ansi, semantic) in [
        ("color0", "background"),
        ("color1", "red"),
        ("color2", "green"),
        ("color3", "yellow"),
        ("color4", "blue"),
        ("color5", "magenta"),
        ("color6", "cyan"),
        ("color7", "foreground"),
        ("color8", "muted"),
        ("color9", "bright_red"),
        ("color10", "bright_green"),
        ("color11", "bright_yellow"),
        ("color12", "bright_blue"),
        ("color13", "bright_magenta"),
        ("color14", "bright_cyan"),
        ("color15", "bright_foreground"),
    ] {
        alias(&mut colors, ansi, semantic);
    }
    if !colors.contains_key("mode") {
        if let Some(kind) = colors.get("theme_type").cloned() {
            colors.insert("mode".into(), kind);
        } else if let Some(bg) = colors.get("background").cloned() {
            let (r, g, b) = parse_hex(&bg).unwrap_or((0, 0, 0));
            let lum = r as u16 + g as u16 + b as u16;
            colors.insert(
                "mode".into(),
                if lum > 382 {
                    "light".into()
                } else {
                    "dark".into()
                },
            );
        } else {
            colors.insert("mode".into(), "dark".into());
        }
    }
    if !colors.contains_key("accent") {
        if let Some(blue) = colors.get("blue").cloned() {
            colors.insert("accent".into(), blue);
        }
    }
    colors
}

fn alias(colors: &mut BTreeMap<String, String>, key: &str, fallback: &str) {
    if !colors.contains_key(key) {
        if let Some(value) = colors.get(fallback).cloned() {
            colors.insert(key.into(), value);
        }
    }
}

fn fill(colors: &mut BTreeMap<String, String>, key: &str, fallbacks: &[&str]) {
    if colors.contains_key(key) {
        return;
    }
    for fallback in fallbacks {
        if let Some(value) = colors.get(*fallback).cloned() {
            colors.insert(key.into(), value);
            return;
        }
    }
}

fn color(colors: &BTreeMap<String, String>, key: &str, fallback: &str) -> String {
    colors
        .get(key)
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

fn mix_tokens(template: &str) -> Vec<String> {
    let mut found = Vec::new();
    let bytes = template.as_bytes();
    let mut i = 0;
    while i + 6 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(end) = template[i..].find("}}") {
                let token = template[i..i + end + 2].to_string();
                if token.contains("mix") && !found.contains(&token) {
                    found.push(token);
                }
                i += end + 2;
                continue;
            }
        }
        i += 1;
    }
    found
}

fn eval_mix(token: &str, colors: &BTreeMap<String, String>) -> Option<String> {
    let inner = token
        .trim()
        .trim_start_matches("{{")
        .trim_end_matches("}}")
        .trim();
    let mut parts = inner.split_whitespace();
    let func = parts.next()?;
    let start = colors.get(parts.next()?)?;
    let end = colors.get(parts.next()?)?;
    let amount = parse_amount(parts.next()?)?;
    let mixed = mix_hex(start, end, amount);
    Some(match func {
        "mix" => mixed,
        "mix_strip" => mixed.trim_start_matches('#').to_string(),
        "mix_rgb" => hex_to_rgb(&mixed)?,
        _ => return None,
    })
}

fn parse_amount(text: &str) -> Option<f64> {
    if let Some(percent) = text.strip_suffix('%') {
        return percent.parse::<f64>().ok().map(|v| v / 100.0);
    }
    let value: f64 = text.parse().ok()?;
    Some(if value > 1.0 { value / 100.0 } else { value })
}

fn hex_to_rgb(hex: &str) -> Option<String> {
    let (r, g, b) = parse_hex(hex)?;
    Some(format!("{r},{g},{b}"))
}

fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() < 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
    ))
}

fn relative_luminance(hex: &str) -> f64 {
    let (r, g, b) = parse_hex(hex).unwrap_or((0, 0, 0));
    let lin = |c: u8| {
        let s = c as f64 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

fn bundled_dark() -> BTreeMap<String, String> {
    palette_map(
        "dark", "#1a1916", "#f3efe6", "#d4763a", "#8a8478", "#e15a4a", "#e0b15a", "#d98a55",
        "#8fbf6a", "#5aa8a8", "#7aa2d4", "#b794d4",
    )
}

fn bundled_light() -> BTreeMap<String, String> {
    palette_map(
        "light", "#f7f4ee", "#1c1915", "#9a4318", "#5e584f", "#a33b32", "#8a5a12", "#9a4318",
        "#2f6b32", "#1f6e73", "#1d4e89", "#6b3d86",
    )
}

#[allow(clippy::too_many_arguments)]
fn palette_map(
    mode: &str,
    background: &str,
    foreground: &str,
    accent: &str,
    muted: &str,
    red: &str,
    yellow: &str,
    orange: &str,
    green: &str,
    cyan: &str,
    blue: &str,
    magenta: &str,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("mode".into(), mode.into()),
        ("background".into(), background.into()),
        ("foreground".into(), foreground.into()),
        ("accent".into(), accent.into()),
        ("muted".into(), muted.into()),
        ("dark_foreground".into(), muted.into()),
        ("red".into(), red.into()),
        ("yellow".into(), yellow.into()),
        ("orange".into(), orange.into()),
        ("green".into(), green.into()),
        ("cyan".into(), cyan.into()),
        ("blue".into(), blue.into()),
        ("magenta".into(), magenta.into()),
    ])
}

fn gsettings_text_scale() -> Option<f64> {
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "text-scaling-factor"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    text.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn mix_matches_the_omarchy_v4_formula() {
        assert_eq!(mix_hex("#1a1b26", "#a9b1d6", 0.08), "#252734");
        assert_eq!(mix_hex("#1a1b26", "#a9b1d6", 0.16), "#313342");
        assert_eq!(contrast_ratio("#000000", "#ffffff"), 21.0);
    }

    #[test]
    fn template_renders_without_placeholders() {
        let text = r##"
mode = "dark"
background = "#1a1b26"
foreground = "#a9b1d6"
accent = "#7aa2f7"
muted = "#414868"
red = "#f7768e"
yellow = "#e0af68"
orange = "#eb927b"
green = "#9ece6a"
cyan = "#449dab"
blue = "#7aa2f7"
magenta = "#ad8ee6"
dark_foreground = "#565f89"
bright_foreground = "#c0caf5"
"##;
        let colors = parse_colors_toml(text);
        let rendered = render_template(TEMPLATE, &colors);
        assert!(!rendered.contains("{{"));
        assert!(rendered.contains("\"surface\": \"#252734\""));
        assert_eq!(colors.get("mode").map(String::as_str), Some("dark"));
        let palette = tokens_from_colors(&colors);
        assert!(
            palette.contrast_warnings.is_empty(),
            "{:?}",
            palette.contrast_warnings
        );
    }

    #[test]
    fn legacy_ansi_names_fill_the_semantic_palette() {
        let colors =
            parse_colors_toml("color0 = \"#111111\"\ncolor7 = \"#eeeeee\"\ncolor1 = \"#ff0000\"\n");
        assert_eq!(
            colors.get("background").map(String::as_str),
            Some("#111111")
        );
        assert_eq!(
            colors.get("foreground").map(String::as_str),
            Some("#eeeeee")
        );
        assert_eq!(colors.get("red").map(String::as_str), Some("#ff0000"));
        assert_eq!(colors.get("mode").map(String::as_str), Some("dark"));
    }

    #[test]
    fn directory_swap_is_what_a_live_theme_switch_does() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("holdover-theme-{nanos}"));
        let paths = ThemePaths::new(&root);
        assert_eq!(paths.watch_dir(), root.as_path());
        fs::create_dir_all(paths.next_theme_dir()).unwrap();
        fs::write(
            paths.next_theme_dir().join("holdover.json"),
            sample_json("#112233"),
        )
        .unwrap();
        fs::remove_dir_all(paths.theme_dir()).ok();
        fs::rename(paths.next_theme_dir(), paths.theme_dir()).unwrap();
        let mut follower = ThemeFollower::new();
        let first = follower.reload(&paths);
        assert_eq!(first.source, PaletteSource::RenderedTemplate);
        assert_eq!(first.tokens.accent, "#112233");

        fs::create_dir_all(paths.next_theme_dir()).unwrap();
        fs::write(
            paths.next_theme_dir().join("holdover.json"),
            sample_json("#abcdef"),
        )
        .unwrap();
        fs::remove_dir_all(paths.theme_dir()).unwrap();
        fs::rename(paths.next_theme_dir(), paths.theme_dir()).unwrap();
        let second = follower.reload(&paths);
        assert_eq!(second.tokens.accent, "#abcdef");

        fs::write(paths.holdover_json(), "{ not json").unwrap();
        fs::remove_file(paths.colors_toml()).ok();
        let kept = follower.reload(&paths);
        assert_eq!(kept.source, PaletteSource::LastValid);
        assert_eq!(kept.tokens.accent, "#abcdef");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn bundled_light_and_dark_keep_hints_warnings_and_focus_readable() {
        for mode in ["light", "dark"] {
            let palette = bundled_palette(mode);
            for background in [&palette.tokens.background, &palette.tokens.surface] {
                assert!(
                    contrast_ratio(&palette.tokens.text_secondary, background) >= 4.5,
                    "{mode} secondary on {background}"
                );
                assert!(
                    contrast_ratio(&palette.tokens.warning, background) >= 4.5,
                    "{mode} warning on {background}"
                );
                assert!(
                    contrast_ratio(&palette.tokens.error, background) >= 4.5,
                    "{mode} error on {background}"
                );
                assert!(
                    contrast_ratio(&palette.tokens.focus, background) >= 3.0,
                    "{mode} focus on {background}"
                );
            }
        }
    }

    #[test]
    fn a_faint_muted_colour_is_lifted_toward_the_foreground() {
        let mut colors = bundled_dark();
        colors.insert("dark_foreground".into(), "#24221e".into());
        let palette = tokens_from_colors(&colors);
        assert!(contrast_ratio(&palette.tokens.text_secondary, &palette.tokens.background) >= 4.5);
        assert_ne!(palette.tokens.text_secondary, "#24221e");
        assert!(palette.contrast_warnings.is_empty());
    }

    #[test]
    fn colors_toml_is_the_fallback_and_bundled_palettes_pass_contrast() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("holdover-theme-toml-{nanos}"));
        let paths = ThemePaths::new(&root);
        fs::create_dir_all(paths.theme_dir()).unwrap();
        fs::write(
            paths.colors_toml(),
            "mode = \"dark\"\nbackground = \"#1a1b26\"\nforeground = \"#a9b1d6\"\naccent = \"#7aa2f7\"\n",
        )
        .unwrap();
        let mut follower = ThemeFollower::new();
        let palette = follower.reload(&paths);
        assert_eq!(palette.source, PaletteSource::ColorsToml);
        assert_eq!(palette.tokens.accent, "#7aa2f7");
        assert!(palette.contrast_warnings.is_empty());
        for mode in ["dark", "light"] {
            let bundled = bundled_palette(mode);
            assert!(
                bundled.contrast_warnings.is_empty(),
                "{mode} {:?}",
                bundled.contrast_warnings
            );
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn text_scale_reads_the_override() {
        std::env::set_var("HOLDOVER_TEXT_SCALE", "1.25");
        assert!((text_scale_factor() - 1.25).abs() < 1e-12);
        std::env::remove_var("HOLDOVER_TEXT_SCALE");
    }

    fn sample_json(accent: &str) -> String {
        format!(
            "{{\n  \"mode\": \"dark\",\n  \"background\": \"#1a1916\",\n  \"foreground\": \"#f3efe6\",\n  \"accent\": \"{accent}\",\n  \"muted\": \"#8a8478\",\n  \"red\": \"#e15a4a\",\n  \"yellow\": \"#e0b15a\",\n  \"orange\": \"#d98a55\",\n  \"green\": \"#8fbf6a\",\n  \"cyan\": \"#5aa8a8\",\n  \"blue\": \"#7aa2d4\",\n  \"magenta\": \"#b794d4\",\n  \"surface\": \"#252219\",\n  \"border\": \"#312e27\"\n}}"
        )
    }
}
