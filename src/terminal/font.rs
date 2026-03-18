use ab_glyph::{FontArc, FontVec};
use fontdb::{Database, Family, Query};
use std::collections::BTreeSet;

pub fn discover_system_terminal_fonts() -> Vec<String> {
    let mut db = Database::new();
    db.load_system_fonts();

    let mut monospaced = BTreeSet::new();

    for face in db.faces() {
        if !face.monospaced {
            continue;
        }
        for (family, _) in &face.families {
            let family = family.trim();
            if !family.is_empty() {
                monospaced.insert(family.to_string());
            }
        }
    }

    monospaced.into_iter().collect()
}

/// Load a system font suitable for CJK/wide character fallback.
pub fn load_cjk_fallback_font() -> Option<FontArc> {
    let mut db = Database::new();
    db.load_system_fonts();

    const CJK_FAMILIES: &[&str] = &[
        "Apple SD Gothic Neo",
        "Hiragino Sans",
        "PingFang SC",
        "Noto Sans CJK KR",
        "Noto Sans CJK JP",
        "Noto Sans CJK SC",
        "Microsoft YaHei",
        "Malgun Gothic",
        "Yu Gothic",
        "Noto Sans Mono CJK KR",
        "Noto Sans Mono CJK JP",
        "Noto Sans Mono CJK SC",
    ];

    for family_name in CJK_FAMILIES {
        let families = [Family::Name(family_name)];
        let query = Query {
            families: &families,
            ..Query::default()
        };
        if let Some(id) = db.query(&query) {
            let result = db.with_face_data(id, |data, index| {
                FontVec::try_from_vec_and_index(data.to_vec(), index)
                    .ok()
                    .map(FontArc::new)
            });
            if let Some(Some(font)) = result {
                return Some(font);
            }
        }
    }

    None
}

pub fn load_system_font_by_family(family: &str) -> Option<FontArc> {
    let family = family.trim();
    if family.is_empty() {
        return None;
    }

    let mut db = Database::new();
    db.load_system_fonts();

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
