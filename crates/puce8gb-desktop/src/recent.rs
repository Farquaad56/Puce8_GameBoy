//! Recent ROM list: most recent first, no duplicates (E13_02, decision A_10).
//! Pure data, no egui type, no file access.

/// Maximum number of remembered ROMs.
#[allow(dead_code)] // used by E13_03
pub const MAX_RECENT: usize = 10;

/// Paths of the last opened ROMs, newest first.
#[allow(dead_code)] // used by E13_03
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecentList {
    items: Vec<String>,
}

#[allow(dead_code)] // used by E13_03
impl RecentList {
    /// Puts `path` first, removing an equal entry; an empty or blank path is ignored.
    pub fn push(&mut self, path: &str) {
        let path = path.trim();
        if path.is_empty() {
            return;
        }
        self.items.retain(|item| item != path);
        self.items.insert(0, path.to_string());
        self.items.truncate(MAX_RECENT);
    }

    /// Removes the entry equal to `path`, if any.
    pub fn remove(&mut self, path: &str) {
        let path = path.trim();
        self.items.retain(|item| item != path);
    }

    /// Forgets every entry.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Entries, newest first.
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// One entry per line, newest first.
    pub fn to_lines(&self) -> Vec<String> {
        self.items.clone()
    }

    /// Rebuilds a list from lines: blank lines skipped, duplicates removed keeping the first,
    /// at most MAX_RECENT entries.
    pub fn from_lines(lines: &[String]) -> RecentList {
        let mut items: Vec<String> = Vec::new();
        for line in lines {
            let line = line.trim();
            if line.is_empty() || items.iter().any(|item| item == line) {
                continue;
            }
            if items.len() >= MAX_RECENT {
                break;
            }
            items.push(line.to_string());
        }
        RecentList { items }
    }
}

/// File name of a path: the text after the last '/' or '\\' (the whole text when there is none).
#[allow(dead_code)] // used by E13_03
pub fn recent_label(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

/// Menu entries for the "Open recent" submenu: (label, full path) pairs, newest first (E13_05).
pub fn menu_entries(list: &RecentList) -> Vec<(String, String)> {
    list.items()
        .iter()
        .map(|path| (recent_label(path), path.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn e13_02_push_moves_an_existing_entry_to_the_front() {
        let mut list = RecentList::default();
        for path in ["a", "b", "c", "a"] {
            list.push(path);
        }
        assert_eq!(list.items(), strings(&["a", "c", "b"]).as_slice());
    }

    #[test]
    fn e13_02_twelve_pushes_keep_ten_newest_first() {
        let mut list = RecentList::default();
        for n in 0..12 {
            list.push(&format!("p{n}"));
        }
        assert_eq!(list.items().len(), MAX_RECENT);
        assert_eq!(list.items()[0], "p11");
        assert_eq!(list.items()[9], "p2");
    }

    #[test]
    fn e13_02_blank_push_is_ignored() {
        let mut list = RecentList::default();
        list.push("a");
        list.push("  ");
        list.push("");
        assert_eq!(list.items(), strings(&["a"]).as_slice());
    }

    #[test]
    fn e13_02_remove_and_clear() {
        let mut list = RecentList::default();
        for path in ["a", "b", "c"] {
            list.push(path);
        }
        list.remove("b");
        assert_eq!(list.items(), strings(&["c", "a"]).as_slice());
        list.remove("missing");
        assert_eq!(list.items().len(), 2);
        list.clear();
        assert!(list.items().is_empty());
    }

    #[test]
    fn e13_02_from_lines_skips_blank_lines_and_duplicates() {
        let lines = strings(&["x", "", "y", "x"]);
        assert_eq!(
            RecentList::from_lines(&lines).items(),
            strings(&["x", "y"]).as_slice()
        );
    }

    #[test]
    fn e13_02_from_lines_caps_at_the_maximum() {
        let lines: Vec<String> = (0..12).map(|n| format!("p{n}")).collect();
        let list = RecentList::from_lines(&lines);
        assert_eq!(list.items().len(), MAX_RECENT);
        assert_eq!(list.items()[0], "p0");
        assert_eq!(list.items()[9], "p9");
    }

    #[test]
    fn e13_02_lines_round_trip() {
        let mut list = RecentList::default();
        for path in ["a", "b", "c"] {
            list.push(path);
        }
        assert_eq!(RecentList::from_lines(&list.to_lines()), list);
    }

    #[test]
    fn e13_02_recent_label_is_the_file_name() {
        assert_eq!(recent_label("H:\\roms\\tetris.gb"), "tetris.gb");
        assert_eq!(recent_label("/a/b/zelda.gbc"), "zelda.gbc");
        assert_eq!(recent_label("plain.gb"), "plain.gb");
        assert_eq!(recent_label(""), "");
    }

    #[test]
    fn e13_05_menu_entries_gives_file_name_labels_and_full_paths() {
        let mut list = RecentList::default();
        list.push("H:\\roms\\tetris.gb");
        list.push("/a/b/zelda.gbc");
        assert_eq!(
            menu_entries(&list),
            vec![
                ("zelda.gbc".to_string(), "/a/b/zelda.gbc".to_string()),
                ("tetris.gb".to_string(), "H:\\roms\\tetris.gb".to_string()),
            ]
        );
    }

    #[test]
    fn e13_05_menu_entries_empty_list_gives_an_empty_vec() {
        assert!(menu_entries(&RecentList::default()).is_empty());
    }
}
