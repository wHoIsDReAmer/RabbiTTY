use ab_glyph::{FontArc, FontVec};
use fontdb::{Database, Family, Query};
use std::collections::BTreeSet;
use std::sync::LazyLock;

pub struct SystemFont {
    pub family: String,
    pub monospaced: bool,
}

pub fn database() -> &'static Database {
    static DB: LazyLock<Database> = LazyLock::new(|| {
        let mut db = Database::new();
        db.load_system_fonts();

        #[cfg(target_os = "macos")]
        {
            for dir in [
                "/System/Library/Fonts",
                "/System/Library/Fonts/Supplemental",
                "/Library/Fonts",
            ] {
                db.load_fonts_dir(dir);
            }
            if let Some(home) = dirs::home_dir() {
                db.load_fonts_dir(home.join("Library/Fonts"));
            }
        }

        db
    });
    &DB
}

pub fn discover_system_terminal_fonts() -> Vec<SystemFont> {
    let db = database();
    let mut mono = BTreeSet::new();
    let mut others = BTreeSet::new();

    for face in db.faces() {
        for (family, _) in &face.families {
            let family = family.trim();
            if family.is_empty() || family.starts_with('.') || is_excluded_family(family) {
                continue;
            }

            if face.monospaced || is_monospace_name_heuristic(face) {
                mono.insert(family.to_string());
            } else if is_suitable_for_terminal(family) {
                others.insert(family.to_string());
            }
        }
    }

    // Well-known monospaced families (catches Monaco, SF Mono, etc.)
    for name in discover_well_known(db) {
        if !is_excluded_family(&name) {
            others.remove(&name);
            mono.insert(name);
        }
    }

    let mut result: Vec<SystemFont> = mono
        .into_iter()
        .map(|family| SystemFont {
            family,
            monospaced: true,
        })
        .collect();
    result.extend(others.into_iter().map(|family| SystemFont {
        family,
        monospaced: false,
    }));
    result
}

/// Filter out fonts that are not suitable for terminal text rendering.
fn is_excluded_family(family: &str) -> bool {
    let lower = family.to_ascii_lowercase();

    // Obvious non-text fonts
    if lower.contains("bitmap")
        || lower.contains("braille")
        || lower.contains("stix")
        || lower.contains("emoji")
        || lower.contains("symbol")
        || lower.contains("lastresort")
        || lower.contains("wingding")
        || lower.contains("webding")
        || lower.contains("dingbat")
        || lower.contains("ornament")
        || lower.contains("icon")
    {
        return true;
    }

    // Decorative, handwriting, script, display fonts
    if lower.contains("chancery")
        || lower.contains("script")
        || lower.contains("handwrit")
        || lower.contains("calligra")
        || lower.contains("brush")
        || lower.contains("graffiti")
        || lower.contains("tattoo")
        || lower.contains("comic")
        || lower.contains("papyrus")
        || lower.contains("chalkboard")
        || lower.contains("marker")
        || lower.contains("crayon")
        || lower.contains("sketch")
        || lower.contains("engrave")
        || lower.contains("shadow")
        || lower.contains("outline")
        || lower.contains("inline")
    {
        return true;
    }

    // macOS system/UI fonts not useful for terminals
    if lower.starts_with(".")
        || lower.starts_with("apple color")
        || lower == "apple chancery"
        || lower == "zapfino"
        || lower == "signpainter"
        || lower == "snell roundhand"
    {
        return true;
    }

    false
}

/// Check if a font family is a standard text font suitable for terminal use.
/// Rejects decorative, system script-specific, and specialty fonts.
fn is_suitable_for_terminal(family: &str) -> bool {
    let lower = family.to_ascii_lowercase();

    // Allow fonts that are clearly text-oriented
    if lower.contains("sans")
        || lower.contains("serif")
        || lower.contains("mono")
        || lower.contains("gothic")
        || lower.contains("grotesk")
        || lower.contains("grotesque")
        || lower.contains("text")
        || lower.contains("code")
        || lower.contains("courier")
        || lower.contains("consol")
        || lower.contains("helvetica")
        || lower.contains("arial")
        || lower.contains("verdana")
        || lower.contains("georgia")
        || lower.contains("roboto")
        || lower.contains("inter")
        || lower.contains("noto")
        || lower.contains("fira")
        || lower.contains("ibm plex")
        || lower.contains("source")
        || lower.contains("jetbrains")
        || lower.contains("cascadia")
        || lower.contains("geist")
        || lower.contains("iosevka")
        || lower.contains("hack")
        || lower.contains("ubuntu")
        || lower.contains("liberation")
        || lower.contains("dejavu")
        || lower.contains("맑은")
        || lower.contains("malgun")
        || lower.contains("gulim")
        || lower.contains("batang")
        || lower.contains("dotum")
        || lower.contains("myungjo")
        || lower.contains("apple sd")
        || lower.contains("산돌")
        || lower.contains("hiragino")
        || lower.contains("pingfang")
        || lower.contains("heiti")
        || lower.contains("songti")
        || lower.contains("yu gothic")
        || lower.contains("yu mincho")
        || lower.contains("meiryo")
    {
        return true;
    }

    // Well-known standalone font names
    const KNOWN_GOOD: &[&str] = &[
        "menlo",
        "monaco",
        "sf pro",
        "sf mono",
        "new york",
        "geneva",
        "lucida grande",
        "optima",
        "futura",
        "avenir",
        "gill sans",
        "franklin gothic",
        "system font",
        "tahoma",
        "trebuchet ms",
        "palatino",
        "times new roman",
        "maple mono",
        "commit mono",
        "victor mono",
        "input mono",
        "pt mono",
        "pt sans",
        "pt serif",
    ];

    for name in KNOWN_GOOD {
        if lower.contains(name) {
            return true;
        }
    }

    false
}

/// Heuristic: check if font family name suggests monospaced.
fn is_monospace_name_heuristic(face: &fontdb::FaceInfo) -> bool {
    for (family, _) in &face.families {
        let lower = family.to_ascii_lowercase();
        // Match common monospace naming patterns
        if lower.contains("mono")
            || lower.contains(" code")
            || lower.ends_with("code")
            || lower.contains("consol")
            || lower.contains("courier")
        {
            return true;
        }
    }
    false
}

/// Well-known monospaced font families to always include if present.
const WELL_KNOWN_MONOSPACED: &[&str] = &[
    // macOS
    "Monaco",
    "Menlo",
    "SF Mono",
    "Courier",
    "Courier New",
    "Andale Mono",
    // Cross-platform popular
    "JetBrains Mono",
    "Fira Code",
    "Fira Mono",
    "Source Code Pro",
    "Hack",
    "Inconsolata",
    "Cascadia Code",
    "Cascadia Mono",
    "IBM Plex Mono",
    "Ubuntu Mono",
    "Roboto Mono",
    "Noto Sans Mono",
    "Input Mono",
    "Iosevka",
    "Victor Mono",
    "Geist Mono",
    "Commit Mono",
    "Maple Mono",
    // Windows
    "Consolas",
    "Lucida Console",
    // Linux
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Droid Sans Mono",
];

/// Check well-known monospaced families against the fontdb database.
fn discover_well_known(db: &Database) -> Vec<String> {
    let mut found = Vec::new();
    for &family_name in WELL_KNOWN_MONOSPACED {
        let families = [Family::Name(family_name)];
        let query = Query {
            families: &families,
            ..Query::default()
        };
        if db.query(&query).is_some() {
            found.push(family_name.to_string());
        }
    }
    found
}

pub fn load_system_font_by_family(family: &str) -> Option<FontArc> {
    let family = family.trim();
    if family.is_empty() {
        return None;
    }

    let db = database();
    let families = [Family::Name(family)];
    let query = Query {
        families: &families,
        ..Query::default()
    };

    let id = db.query(&query)?;
    db.with_face_data(id, |data, index| {
        FontVec::try_from_vec_and_index(data.to_vec(), index)
            .ok()
            .map(FontArc::new)
    })?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_discovery_finds_fonts() {
        let fonts = discover_system_terminal_fonts();
        let mono: Vec<_> = fonts.iter().filter(|f| f.monospaced).collect();
        let others: Vec<_> = fonts.iter().filter(|f| !f.monospaced).collect();
        eprintln!("Monospaced ({}):", mono.len());
        for f in &mono {
            eprintln!("  - {}", f.family);
        }
        eprintln!("Other ({}):", others.len());
        for f in others.iter().take(10) {
            eprintln!("  - {}", f.family);
        }
        if others.len() > 10 {
            eprintln!("  ... and {} more", others.len() - 10);
        }
        assert!(!fonts.is_empty(), "Should find at least one font");
        assert!(!mono.is_empty(), "Should find at least one monospaced font");
    }
}
