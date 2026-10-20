use crate::workspace::FileIndex;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub enum Command {
    New,
    OpenFile,
    OpenFolder,
    Save,
    SaveAs,
    Close,
    Find,
    Sidebar,
    Terminal,
    Restart,
    NewTerminal,
    FocusTerminal,
    FocusEditor,
    ZoomIn,
    ZoomOut,
    AppZoomIn,
    AppZoomOut,
    AppZoomReset,
    Refresh,
    Preferences,
    Preview,
}
impl Command {
    pub fn all() -> &'static [(Self, &'static str, &'static str)] {
        use Command::*;
        &[
            (New, "File: New Text File", "N"),
            (OpenFile, "File: Open File…", "O"),
            (OpenFolder, "File: Open Folder…", "⇧ O"),
            (Save, "File: Save", "S"),
            (SaveAs, "File: Save As…", "⇧ S"),
            (Close, "File: Close Editor", "W"),
            (Find, "Edit: Find and Replace", "F"),
            (Sidebar, "View: Toggle Sidebar", "B"),
            (Terminal, "View: Toggle Terminal", "J"),
            (FocusTerminal, "Terminal: Focus Session", ""),
            (NewTerminal, "Terminal: New Session", "⇧ N"),
            (Restart, "Terminal: Restart in Workspace", ""),
            (FocusEditor, "View: Focus Editor", "F6"),
            (ZoomIn, "Editor: Increase Font Size", "+"),
            (ZoomOut, "Editor: Decrease Font Size", "−"),
            (AppZoomIn, "View: Zoom In", "+"),
            (AppZoomOut, "View: Zoom Out", "−"),
            (AppZoomReset, "View: Reset Zoom", "0"),
            (Refresh, "Explorer: Refresh Files", ""),
            (Preferences, "Preferences: Open Settings", ","),
            (Preview, "View: Toggle Markdown / Mermaid Preview", "⇧ V"),
        ]
    }
}
#[derive(Debug, Clone)]
pub enum Action {
    Command(Command),
    File(PathBuf),
}
pub struct Item {
    pub title: String,
    pub detail: String,
    pub action: Action,
}
pub struct Palette {
    pub query: String,
    pub selected: usize,
    pub indexing: bool,
}
impl Palette {
    pub fn new(commands: bool) -> Self {
        Self {
            query: if commands { ">".into() } else { String::new() },
            selected: 0,
            indexing: false,
        }
    }
    pub fn commands(&self) -> bool {
        self.query.starts_with('>')
    }
    pub fn items(&self, index: &FileIndex, root: Option<&Path>) -> Vec<Item> {
        let query = self.query.trim_start_matches('>').trim().to_lowercase();
        let mut items: Vec<(u32, Item)> = if self.commands() {
            Command::all()
                .iter()
                .filter_map(|(command, title, key)| {
                    score(&title.to_lowercase(), &query).map(|rank| {
                        (
                            rank,
                            Item {
                                title: (*title).into(),
                                detail: if key.is_empty() {
                                    String::new()
                                } else if *key == "F6" {
                                    (*key).into()
                                } else {
                                    let modifier =
                                        if matches!(command, Command::ZoomIn | Command::ZoomOut) {
                                            if cfg!(target_os = "macos") {
                                                "⌥"
                                            } else {
                                                "Alt"
                                            }
                                        } else if cfg!(target_os = "macos") {
                                            "⌘"
                                        } else {
                                            "Ctrl"
                                        };
                                    format!("{modifier} {key}")
                                },
                                action: Action::Command(*command),
                            },
                        )
                    })
                })
                .collect()
        } else {
            index
                .files
                .iter()
                .filter_map(|path| {
                    let relative = path
                        .strip_prefix(root.unwrap_or(Path::new("")))
                        .unwrap_or(path);
                    let label = relative.to_string_lossy();
                    score(&label.to_lowercase(), &query).map(|rank| {
                        (
                            rank,
                            Item {
                                title: path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned(),
                                detail: relative
                                    .parent()
                                    .map(|p| p.display().to_string())
                                    .unwrap_or_default(),
                                action: Action::File(path.clone()),
                            },
                        )
                    })
                })
                .collect()
        };
        items.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.title.cmp(&b.1.title)));
        items.into_iter().take(10).map(|(_, item)| item).collect()
    }
}
fn score(value: &str, query: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }
    if let Some(offset) = value.find(query) {
        return Some(offset as u32);
    }
    let mut remaining = value.char_indices();
    let mut rank = 100;
    for character in query.chars() {
        rank += remaining.find(|(_, c)| *c == character)?.0 as u32;
    }
    Some(rank)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fuzzy_file_search_and_command_mode() {
        let files = FileIndex {
            files: vec![
                PathBuf::from("/project/src/main.rs"),
                PathBuf::from("/project/README.md"),
            ],
            limited: false,
        };
        let mut palette = Palette::new(false);
        palette.query = "smrs".into();
        assert_eq!(
            palette.items(&files, Some(Path::new("/project")))[0].title,
            "main.rs"
        );
        palette.query = ">save".into();
        let items = palette.items(&files, None);
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| matches!(i.action, Action::Command(_))));
    }

    #[test]
    fn focus_editor_displays_the_unmodified_function_key() {
        let files = FileIndex {
            files: vec![],
            limited: false,
        };
        let mut palette = Palette::new(true);
        palette.query = ">focus editor".into();
        let items = palette.items(&files, None);
        assert_eq!(items.len(), 1);
        assert!(matches!(
            items[0].action,
            Action::Command(Command::FocusEditor)
        ));
        assert_eq!(items[0].detail, "F6");
    }

    #[test]
    fn preferences_are_searchable_by_settings() {
        let files = FileIndex {
            files: vec![],
            limited: false,
        };
        let mut palette = Palette::new(true);
        palette.query = ">settings".into();
        let items = palette.items(&files, None);
        assert_eq!(items.len(), 1);
        assert!(matches!(
            items[0].action,
            Action::Command(Command::Preferences)
        ));
        assert_eq!(items[0].title, "Preferences: Open Settings");
        assert_eq!(
            items[0].detail,
            if cfg!(target_os = "macos") {
                "⌘ ,"
            } else {
                "Ctrl ,"
            }
        );
    }

    #[test]
    fn app_zoom_and_editor_font_size_have_distinct_shortcuts() {
        let files = FileIndex::default();
        let mut palette = Palette::new(true);
        palette.query = ">zoom".into();
        let items = palette.items(&files, None);
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|item| matches!(
            item.action,
            Action::Command(Command::AppZoomIn | Command::AppZoomOut | Command::AppZoomReset)
        )));
        let modifier = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Ctrl"
        };
        assert!(items.iter().all(|item| item.detail.starts_with(modifier)));
        let reset = items
            .iter()
            .find(|item| matches!(item.action, Action::Command(Command::AppZoomReset)))
            .unwrap();
        assert_eq!(reset.detail, format!("{modifier} 0"));

        palette.query = ">font size".into();
        let items = palette.items(&files, None);
        assert_eq!(items.len(), 2);
        let modifier = if cfg!(target_os = "macos") {
            "⌥"
        } else {
            "Alt"
        };
        assert!(items.iter().all(|item| item.detail.starts_with(modifier)));
    }
}
