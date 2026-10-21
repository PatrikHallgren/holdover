use std::collections::BTreeMap;

pub fn english_strings() -> BTreeMap<String, String> {
    let raw = include_str!("../../../locales/en.ftl");
    let mut map = BTreeMap::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    #[test]
    fn english_copy_keeps_the_open_limits_and_the_licences() {
        let text = include_str!("../../../locales/en.ftl");
        for key in [
            "about-limits",
            "about-mit",
            "about-crates",
            "about-drag",
            "about-personal",
            "log-boundary",
            "jump-note",
        ] {
            assert!(text.contains(&format!("{key} =")), "{key}");
        }
        let limits = text
            .lines()
            .find(|line| line.starts_with("about-limits ="))
            .unwrap();
        for marker in ["T2", "T3", "T4", "T5", "T6", "IPSC", "not applied"] {
            assert!(limits.contains(marker), "{marker}");
        }
        assert!(text.contains("rusqlite 0.32.1"));
        assert!(text.contains("zip 2.6.1"));
        assert!(text.contains("CC BY 4.0"));
        assert!(text.contains("py-ballisticcalc 3.1.1"));
    }
}
