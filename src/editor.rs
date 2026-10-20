//! Undo records contain only touched lines, not a copy of the document per key.
use iced::advanced::text::editor::Position;
use iced::widget::text_editor::{Action, Content, Cursor, Edit};
use std::{collections::VecDeque, sync::Arc};

const MAX_HISTORY_BYTES: usize = 2 * 1024 * 1024;
const MAX_HISTORY_ENTRIES: usize = 512;

struct Change {
    start: usize,
    before: String,
    after: String,
    before_lines: usize,
    after_lines: usize,
    before_cursor: Cursor,
    after_cursor: Cursor,
    before_version: u64,
    after_version: u64,
}
impl Change {
    fn bytes(&self) -> usize {
        self.before.len() + self.after.len()
    }
}

pub struct Editor {
    pub content: Content,
    undo: VecDeque<Change>,
    redo: Vec<Change>,
    history_bytes: usize,
    pub version: u64,
    next_version: u64,
    pub saved_version: u64,
}

impl Editor {
    pub fn new(text: &str) -> Self {
        Self {
            content: Content::with_text(text),
            undo: VecDeque::new(),
            redo: vec![],
            history_bytes: 0,
            version: 0,
            next_version: 1,
            saved_version: 0,
        }
    }
    pub fn dirty(&self) -> bool {
        self.version != self.saved_version
    }
    pub fn perform(&mut self, action: Action) {
        if !action.is_edit() {
            self.content.perform(action);
            return;
        }
        let before_cursor = self.content.cursor();
        let other = before_cursor.selection.unwrap_or(before_cursor.position);
        let start = before_cursor
            .position
            .line
            .min(other.line)
            .saturating_sub(1);
        let total_before = self.content.line_count();
        let end = (before_cursor.position.line.max(other.line) + 2).min(total_before);
        let before_lines = end.saturating_sub(start);
        let before = span(&self.content, start, before_lines);
        self.content.perform(action);
        let after_lines = before_lines
            .saturating_add(self.content.line_count())
            .saturating_sub(total_before);
        let after = span(&self.content, start, after_lines);
        if before == after {
            return;
        }
        let after_version = self.next_version;
        self.next_version += 1;
        let change = Change {
            start,
            before,
            after,
            before_lines,
            after_lines,
            before_cursor,
            after_cursor: self.content.cursor(),
            before_version: self.version,
            after_version,
        };
        self.version = after_version;
        self.redo.clear();
        self.history_bytes += change.bytes();
        self.undo.push_back(change);
        while self.undo.len() > 1
            && (self.undo.len() > MAX_HISTORY_ENTRIES || self.history_bytes > MAX_HISTORY_BYTES)
        {
            if let Some(change) = self.undo.pop_front() {
                self.history_bytes = self.history_bytes.saturating_sub(change.bytes());
            } else {
                break;
            }
        }
    }
    pub fn undo(&mut self) {
        if let Some(change) = self.undo.pop_back() {
            self.history_bytes = self.history_bytes.saturating_sub(change.bytes());
            replace_span(
                &mut self.content,
                change.start,
                change.after_lines,
                &change.before,
            );
            self.content.move_to(change.before_cursor);
            self.version = change.before_version;
            self.redo.push(change);
        }
    }
    pub fn redo(&mut self) {
        if let Some(change) = self.redo.pop() {
            replace_span(
                &mut self.content,
                change.start,
                change.before_lines,
                &change.after,
            );
            self.content.move_to(change.after_cursor);
            self.version = change.after_version;
            self.history_bytes += change.bytes();
            self.undo.push_back(change);
        }
    }
    pub fn select_bytes(&mut self, range: std::ops::Range<usize>) {
        let value = self.content.text();
        self.content.move_to(Cursor {
            position: position(&value, range.end),
            selection: Some(position(&value, range.start)),
        });
    }
}

fn position(text: &str, byte: usize) -> Position {
    let prefix = &text[..byte.min(text.len())];
    Position {
        line: prefix.bytes().filter(|b| *b == b'\n').count(),
        column: prefix.rsplit('\n').next().unwrap_or("").len(),
    }
}

fn span(content: &Content, start: usize, count: usize) -> String {
    let mut result = String::new();
    for index in start..start + count {
        if let Some(line) = content.line(index) {
            result.push_str(&line.text);
            if index + 1 < content.line_count() {
                result.push('\n');
            }
        }
    }
    result
}

fn replace_span(content: &mut Content, start: usize, count: usize, replacement: &str) {
    let end_line = start + count;
    let end = if end_line < content.line_count() {
        Position {
            line: end_line,
            column: 0,
        }
    } else {
        let line = content.line_count().saturating_sub(1);
        Position {
            line,
            column: content.line(line).map(|l| l.text.len()).unwrap_or(0),
        }
    };
    content.move_to(Cursor {
        position: end,
        selection: Some(Position {
            line: start,
            column: 0,
        }),
    });
    content.perform(Action::Edit(Edit::Paste(Arc::new(replacement.to_owned()))));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn undo_redo_multiline_unicode_and_savepoint() {
        let original = "fn main() {\n    println!(\"𝄞\");\n}\n";
        let mut editor = Editor::new(original);
        editor.content.move_to(Cursor {
            position: Position { line: 1, column: 4 },
            selection: None,
        });
        editor.perform(Action::Edit(Edit::Paste(Arc::new("// 中文\n    ".into()))));
        let edited = editor.content.text();
        assert_ne!(edited, original);
        assert!(editor.dirty());
        editor.undo();
        assert_eq!(editor.content.text(), original);
        assert!(!editor.dirty());
        editor.redo();
        assert_eq!(editor.content.text(), edited);
        editor.saved_version = editor.version;
        assert!(!editor.dirty());
        editor.undo();
        assert!(editor.dirty());
    }
    #[test]
    fn deleting_newline_and_entire_buffer_roundtrip() {
        let mut editor = Editor::new("a\nb\nc");
        editor.content.move_to(Cursor {
            position: Position { line: 1, column: 0 },
            selection: None,
        });
        editor.perform(Action::Edit(Edit::Backspace));
        assert_eq!(editor.content.text(), "ab\nc");
        editor.undo();
        assert_eq!(editor.content.text(), "a\nb\nc");
        editor.content.perform(Action::SelectAll);
        editor.perform(Action::Edit(Edit::Paste(Arc::new(String::new()))));
        assert_eq!(editor.content.text(), "");
        editor.undo();
        assert_eq!(editor.content.text(), "a\nb\nc");
        editor.redo();
        assert_eq!(editor.content.text(), "");
    }
    #[test]
    fn edit_after_undo_clears_redo() {
        let mut editor = Editor::new("");
        editor.perform(Action::Edit(Edit::Insert('a')));
        editor.undo();
        editor.perform(Action::Edit(Edit::Insert('b')));
        editor.redo();
        assert_eq!(editor.content.text(), "b");
    }
}
