use crate::config::ShortcutId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::gui) enum CommandTarget {
    Builtin(ShortcutId),
    Plugin { plugin: String, command: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::gui) struct CommandEntry {
    pub label: String,
    pub detail: String,
    pub target: CommandTarget,
}

pub(in crate::gui) fn filter(entries: Vec<CommandEntry>, query: &str) -> Vec<CommandEntry> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return entries;
    }
    entries
        .into_iter()
        .filter(|entry| {
            entry.label.to_lowercase().contains(&needle)
                || entry.detail.to_lowercase().contains(&needle)
        })
        .collect()
}

pub(in crate::gui) fn wrap(selected: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let len = len as i32;
    let next = (selected as i32 + delta).rem_euclid(len);
    next as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(label: &str, detail: &str) -> CommandEntry {
        CommandEntry {
            label: label.to_string(),
            detail: detail.to_string(),
            target: CommandTarget::Builtin(ShortcutId::NewTab),
        }
    }

    fn labels(entries: &[CommandEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.label.as_str()).collect()
    }

    #[test]
    fn an_empty_query_keeps_everything() {
        let all = vec![entry("New Tab", ""), entry("Quit", "")];
        assert_eq!(filter(all.clone(), "").len(), 2);
        assert_eq!(filter(all, "   ").len(), 2);
    }

    #[test]
    fn matching_ignores_case() {
        let all = vec![entry("New Tab", ""), entry("Quit", "")];
        assert_eq!(labels(&filter(all, "NEW")), vec!["New Tab"]);
    }

    #[test]
    fn the_detail_is_searchable_too() {
        let all = vec![entry("Say hi", "hello plugin")];
        assert_eq!(
            labels(&filter(all, "hello")).len(),
            1,
            "typing a plugin name must find the commands it contributes"
        );
    }

    #[test]
    fn a_query_matching_nothing_yields_nothing() {
        let all = vec![entry("New Tab", ""), entry("Quit", "")];
        assert!(filter(all, "zzz").is_empty());
    }

    #[test]
    fn selection_wraps_at_both_ends() {
        assert_eq!(wrap(0, -1, 3), 2);
        assert_eq!(wrap(2, 1, 3), 0);
        assert_eq!(wrap(1, 1, 3), 2);
    }

    #[test]
    fn selection_stays_put_on_an_empty_list() {
        assert_eq!(wrap(0, 1, 0), 0);
        assert_eq!(wrap(0, -1, 0), 0);
    }
}
