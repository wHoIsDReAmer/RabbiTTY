use std::collections::HashMap;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=locales/");

    let locales_dir = Path::new("locales");
    let mut locales: Vec<(String, String, HashMap<String, String>)> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(locales_dir) {
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("toml"))
            .collect();
        paths.sort_by_key(|e| e.path());

        for entry in paths {
            let path = entry.path();
            let locale = path.file_stem().unwrap().to_str().unwrap().to_string();
            let src = std::fs::read_to_string(&path).unwrap();
            let mut map = parse_toml(&src);
            let native_label = map.remove("meta.name").unwrap_or_else(|| locale.clone());
            map.retain(|k, _| !k.starts_with("meta."));
            locales.push((locale, native_label, map));
        }
    }

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("translations.rs");

    let mut code = String::new();

    code.push_str("fn get_translation(locale: &str, key: &str) -> Option<&'static str> {\n");
    code.push_str("    match locale {\n");

    for (locale, _label, map) in &locales {
        if locale == "en" {
            continue;
        }
        code.push_str(&format!("        {:?} => match key {{\n", locale));
        let mut keys: Vec<_> = map.keys().collect();
        keys.sort();
        for k in keys {
            code.push_str(&format!("            {:?} => Some({:?}),\n", k, map[k]));
        }
        code.push_str("            _ => None,\n");
        code.push_str("        },\n");
    }

    code.push_str("        _ => match key {\n");
    if let Some((_, _, en_map)) = locales.iter().find(|(l, _, _)| l == "en") {
        let mut keys: Vec<_> = en_map.keys().collect();
        keys.sort();
        for k in keys {
            code.push_str(&format!("            {:?} => Some({:?}),\n", k, en_map[k]));
        }
    }
    code.push_str("            _ => None,\n");
    code.push_str("        },\n");
    code.push_str("    }\n");
    code.push_str("}\n");

    code.push_str("\npub const AVAILABLE_LOCALES: &[LocaleInfo] = &[\n");
    for (locale, label, _map) in &locales {
        code.push_str(&format!(
            "    LocaleInfo {{ tag: {:?}, native_label: {:?} }},\n",
            locale, label
        ));
    }
    code.push_str("];\n");

    std::fs::write(out_path, code).unwrap();
}

fn parse_toml(src: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut section = String::new();

    for line in src.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
        } else if let Some(eq) = line.find('=') {
            let key = line[..eq].trim();
            let val = line[eq + 1..].trim();
            if val.starts_with('"') && val.ends_with('"') {
                let val = val[1..val.len() - 1].to_string();
                let full_key = if section.is_empty() {
                    key.to_string()
                } else {
                    format!("{}.{}", section, key)
                };
                map.insert(full_key, val);
            }
        }
    }
    map
}
