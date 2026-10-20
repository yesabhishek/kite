use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub directory: bool,
    pub depth: usize,
}

#[derive(Debug, Clone, Default)]
pub struct FileIndex {
    pub files: Vec<PathBuf>,
    pub limited: bool,
}

pub fn index_files(root: &Path) -> FileIndex {
    let mut result = FileIndex::default();
    let mut walked = 0;
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .follow_links(false)
        .require_git(false)
        .max_depth(Some(32))
        .filter_entry(|entry| {
            entry.depth() == 0
                || !entry.file_type().is_some_and(|t| t.is_dir())
                || !matches!(
                    entry.file_name().to_str(),
                    Some(".git" | "node_modules" | "target" | "dist" | ".next" | ".venv")
                )
        })
        .build();
    for entry in walker.flatten() {
        walked += 1;
        if walked > 50_000 || result.files.len() >= 10_000 {
            result.limited = true;
            break;
        }
        if entry.file_type().is_some_and(|t| t.is_file()) {
            result.files.push(entry.into_path());
        }
    }
    result.files.sort();
    result
}

pub fn read_directory(path: &Path, depth: usize) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == ".git" {
            continue;
        }
        // Symlinks remain files in the tree so a symlink cycle cannot recurse indefinitely.
        let directory = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        entries.push(Entry {
            path: entry.path(),
            name,
            directory,
            depth,
        });
    }
    entries.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quick_open_respects_ignore_files_and_skips_build_trees() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "secret.txt\n").unwrap();
        fs::write(dir.path().join("secret.txt"), "").unwrap();
        fs::write(dir.path().join("main.rs"), "").unwrap();
        fs::create_dir(dir.path().join("node_modules")).unwrap();
        fs::write(dir.path().join("node_modules/large.js"), "").unwrap();
        let index = index_files(dir.path());
        assert!(index.files.iter().any(|p| p.ends_with("main.rs")));
        assert!(
            !index
                .files
                .iter()
                .any(|p| p.ends_with("secret.txt") || p.ends_with("large.js"))
        );
    }
    #[test]
    fn tree_is_sorted_lazy_and_hides_git() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("z-dir")).unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join("b.rs"), "").unwrap();
        fs::write(dir.path().join("A.rs"), "").unwrap();
        let entries = read_directory(dir.path(), 0).unwrap();
        assert_eq!(
            entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["z-dir", "A.rs", "b.rs"]
        );
        assert!(entries[0].directory);
    }
}
