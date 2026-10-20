use crate::{
    document::{self, LineEnding, LoadedFile},
    editor::Editor,
    icons::{self, Icon},
    palette::{self, Palette},
    settings::{AppZoom, Settings},
    terminal::Terminals,
    workspace::{self, Entry, FileIndex},
};
use iced::{
    Color, Element, Length, Subscription, Task, Theme, alignment, event, keyboard,
    widget::{
        self, button, column, container, mouse_area, pane_grid, row, scrollable, space, text,
        text_editor, text_input,
    },
    window,
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DocumentMode {
    #[default]
    Code,
    Split,
    Preview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewKey {
    version: u64,
    appearance: crate::appearance::Appearance,
    extension: String,
}

#[derive(Default)]
struct DocumentPreview {
    mode: DocumentMode,
    key: Option<PreviewKey>,
    generation: u64,
    loading: bool,
    rendered: Option<crate::preview::Preview>,
    error: Option<String>,
}

struct Tab {
    id: u64,
    path: Option<PathBuf>,
    editor: Editor,
    line_ending: LineEnding,
    bom: bool,
    fingerprint: Option<blake3::Hash>,
    saving: bool,
    preview: DocumentPreview,
}
impl Tab {
    fn name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }
    fn syntax(&self) -> &str {
        self.path
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|s| s.to_str())
            .unwrap_or("txt")
    }
}

#[derive(Debug, Clone, Copy)]
enum Pane {
    Editor,
    Terminal,
}
#[derive(Debug, Clone, Copy)]
enum WorkspacePane {
    Sidebar,
    Editor,
}
#[derive(Debug, Clone, Copy)]
pub enum Pending {
    Close(u64),
    Quit,
}
#[derive(Debug, Clone, Copy)]
pub enum Decision {
    Save,
    Discard,
    Cancel,
}

#[derive(Debug, Clone)]
pub enum Message {
    New,
    OpenFile,
    OpenFolder,
    OpenPath(PathBuf),
    PickedFile(Option<PathBuf>),
    PickedFolder(Option<PathBuf>),
    Loaded(Result<LoadedFile, String>),
    Edit(text_editor::Action),
    Undo,
    Redo,
    Save,
    SaveAs,
    SavePath(u64, Option<PathBuf>),
    Saved(u64, u64, PathBuf, Result<blake3::Hash, String>),
    Tab(usize),
    CloseCurrent,
    NextTab(bool),
    Close(u64),
    Quit,
    Confirmed(Decision),
    ToggleSidebar,
    ToggleSettings,
    CloseSettings,
    ToggleTerminal,
    FocusEditor,
    FocusTerminal,
    RestartTerminal,
    NewTerminal,
    SelectTerminal(usize),
    CloseTerminal(usize),
    ThemeSelected(crate::appearance::Appearance),
    Terminal(iced_term::Event),
    Resized(pane_grid::ResizeEvent),
    ColumnResized(pane_grid::ResizeEvent),
    RefreshTree,
    CollapseTree,
    Expand(PathBuf),
    TreeRead(PathBuf, usize, u64, Result<Vec<Entry>, String>),
    ToggleFind,
    Query(String),
    Replacement(String),
    FindNext,
    Replace,
    ReplaceAll,
    HideFind,
    Zoom(f32),
    AppZoom(i32),
    AppZoomSelected(AppZoom),
    AppZoomReset,
    DocumentMode(DocumentMode),
    TogglePreview,
    PreviewTick(u64, u64),
    PreviewReady(
        u64,
        u64,
        PreviewKey,
        Result<Arc<crate::preview::PreparedPreview>, String>,
    ),
    PreviewLink(String),
    PreviewLinkOpened(Result<(), String>),
    DismissError,
    OpenPalette(bool),
    PaletteQuery(String),
    PaletteMove(i32),
    PaletteExecute(usize),
    PaletteAccept,
    ClosePalette,
    Indexed(u64, FileIndex),
    Key(keyboard::Key, keyboard::Modifiers),
    DragWindow,
}

pub struct App {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    folder: Option<PathBuf>,
    tree: Vec<Entry>,
    expanded: HashSet<PathBuf>,
    tree_generation: u64,
    terminal: Terminals,
    panes: pane_grid::State<Pane>,
    columns: pane_grid::State<WorkspacePane>,
    settings: Settings,
    error: Option<String>,
    status: String,
    find: bool,
    settings_open: bool,
    query: String,
    replacement: String,
    match_end: usize,
    modal: bool,
    pending: Option<Pending>,
    pending_save: Option<u64>,
    quit_discarded: HashSet<u64>,
    palette: Option<Palette>,
    file_index: FileIndex,
    persist_session: bool,
}

impl App {
    pub fn boot() -> (Self, Task<Message>) {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let persist_session = !args.iter().any(|a| a == "--temporary-session");
        let restore = persist_session && !args.iter().any(|a| a == "--no-restore");
        let mut settings = if restore {
            Settings::load()
        } else {
            Settings::default()
        };
        settings.font_size = settings.font_size.clamp(10.0, 28.0);
        let mut paths = Vec::new();
        for arg in args.iter().filter(|a| !a.starts_with('-')) {
            let path = PathBuf::from(arg);
            if path.is_dir() {
                settings.folder = path.canonicalize().ok();
            } else {
                paths.push(path);
            }
        }
        if paths.is_empty() && restore {
            paths = settings.files.clone();
        }
        let folder = settings.folder.clone().filter(|p| p.is_dir());
        let cwd = folder.clone().unwrap_or_else(|| {
            std::env::current_dir()
                .ok()
                .filter(|p| p.parent().is_some())
                .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().to_owned()))
                .unwrap_or_else(|| PathBuf::from("."))
        });
        let (mut panes, editor_pane) = pane_grid::State::new(Pane::Editor);
        if let Some((_, split)) =
            panes.split(pane_grid::Axis::Horizontal, editor_pane, Pane::Terminal)
        {
            panes.resize(split, 0.72);
        }
        let (mut columns, sidebar) = pane_grid::State::new(WorkspacePane::Sidebar);
        if let Some((_, split)) =
            columns.split(pane_grid::Axis::Vertical, sidebar, WorkspacePane::Editor)
        {
            columns.resize(split, 0.19);
        }
        let mut app = Self {
            tabs: vec![],
            active: 0,
            next_id: 1,
            folder,
            tree: vec![],
            expanded: HashSet::new(),
            tree_generation: 0,
            terminal: Terminals::new(cwd),
            panes,
            columns,
            settings,
            error: None,
            status: "Ready".into(),
            find: false,
            settings_open: false,
            query: String::new(),
            replacement: String::new(),
            match_end: 0,
            modal: false,
            pending: None,
            pending_save: None,
            quit_discarded: HashSet::new(),
            palette: None,
            file_index: FileIndex::default(),
            persist_session,
        };
        app.terminal.set_appearance(app.settings.theme);
        let mut tasks: Vec<Task<Message>> = paths.into_iter().map(load_file).collect();
        tasks.push(app.refresh_tree());
        (app, Task::batch(tasks))
    }

    pub fn title(&self) -> String {
        let document = self
            .tabs
            .get(self.active)
            .map(|t| {
                format!(
                    "{}{} - ",
                    if t.editor.dirty() { "● " } else { "" },
                    t.name()
                )
            })
            .unwrap_or_default();
        format!("{document}Kite")
    }
    pub fn theme(&self) -> Theme {
        Theme::custom(
            "Kite",
            iced::theme::Palette {
                background: self.settings.theme.colors().bg,
                text: self.settings.theme.colors().ink,
                primary: self.settings.theme.colors().accent,
                success: self.settings.theme.colors().success,
                warning: self.settings.theme.colors().accent,
                danger: self.settings.theme.colors().danger,
            },
        )
    }
    pub fn scale_factor(&self) -> f32 {
        self.settings.app_zoom.scale_factor()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            self.terminal.subscription().map(Message::Terminal),
            event::listen_with(|event, status, _| match event {
                iced::Event::Window(window::Event::CloseRequested) => Some(Message::Quit),
                iced::Event::Window(window::Event::FileDropped(path)) => {
                    Some(Message::OpenPath(path))
                }
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                    if status == event::Status::Ignored =>
                {
                    Some(Message::Key(key, modifiers))
                }
                _ => None,
            }),
        ])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Ignore editor input queued before an overlay took keyboard focus.
        if (self.settings_open || self.palette.is_some()) && matches!(message, Message::Edit(_)) {
            return Task::none();
        }
        // Native confirmation dialogs own input until their result arrives.
        if self.modal
            && !matches!(
                message,
                Message::Confirmed(_)
                    | Message::SavePath(..)
                    | Message::Saved(..)
                    | Message::Terminal(_)
                    | Message::Loaded(_)
                    | Message::TreeRead(..)
                    | Message::PickedFile(_)
                    | Message::PickedFolder(_)
                    | Message::PreviewTick(..)
                    | Message::PreviewReady(..)
                    | Message::PreviewLinkOpened(_)
            )
        {
            return Task::none();
        }
        match message {
            Message::DragWindow => {
                return window::oldest().then(|id| id.map(window::drag).unwrap_or_else(Task::none));
            }
            Message::Key(key, modifiers) => {
                if self.settings_open {
                    if matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape)) {
                        return self.update(Message::CloseSettings);
                    }
                    if matches!(shortcut(&key, modifiers), Some(Message::ToggleSettings)) {
                        return self.update(Message::CloseSettings);
                    }
                    if let Some(message @ (Message::AppZoom(_) | Message::AppZoomReset)) =
                        shortcut(&key, modifiers)
                    {
                        return self.update(message);
                    }
                    return Task::none();
                }
                if self.palette.is_some() {
                    match key {
                        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                            return self.update(Message::PaletteMove(1));
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                            return self.update(Message::PaletteMove(-1));
                        }
                        keyboard::Key::Named(keyboard::key::Named::Escape) => {
                            return self.update(Message::ClosePalette);
                        }
                        _ => return Task::none(),
                    }
                }
                if let Some(message) = shortcut(&key, modifiers) {
                    return self.update(message);
                }
            }
            Message::OpenPalette(commands) => {
                self.settings_open = false;
                self.palette = Some(Palette::new(commands));
                let focus = widget::operation::focus("palette");
                if !commands {
                    if let Some(root) = self.folder.clone() {
                        self.palette.as_mut().unwrap().indexing = true;
                        let generation = self.tree_generation;
                        return Task::batch([
                            focus,
                            Task::perform(
                                async move {
                                    let index = tokio::task::spawn_blocking(move || {
                                        workspace::index_files(&root)
                                    })
                                    .await
                                    .unwrap_or_default();
                                    (generation, index)
                                },
                                |(generation, index)| Message::Indexed(generation, index),
                            ),
                        ]);
                    }
                    self.file_index.files =
                        self.tabs.iter().filter_map(|t| t.path.clone()).collect();
                }
                return focus;
            }
            Message::PaletteQuery(query) => {
                if let Some(palette) = &mut self.palette {
                    palette.query = query;
                    palette.selected = 0;
                }
                return widget::operation::snap_to(
                    "palette-results",
                    widget::operation::RelativeOffset::START,
                );
            }
            Message::PaletteMove(delta) => {
                if let Some(palette) = &mut self.palette {
                    let count = palette
                        .items(&self.file_index, self.folder.as_deref())
                        .len();
                    if count > 0 {
                        palette.selected =
                            (palette.selected as i32 + delta).rem_euclid(count as i32) as usize;
                        return widget::operation::snap_to(
                            "palette-results",
                            widget::operation::RelativeOffset {
                                x: 0.,
                                y: palette.selected as f32 / count.saturating_sub(1).max(1) as f32,
                            },
                        );
                    }
                }
            }
            Message::PaletteAccept => {
                if let Some(palette) = &self.palette {
                    return self.update(Message::PaletteExecute(palette.selected));
                }
            }
            Message::PaletteExecute(index) => {
                if let Some(palette) = &self.palette {
                    if let Some(item) = palette
                        .items(&self.file_index, self.folder.as_deref())
                        .into_iter()
                        .nth(index)
                    {
                        self.palette = None;
                        return match item.action {
                            palette::Action::Command(command) => {
                                self.update(command_message(command))
                            }
                            palette::Action::File(path) => load_file(path),
                        };
                    }
                }
            }
            Message::ClosePalette => {
                self.palette = None;
                return focus_editor();
            }
            Message::Indexed(generation, index) => {
                if generation == self.tree_generation {
                    self.file_index = index;
                    if let Some(palette) = &mut self.palette {
                        palette.indexing = false;
                    }
                }
            }
            Message::New => {
                let id = self.next_id;
                self.next_id += 1;
                self.tabs.push(Tab {
                    id,
                    path: None,
                    editor: Editor::new(""),
                    line_ending: if cfg!(windows) {
                        LineEnding::CrLf
                    } else {
                        LineEnding::Lf
                    },
                    bom: false,
                    fingerprint: None,
                    saving: false,
                    preview: DocumentPreview::default(),
                });
                self.active = self.tabs.len() - 1;
                self.match_end = 0;
                return focus_editor();
            }
            Message::OpenFile => {
                self.modal = true;
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Open a text file")
                            .pick_file()
                            .await
                            .map(|f| f.path().to_owned())
                    },
                    Message::PickedFile,
                );
            }
            Message::PickedFile(path) => {
                self.modal = false;
                if let Some(path) = path {
                    return load_file(path);
                }
            }
            Message::OpenFolder => {
                self.modal = true;
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Open folder")
                            .pick_folder()
                            .await
                            .map(|f| f.path().to_owned())
                    },
                    Message::PickedFolder,
                );
            }
            Message::PickedFolder(path) => {
                self.modal = false;
                if let Some(path) = path {
                    return self.set_folder(path);
                }
            }
            Message::OpenPath(path) => {
                if path.is_dir() {
                    return self.set_folder(path);
                }
                return load_file(path);
            }
            Message::Loaded(result) => match result {
                Ok(file) => {
                    if let Some(index) = self
                        .tabs
                        .iter()
                        .position(|t| t.path.as_ref() == Some(&file.path))
                    {
                        if !self.modal {
                            self.active = index;
                        }
                    } else {
                        let id = self.next_id;
                        self.next_id += 1;
                        self.tabs.push(Tab {
                            id,
                            path: Some(file.path),
                            editor: Editor::new(&file.text),
                            line_ending: file.line_ending,
                            bom: file.bom,
                            fingerprint: Some(file.fingerprint),
                            saving: false,
                            preview: DocumentPreview::default(),
                        });
                        if !self.modal {
                            self.active = self.tabs.len() - 1;
                        }
                    }
                    self.status = "Ready".into();
                    self.match_end = 0;
                    if !self.modal && !self.settings_open && self.palette.is_none() {
                        return focus_editor();
                    }
                }
                Err(e) => self.error = Some(e),
            },
            Message::Edit(action) => {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    if tab.preview.mode != DocumentMode::Preview {
                        tab.editor.perform(action);
                    }
                }
            }
            Message::Undo => {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.editor.undo();
                }
            }
            Message::Redo => {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.editor.redo();
                }
            }
            Message::Save => return self.save_current(false),
            Message::SaveAs => return self.save_current(true),
            Message::SavePath(id, path) => {
                self.modal = false;
                if let Some(path) = path {
                    return self.save_tab(id, path, true);
                }
                self.pending = None;
                self.quit_discarded.clear();
            }
            Message::Saved(id, version, path, result) => {
                // A background save must not dismiss another tab's dialog or
                // consume its pending close action.
                let completes_pending = self.pending_save == Some(id);
                if completes_pending {
                    self.pending_save = None;
                    self.modal = false;
                }
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                    tab.saving = false;
                    match result {
                        Ok(hash) => {
                            tab.path = Some(path);
                            tab.fingerprint = Some(hash);
                            tab.editor.saved_version = version;
                            self.status = "Saved".into();
                        }
                        Err(error) => {
                            self.error = Some(error);
                            if completes_pending {
                                self.pending = None;
                                self.quit_discarded.clear();
                            }
                            return Task::none();
                        }
                    }
                }
                if completes_pending && let Some(action) = self.pending.take() {
                    return self.continue_action(action);
                }
            }
            Message::Tab(index) => {
                if index < self.tabs.len() {
                    self.active = index;
                    self.match_end = 0;
                    return Task::batch([focus_editor(), self.queue_preview(false)]);
                }
            }
            Message::CloseCurrent => {
                if let Some(tab) = self.tabs.get(self.active) {
                    return self.update(Message::Close(tab.id));
                }
            }
            Message::NextTab(backwards) => {
                if !self.tabs.is_empty() {
                    self.active = if backwards {
                        (self.active + self.tabs.len() - 1) % self.tabs.len()
                    } else {
                        (self.active + 1) % self.tabs.len()
                    };
                    self.match_end = 0;
                    return Task::batch([focus_editor(), self.queue_preview(false)]);
                }
            }
            Message::Close(id) => {
                if self.tabs.iter().any(|t| t.id == id && t.saving) {
                    self.status = "Wait for this file to finish saving".into();
                } else if let Some(tab) = self.tabs.iter().find(|t| t.id == id && t.editor.dirty())
                {
                    return self.confirm(Pending::Close(id), tab.name());
                } else {
                    self.close_tab(id);
                }
            }
            Message::Quit => return self.quit(),
            Message::Confirmed(decision) => {
                self.modal = false;
                if let Some(action) = self.pending.take() {
                    match decision {
                        Decision::Cancel => {
                            self.quit_discarded.clear();
                        }
                        Decision::Discard => {
                            if let Pending::Quit = action {
                                if let Some(tab) = self.tabs.get(self.active) {
                                    self.quit_discarded.insert(tab.id);
                                }
                            }
                            return self.continue_action(action);
                        }
                        Decision::Save => {
                            self.pending = Some(action);
                            return self.save_current(false);
                        }
                    }
                }
            }
            Message::ThemeSelected(appearance) => {
                self.settings.theme = appearance;
                self.terminal.set_appearance(appearance);
                self.status = format!("{appearance} theme");
            }
            Message::NewTerminal => {
                self.settings.terminal = true;
                let cwd = self
                    .folder
                    .clone()
                    .unwrap_or_else(|| self.terminal.cwd().to_owned());
                return self.terminal.add(cwd).map(Message::Terminal);
            }
            Message::SelectTerminal(index) => {
                if index < self.terminal.sessions.len() {
                    self.terminal.active = index;
                }
                return self.terminal.focus().map(Message::Terminal);
            }
            Message::CloseTerminal(index) => {
                return self.terminal.close(index).map(Message::Terminal);
            }
            Message::ToggleSettings => {
                self.settings_open = !self.settings_open;
                self.palette = None;
                return if self.settings_open {
                    widget::operation::focus("settings")
                } else {
                    focus_editor()
                };
            }
            Message::CloseSettings => {
                self.settings_open = false;
                return focus_editor();
            }
            Message::ToggleSidebar => self.settings.sidebar = !self.settings.sidebar,
            Message::ToggleTerminal => {
                self.settings.terminal = !self.settings.terminal;
                if self.settings_open {
                    return Task::none();
                }
                if self.settings.terminal {
                    return self.terminal.focus().map(Message::Terminal);
                }
                return focus_editor();
            }
            Message::FocusTerminal => {
                self.settings.terminal = true;
                return self.terminal.focus().map(Message::Terminal);
            }
            Message::RestartTerminal => {
                return self
                    .terminal
                    .restart_in(
                        self.folder
                            .clone()
                            .unwrap_or_else(|| self.terminal.cwd().to_owned()),
                    )
                    .map(Message::Terminal);
            }
            Message::Terminal(event) => return self.terminal.update(event).map(Message::Terminal),
            Message::FocusEditor => return focus_editor(),
            Message::Resized(event) => self
                .panes
                .resize(event.split, event.ratio.clamp(0.18, 0.88)),
            Message::ColumnResized(event) => self
                .columns
                .resize(event.split, event.ratio.clamp(0.12, 0.42)),
            Message::CollapseTree => {
                self.tree_generation += 1;
                self.expanded.clear();
                self.tree.retain(|entry| entry.depth == 0);
            }
            Message::RefreshTree => return self.refresh_tree(),
            Message::Expand(path) => {
                if self.expanded.remove(&path) {
                    self.tree
                        .retain(|e| e.path == path || !e.path.starts_with(&path));
                    self.expanded.retain(|p| !p.starts_with(&path));
                } else if let Some(entry) = self.tree.iter().find(|e| e.path == path) {
                    let depth = entry.depth + 1;
                    self.expanded.insert(path.clone());
                    return read_tree(path, depth, self.tree_generation);
                }
            }
            Message::TreeRead(path, depth, generation, result) => {
                if generation != self.tree_generation {
                    return Task::none();
                }
                match result {
                    Ok(entries) => {
                        if depth == 0 {
                            self.tree = entries;
                        } else if self.expanded.contains(&path) {
                            if let Some(index) = self.tree.iter().position(|e| e.path == path) {
                                self.tree.splice(index + 1..index + 1, entries);
                            }
                        }
                    }
                    Err(e) => {
                        self.expanded.remove(&path);
                        self.error = Some(e);
                    }
                }
            }
            Message::ToggleFind => {
                self.find = !self.find;
                self.match_end = 0;
                if self.find {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.preview.mode == DocumentMode::Preview {
                            tab.preview.mode = DocumentMode::Split;
                        }
                    }
                    return Task::batch([
                        widget::operation::focus("find"),
                        self.queue_preview(false),
                    ]);
                }
                return focus_editor();
            }
            Message::HideFind => {
                self.find = false;
                return focus_editor();
            }
            Message::Query(query) => {
                self.query = query;
                self.match_end = 0;
            }
            Message::Replacement(value) => self.replacement = value,
            Message::FindNext => return self.find_next(),
            Message::Replace => {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    if !self.query.is_empty()
                        && tab.editor.content.selection().as_deref() == Some(self.query.as_str())
                    {
                        tab.editor
                            .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                                Arc::new(self.replacement.clone()),
                            )));
                        self.match_end = 0;
                    }
                }
                return Task::batch([self.find_next(), self.queue_preview(true)]);
            }
            Message::ReplaceAll => {
                if !self.query.is_empty() {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        let value = tab.editor.content.text();
                        let count = value.matches(&self.query).count();
                        if count > 0 {
                            tab.editor.content.perform(text_editor::Action::SelectAll);
                            tab.editor.perform(text_editor::Action::Edit(
                                text_editor::Edit::Paste(Arc::new(
                                    value.replace(&self.query, &self.replacement),
                                )),
                            ));
                        }
                        self.status = format!("Replaced {count} matches");
                        self.match_end = 0;
                    }
                }
            }
            Message::Zoom(delta) => {
                self.settings.font_size = (self.settings.font_size + delta).clamp(10.0, 28.0)
            }
            Message::AppZoom(delta) => self.settings.app_zoom = self.settings.app_zoom.step(delta),
            Message::AppZoomSelected(zoom) => self.settings.app_zoom = zoom,
            Message::AppZoomReset => self.settings.app_zoom = AppZoom::default(),
            Message::DocumentMode(mode) => {
                let Some(tab) = self.tabs.get_mut(self.active) else {
                    return Task::none();
                };
                if mode != DocumentMode::Code
                    && !tab
                        .path
                        .as_deref()
                        .is_some_and(crate::preview::is_supported)
                {
                    self.status = "Preview is available for Markdown and Mermaid files".into();
                    return Task::none();
                }
                tab.preview.mode = mode;
                if mode == DocumentMode::Code
                    && (tab.preview.loading || tab.preview.error.is_some())
                {
                    tab.preview.key = None;
                    tab.preview.generation = tab.preview.generation.wrapping_add(1);
                    tab.preview.loading = false;
                }
                if mode == DocumentMode::Preview {
                    self.find = false;
                }
                let focus = if mode == DocumentMode::Preview {
                    widget::operation::focus("preview")
                } else {
                    focus_editor()
                };
                return Task::batch([focus, self.queue_preview(false)]);
            }
            Message::TogglePreview => {
                let mode = self
                    .tabs
                    .get(self.active)
                    .map(|tab| tab.preview.mode)
                    .unwrap_or_default();
                return self.update(Message::DocumentMode(if mode == DocumentMode::Preview {
                    DocumentMode::Code
                } else {
                    DocumentMode::Preview
                }));
            }
            Message::PreviewTick(id, generation) => return self.prepare_preview(id, generation),
            Message::PreviewReady(id, generation, key, result) => {
                if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                    if tab.preview.generation == generation
                        && tab.preview.key.as_ref() == Some(&key)
                    {
                        tab.preview.loading = false;
                        // A background tab can cache the theme it rendered;
                        // queue_preview checks that key before displaying it again.
                        if tab.editor.version == key.version
                            && tab.syntax().to_lowercase() == key.extension
                        {
                            match result {
                                Ok(prepared) => {
                                    tab.preview.rendered =
                                        Some(crate::preview::Preview::new((*prepared).clone()));
                                    tab.preview.error = None;
                                }
                                Err(error) => tab.preview.error = Some(error),
                            }
                        } else {
                            tab.preview.key = None;
                        }
                    }
                }
            }
            Message::PreviewLink(uri) => return self.open_preview_link(uri),
            Message::PreviewLinkOpened(result) => {
                if let Err(error) = result {
                    self.error = Some(error);
                }
            }
            Message::DismissError => self.error = None,
        }
        self.queue_preview(true)
    }

    fn queue_preview(&mut self, debounce: bool) -> Task<Message> {
        let appearance = self.settings.theme;
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return Task::none();
        };
        if !tab
            .path
            .as_deref()
            .is_some_and(crate::preview::is_supported)
        {
            if tab.preview.mode != DocumentMode::Code {
                tab.preview = DocumentPreview::default();
                return focus_editor();
            }
            return Task::none();
        }
        if tab.preview.mode == DocumentMode::Code {
            return Task::none();
        }
        let key = PreviewKey {
            version: tab.editor.version,
            appearance,
            extension: tab.syntax().to_lowercase(),
        };
        if tab.preview.key.as_ref() == Some(&key) {
            return Task::none();
        }
        tab.preview.key = Some(key);
        tab.preview.generation = tab.preview.generation.wrapping_add(1);
        tab.preview.loading = true;
        tab.preview.error = None;
        let id = tab.id;
        let generation = tab.preview.generation;
        if debounce {
            Task::perform(
                async move {
                    tokio::time::sleep(std::time::Duration::from_millis(180)).await;
                    (id, generation)
                },
                |(id, generation)| Message::PreviewTick(id, generation),
            )
        } else {
            self.prepare_preview(id, generation)
        }
    }

    fn prepare_preview(&self, id: u64, generation: u64) -> Task<Message> {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) else {
            return Task::none();
        };
        if tab.preview.generation != generation || tab.preview.mode == DocumentMode::Code {
            return Task::none();
        }
        let Some(key) = tab.preview.key.clone() else {
            return Task::none();
        };
        let source = tab.editor.content.text();
        let worker_key = key.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    Arc::new(crate::preview::prepare(
                        &source,
                        &worker_key.extension,
                        worker_key.appearance,
                    ))
                })
                .await
                .map_err(|error| format!("Preview could not be rendered: {error}"))
            },
            move |result| Message::PreviewReady(id, generation, key.clone(), result),
        )
    }

    fn document_preview<'a>(&'a self, tab: &'a Tab) -> Element<'a, Message> {
        let c = self.settings.theme.colors();
        let mut body = column![].spacing(14);
        if let Some(error) = &tab.preview.error {
            body = body.push(
                container(
                    column![
                        text("Preview unavailable").size(14).color(c.ink),
                        text(error).size(12).color(c.muted)
                    ]
                    .spacing(8),
                )
                .padding(20),
            );
        } else if let Some(rendered) = &tab.preview.rendered {
            body = body.push(rendered.view(self.settings.theme).map(|event| match event {
                crate::preview::PreviewEvent::Link(uri) => Message::PreviewLink(uri),
            }));
        } else {
            body = body.push(
                container(
                    text(if tab.preview.loading {
                        "Rendering preview…"
                    } else {
                        "Preparing preview…"
                    })
                    .size(13)
                    .color(c.muted),
                )
                .padding(20),
            );
        }
        let id = format!("document-preview-{}", tab.id);
        let view = scrollable(
            container(container(body).width(Length::Fill).max_width(1000))
                .padding(24)
                .center_x(Length::Fill),
        )
        .id(id)
        .height(Length::Fill)
        .width(Length::Fill);
        container(view)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style::default().background(c.bg))
            .into()
    }

    fn preview_mode_controls(&self, mode: DocumentMode) -> Element<'_, Message> {
        let c = self.settings.theme.colors();
        let mut controls = row![].spacing(2);
        for (option, label) in [
            (DocumentMode::Code, "Code"),
            (DocumentMode::Split, "Split"),
            (DocumentMode::Preview, "Preview"),
        ] {
            controls = controls.push(
                button(text(label).size(10))
                    .on_press(Message::DocumentMode(option))
                    .padding([5, 8])
                    .style(move |_, status| button::Style {
                        background: if mode == option {
                            Some(c.raised.into())
                        } else if status == button::Status::Hovered {
                            Some(c.hover.into())
                        } else {
                            None
                        },
                        text_color: if mode == option { c.ink } else { c.muted },
                        border: iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
            );
        }
        controls.into()
    }

    fn open_preview_link(&mut self, uri: String) -> Task<Message> {
        if uri.starts_with("https://") || uri.starts_with("http://") {
            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        #[cfg(target_os = "macos")]
                        let mut command = std::process::Command::new("open");
                        #[cfg(target_os = "windows")]
                        let mut command = std::process::Command::new("explorer.exe");
                        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                        let mut command = std::process::Command::new("xdg-open");
                        command
                            .arg(uri)
                            .spawn()
                            .map(|_| ())
                            .map_err(|error| error.to_string())
                    })
                    .await
                    .unwrap_or_else(|error| Err(error.to_string()))
                },
                Message::PreviewLinkOpened,
            );
        }
        if uri.starts_with('#') {
            self.status = "Section links are available in the source".into();
        } else if !uri.contains(':') {
            let destination = uri.split('#').next().unwrap_or("").replace("%20", " ");
            if let Some(parent) = self
                .tabs
                .get(self.active)
                .and_then(|tab| tab.path.as_ref())
                .and_then(|path| path.parent())
            {
                let path = parent.join(destination);
                if path.is_file() {
                    return load_file(path);
                }
                self.error = Some(format!("Cannot find {}", path.display()));
            }
        } else {
            self.status = "This link type is not supported in preview".into();
        }
        Task::none()
    }

    fn set_folder(&mut self, path: PathBuf) -> Task<Message> {
        self.folder = Some(path);
        // Changing folders never kills or sends commands to a live shell.
        self.status = "Folder opened · New terminals start here".into();
        self.refresh_tree()
    }
    fn refresh_tree(&mut self) -> Task<Message> {
        self.tree_generation += 1;
        self.file_index = FileIndex::default();
        self.expanded.clear();
        self.tree.clear();
        self.folder
            .clone()
            .map(|path| read_tree(path, 0, self.tree_generation))
            .unwrap_or_else(Task::none)
    }
    fn close_tab(&mut self, id: u64) {
        if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(index);
            if index < self.active {
                self.active -= 1;
            }
            self.active = self.active.min(self.tabs.len().saturating_sub(1));
            self.match_end = 0;
        }
    }
    fn save_current(&mut self, save_as: bool) -> Task<Message> {
        let Some(tab) = self.tabs.get(self.active) else {
            return Task::none();
        };
        if tab.saving {
            return Task::none();
        }
        if save_as || tab.path.is_none() {
            self.modal = true;
            let id = tab.id;
            let name = tab.name();
            let directory = tab
                .path
                .as_ref()
                .and_then(|p| p.parent())
                .map(Path::to_owned)
                .or(self.folder.clone());
            return Task::perform(
                async move {
                    let mut picker = rfd::AsyncFileDialog::new()
                        .set_title("Save file")
                        .set_file_name(&name);
                    if let Some(directory) = directory {
                        picker = picker.set_directory(directory);
                    }
                    (id, picker.save_file().await.map(|f| f.path().to_owned()))
                },
                |(id, path)| Message::SavePath(id, path),
            );
        }
        self.save_tab(tab.id, tab.path.clone().unwrap(), false)
    }
    fn save_tab(&mut self, id: u64, path: PathBuf, selected_destination: bool) -> Task<Message> {
        if self
            .tabs
            .iter()
            .any(|t| t.id != id && t.path.as_deref() == Some(path.as_path()))
        {
            self.error = Some("That file is already open in another tab.".into());
            self.pending = None;
            self.quit_discarded.clear();
            return Task::none();
        }
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) else {
            return Task::none();
        };
        let text = tab.editor.content.text();
        let ending = tab.line_ending;
        let bom = tab.bom;
        let version = tab.editor.version;
        let original = tab.fingerprint;
        let same_path = tab.path.as_ref() == Some(&path);
        tab.saving = true;
        if self.pending.is_some() {
            self.modal = true;
            self.pending_save = Some(id);
        }
        Task::perform(
            async move {
                let output = path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    let expected = if selected_destination && !same_path {
                        document::fingerprint(&path)?
                    } else {
                        original
                    };
                    document::save(&path, &text, ending, bom, expected)
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
                (id, version, output, result)
            },
            |(id, version, path, result)| Message::Saved(id, version, path, result),
        )
    }
    fn confirm(&mut self, action: Pending, name: String) -> Task<Message> {
        if let Pending::Close(id) = action {
            if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
                self.active = index;
            }
        }
        self.pending = Some(action);
        self.modal = true;
        Task::perform(
            async move {
                match rfd::AsyncMessageDialog::new()
                    .set_title("Unsaved changes")
                    .set_description(format!("Save changes to {name} before closing?"))
                    .set_buttons(rfd::MessageButtons::YesNoCancelCustom(
                        "Save".into(),
                        "Discard".into(),
                        "Cancel".into(),
                    ))
                    .show()
                    .await
                {
                    rfd::MessageDialogResult::Yes => Decision::Save,
                    rfd::MessageDialogResult::No => Decision::Discard,
                    rfd::MessageDialogResult::Custom(value) if value == "Save" => Decision::Save,
                    rfd::MessageDialogResult::Custom(value) if value == "Discard" => {
                        Decision::Discard
                    }
                    _ => Decision::Cancel,
                }
            },
            Message::Confirmed,
        )
    }
    fn continue_action(&mut self, action: Pending) -> Task<Message> {
        match action {
            Pending::Close(id) => {
                self.close_tab(id);
                focus_editor()
            }
            Pending::Quit => self.quit(),
        }
    }
    fn quit(&mut self) -> Task<Message> {
        if self.tabs.iter().any(|t| t.saving) {
            self.status = "Wait for saving to finish, then close again".into();
            return Task::none();
        }
        if let Some(index) = self
            .tabs
            .iter()
            .position(|t| t.editor.dirty() && !self.quit_discarded.contains(&t.id))
        {
            self.active = index;
            return self.confirm(Pending::Quit, self.tabs[index].name());
        }
        self.settings.folder = self.folder.clone();
        self.settings.files = self.tabs.iter().filter_map(|t| t.path.clone()).collect();
        if self.persist_session
            && let Err(e) = self.settings.store()
        {
            self.error = Some(format!("Cannot save session: {e}"));
        }
        iced::exit()
    }
    fn find_next(&mut self) -> Task<Message> {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            if let Some(range) =
                document::next_match(&tab.editor.content.text(), &self.query, self.match_end)
            {
                self.match_end = range.end;
                tab.editor.select_bytes(range);
                self.status = "Match selected".into();
                return focus_editor();
            }
            self.status = if self.query.is_empty() {
                "Enter text to find"
            } else {
                "No matches"
            }
            .into();
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let c = self.settings.theme.colors();
        let toolbar = container(widget::responsive(move |size| self.titlebar(size)))
            .height(44. / self.scale_factor().min(1.));
        let editor: Element<'_, Message> = if self.settings.sidebar {
            pane_grid::PaneGrid::new(&self.columns, |_, pane, _| {
                pane_grid::Content::new(match pane {
                    WorkspacePane::Sidebar => self.sidebar(),
                    WorkspacePane::Editor => self.editor_frame(),
                })
            })
            .spacing(1)
            .on_resize(6, Message::ColumnResized)
            .into()
        } else {
            self.editor_frame()
        };
        let activity = container(
            column![
                container(crate::brand::mark(27., self.settings.theme)).padding([7, 0]),
                activity_button(
                    "Explorer",
                    Icon::Files,
                    Message::ToggleSidebar,
                    self.settings.sidebar
                ),
                activity_button(
                    "Quick open",
                    Icon::Search,
                    Message::OpenPalette(false),
                    false
                ),
                activity_button(
                    "Terminal",
                    Icon::Terminal,
                    Message::ToggleTerminal,
                    self.settings.terminal
                ),
                space::vertical(),
                activity_button(
                    "Command palette",
                    Icon::Command,
                    Message::OpenPalette(true),
                    self.palette.is_some()
                ),
                activity_button(
                    "Settings",
                    Icon::Settings,
                    Message::ToggleSettings,
                    self.settings_open
                ),
            ]
            .spacing(7)
            .padding([8, 7])
            .align_x(alignment::Horizontal::Center)
            .height(Length::Fill),
        )
        .width(48)
        .height(Length::Fill)
        .style(move |_| container::Style::default().background(c.panel));
        let body = row![
            activity,
            hairline(c.edge, true),
            container(editor).width(Length::Fill).height(Length::Fill)
        ];
        let mut content = column![toolbar, hairline(c.edge, false)];
        if let Some(error) = &self.error {
            content = content.push(
                container(
                    row![
                        icons::icon_tinted(Icon::Info, c.danger),
                        text(error).size(12).color(c.ink).width(Length::Fill),
                        icon_button("Dismiss notification", Icon::Close, Message::DismissError),
                    ]
                    .spacing(10)
                    .align_y(alignment::Vertical::Center),
                )
                .padding([6, 14])
                .width(Length::Fill)
                .style(move |_| {
                    container::Style::default().background(blend(c.bg, c.danger, 0.12))
                }),
            );
        }
        let base: Element<'_, Message> = container(
            content
                .push(body.height(Length::Fill))
                .push(hairline(c.edge, false))
                .push(self.statusbar()),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style::default().background(c.bg))
        .into();
        let overlay_content = if let Some(palette) = &self.palette {
            Some((self.palette_view(palette), Message::ClosePalette, 68.))
        } else if self.settings_open {
            Some((self.settings_view(), Message::CloseSettings, 74.))
        } else {
            None
        };
        if let Some((dialog, close, top)) = overlay_content {
            let backdrop = widget::opaque(
                mouse_area(
                    container(space().width(Length::Fill).height(Length::Fill))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .style(move |_| container::Style::default().background(c.overlay)),
                )
                .on_press(close),
            );
            let overlay = container(widget::opaque(dialog))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(alignment::Horizontal::Center)
                .padding(iced::Padding {
                    top: top / self.scale_factor(),
                    right: 20.,
                    bottom: 20.,
                    left: 20.,
                });
            widget::stack![base, backdrop, overlay].into()
        } else {
            base
        }
    }

    fn titlebar(&self, size: iced::Size) -> Element<'_, Message> {
        let c = self.settings.theme.colors();
        let compact = size.width < 820.;
        let window_inset = if cfg!(target_os = "macos") {
            76. / self.scale_factor()
        } else {
            8.
        };
        let side_width = if compact {
            (window_inset + 36.).max(80.)
        } else {
            180.
        };
        let search = button(
            row![
                icons::icon_tinted(Icon::Search, c.muted),
                text(if self.folder.is_some() {
                    self.project_name()
                } else {
                    "Search files and commands".into()
                })
                .size(12)
                .color(c.muted)
                .wrapping(widget::text::Wrapping::None)
                .width(Length::Fill),
                keycap(platform_key("P")),
            ]
            .spacing(10)
            .align_y(alignment::Vertical::Center),
        )
        .on_press(Message::OpenPalette(false))
        .padding([5, 10])
        .width(Length::Fill)
        .style(move |_, status| button::Style {
            background: Some(
                if status == button::Status::Hovered {
                    c.hover
                } else {
                    c.bg
                }
                .into(),
            ),
            text_color: c.ink,
            border: iced::Border {
                color: c.edge,
                width: 1.,
                radius: 6.0.into(),
            },
            ..Default::default()
        });
        let left = container(
            row![
                space().width(window_inset),
                icon_button("Toggle sidebar", Icon::Sidebar, Message::ToggleSidebar)
            ]
            .align_y(alignment::Vertical::Center),
        )
        .width(side_width);
        let mut actions = row![].spacing(5).align_y(alignment::Vertical::Center);
        if !compact {
            actions = actions
                .push(icon_button("New file", Icon::Plus, Message::New))
                .push(icon_button("Open file", Icon::Folder, Message::OpenFile))
                .push(icon_button("Save file", Icon::Save, Message::Save))
                .push(
                    container(space())
                        .width(1)
                        .height(16)
                        .style(move |_| container::Style::default().background(c.edge)),
                );
        }
        actions = actions
            .push(icon_button(
                "Toggle terminal",
                Icon::Panel,
                Message::ToggleTerminal,
            ))
            .push(icon_button(
                "Settings",
                Icon::Settings,
                Message::ToggleSettings,
            ));
        let right = container(actions)
            .width(side_width)
            .align_x(alignment::Horizontal::Right);
        mouse_area(
            container(
                row![
                    left,
                    container(
                        container(search)
                            .id("titlebar-search")
                            .max_width(460)
                            .width(Length::Fill)
                    )
                    .center_x(Length::Fill),
                    right,
                ]
                .spacing(8)
                .align_y(alignment::Vertical::Center),
            )
            .height(Length::Fill)
            .align_y(alignment::Vertical::Center)
            .padding([5, 8])
            .style(move |_| container::Style::default().background(c.panel)),
        )
        .on_press(Message::DragWindow)
        .into()
    }

    fn project_name(&self) -> String {
        self.folder
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Workspace".into())
    }

    fn editor_frame(&self) -> Element<'_, Message> {
        if self.settings.terminal {
            pane_grid::PaneGrid::new(&self.panes, |_, pane, _| {
                pane_grid::Content::new(match pane {
                    Pane::Editor => self.editor_view(),
                    Pane::Terminal => self.terminal_view(),
                })
            })
            .spacing(1)
            .on_resize(8, Message::Resized)
            .into()
        } else {
            self.editor_view()
        }
    }

    fn palette_view(&self, palette: &Palette) -> Element<'_, Message> {
        let c = self.settings.theme.colors();
        let items = palette.items(&self.file_index, self.folder.as_deref());
        let input = text_input(
            "Search files by name, or type > for commands",
            &palette.query,
        )
        .id("palette")
        .on_input(Message::PaletteQuery)
        .on_submit(Message::PaletteAccept)
        .size(14)
        .padding([13, 12])
        .style(input_style);
        let mut results = column![].spacing(2);
        for (index, item) in items.iter().enumerate() {
            let selected = index == palette.selected;
            let icon = match &item.action {
                palette::Action::File(path) => icons::file_icon(path),
                palette::Action::Command(_) => Icon::Command,
            };
            results = results.push(
                button(
                    row![
                        icons::icon_tinted(icon, if selected { c.accent } else { c.muted }),
                        text(item.title.clone())
                            .size(13)
                            .wrapping(widget::text::Wrapping::None)
                            .width(Length::FillPortion(3)),
                        text(item.detail.clone())
                            .size(11)
                            .color(c.muted)
                            .wrapping(widget::text::Wrapping::None)
                            .align_x(alignment::Horizontal::Right)
                            .width(Length::FillPortion(2)),
                    ]
                    .spacing(12)
                    .align_y(alignment::Vertical::Center),
                )
                .on_press(Message::PaletteExecute(index))
                .padding([10, 12])
                .width(Length::Fill)
                .style(move |_, status| button::Style {
                    background: if selected {
                        Some(c.selection.into())
                    } else if status == button::Status::Hovered {
                        Some(c.hover.into())
                    } else {
                        None
                    },
                    text_color: c.ink,
                    border: iced::Border {
                        radius: 5.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            );
        }
        if items.is_empty() {
            results = results.push(
                container(
                    column![
                        text(if palette.indexing {
                            "Indexing workspace…"
                        } else {
                            "No results found"
                        })
                        .size(13)
                        .color(c.ink),
                        text(if palette.indexing {
                            "Your files will appear here in a moment."
                        } else if self.folder.is_none() && !palette.commands() {
                            "Open a folder to search its files, or type > for commands."
                        } else {
                            "Try a different name or a shorter search."
                        })
                        .size(12)
                        .color(c.muted),
                    ]
                    .spacing(8),
                )
                .padding([22, 12]),
            );
        }
        let hint = if palette.commands() {
            "Commands"
        } else if self.file_index.limited {
            "File index limited · refine your search"
        } else {
            "Files"
        };
        container(column![
            container(
                row![
                    icons::icon_tinted(Icon::Search, c.muted),
                    input,
                    keycap("esc".into())
                ]
                .spacing(6)
                .align_y(alignment::Vertical::Center)
            )
            .padding([2, 10]),
            hairline(c.edge, false),
            scrollable(container(results).padding(6))
                .id("palette-results")
                .height(Length::Fill)
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new().width(4).scroller_width(3)
                )),
            hairline(c.edge, false),
            container(row![
                text(hint).size(11).color(c.muted),
                space::horizontal(),
                text("↑ ↓  navigate     ↵  select").size(10).color(c.muted)
            ])
            .padding([10, 14]),
        ])
        .width(620)
        .height((items.len().max(2) as f32 * 39. + 100.).min(490.))
        .style(dialog_style)
        .into()
    }

    fn settings_view(&self) -> Element<'_, Message> {
        let c = self.settings.theme.colors();
        let mut theme_rows = column![].spacing(8);
        let mut themes = row![].spacing(8);
        for (index, appearance) in crate::appearance::Appearance::ALL.into_iter().enumerate() {
            let sample = appearance.colors();
            let selected = appearance == self.settings.theme;
            let swatch = container(
                column![
                    container(space())
                        .height(5)
                        .width(36)
                        .style(move |_| container::Style::default().background(sample.accent)),
                    container(space())
                        .height(4)
                        .width(58)
                        .style(move |_| container::Style::default().background(sample.muted)),
                    container(space())
                        .height(4)
                        .width(46)
                        .style(move |_| container::Style::default().background(sample.edge)),
                ]
                .spacing(6),
            )
            .padding([14, 12])
            .width(Length::Fill)
            .height(62)
            .style(move |_| {
                container::Style::default()
                    .background(sample.bg)
                    .border(iced::Border {
                        color: sample.edge,
                        width: 1.,
                        radius: 4.0.into(),
                    })
            });
            themes = themes.push(
                button(
                    column![
                        swatch,
                        row![
                            text(appearance.to_string()).size(11).width(Length::Fill),
                            if selected {
                                icons::icon_tinted(Icon::Check, c.accent)
                            } else {
                                space().width(16).into()
                            }
                        ]
                        .align_y(alignment::Vertical::Center),
                    ]
                    .spacing(10),
                )
                .on_press(Message::ThemeSelected(appearance))
                .padding(9)
                .width(Length::Fill)
                .style(move |_, status| button::Style {
                    background: Some(
                        if status == button::Status::Hovered {
                            c.hover
                        } else {
                            c.panel
                        }
                        .into(),
                    ),
                    text_color: c.ink,
                    border: iced::Border {
                        color: if selected { c.accent } else { c.edge },
                        width: 1.,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                }),
            );
            if index == 2 {
                theme_rows = theme_rows.push(themes);
                themes = row![].spacing(8);
            }
        }
        let themes = theme_rows.push(themes.push(space().width(Length::Fill)));
        let zoom_controls = row![
            widget::pick_list(
                AppZoom::ALL,
                Some(self.settings.app_zoom),
                Message::AppZoomSelected
            )
            .text_size(12)
            .padding([7, 10])
            .style(move |_, status| widget::pick_list::Style {
                text_color: c.ink,
                placeholder_color: c.muted,
                handle_color: c.muted,
                background: if status == widget::pick_list::Status::Hovered {
                    c.hover
                } else {
                    c.bg
                }
                .into(),
                border: iced::Border {
                    color: c.edge,
                    width: 1.,
                    radius: 4.0.into()
                },
            })
            .menu_style(move |_| widget::overlay::menu::Style {
                text_color: c.ink,
                background: c.raised.into(),
                border: iced::Border {
                    color: c.edge,
                    width: 1.,
                    radius: 4.0.into()
                },
                selected_text_color: c.ink,
                selected_background: c.selection.into(),
                shadow: iced::Shadow::default(),
            }),
            tool("Reset", Message::AppZoomReset),
        ]
        .spacing(6)
        .align_y(alignment::Vertical::Center);
        let font_controls = row![
            icon_button("Decrease editor font size", Icon::Minus, Message::Zoom(-1.)),
            container(
                text(format!("{} px", self.settings.font_size as u32))
                    .size(12)
                    .color(c.ink)
            )
            .width(54)
            .align_x(alignment::Horizontal::Center),
            icon_button("Increase editor font size", Icon::Plus, Message::Zoom(1.)),
        ]
        .spacing(4)
        .align_y(alignment::Vertical::Center);
        let preferences = column![
            text("APPEARANCE").size(10).color(c.muted),
            setting_row(
                "App zoom",
                "Scale the entire interface. Use ⌘/Ctrl +, − or 0.",
                zoom_controls.into()
            ),
            hairline(c.edge, false),
            text("Color theme").size(13).color(c.ink),
            themes,
            space().height(10),
            hairline(c.edge, false),
            space().height(6),
            setting_row(
                "Editor font size",
                "Adjust code text independently. Use Option/Alt + or −.",
                font_controls.into()
            ),
            hairline(c.edge, false),
            setting_row(
                "Explorer",
                "Show the file tree beside your editor.",
                preference_toggle(self.settings.sidebar, Message::ToggleSidebar)
            ),
            hairline(c.edge, false),
            setting_row(
                "Terminal",
                "Show the integrated shell panel.",
                preference_toggle(self.settings.terminal, Message::ToggleTerminal)
            ),
        ]
        .spacing(10);
        container(column![
            container(
                row![
                    icons::icon_tinted(Icon::Settings, c.muted),
                    text("Settings").size(16).color(c.ink),
                    space::horizontal(),
                    icon_button("Close settings", Icon::Close, Message::CloseSettings),
                ]
                .spacing(10)
                .align_y(alignment::Vertical::Center)
            )
            .padding([14, 20]),
            hairline(c.edge, false),
            scrollable(container(preferences).padding(22)).height(Length::Fill),
            hairline(c.edge, false),
            container(row![
                text(format!("Kite / {}", env!("CARGO_PKG_VERSION")))
                    .font(iced::Font::MONOSPACE)
                    .size(11)
                    .color(c.muted),
                space::horizontal(),
                text("Changes apply immediately").size(11).color(c.muted)
            ])
            .padding([12, 22]),
        ])
        .width(610)
        .height(550)
        .style(dialog_style)
        .into()
    }

    fn welcome_view(&self, size: iced::Size) -> Element<'_, Message> {
        let c = self.settings.theme.colors();
        let compact = size.height < 450.;
        let identity: Element<'_, Message> = if compact {
            column![
                crate::brand::wordmark(160., self.settings.theme),
                text("A small space for serious work.")
                    .size(12)
                    .color(c.muted),
            ]
            .spacing(8)
            .into()
        } else {
            column![
                crate::brand::wordmark(230., self.settings.theme),
                space().height(8),
                text("A small space for serious work.")
                    .size(14)
                    .color(c.muted),
            ]
            .spacing(7)
            .into()
        };
        let mut welcome = column![
            identity,
            space().height(if compact { 8 } else { 28 }),
            welcome_action(
                Icon::Folder,
                "Open Folder…",
                platform_key("⇧ O"),
                Message::OpenFolder,
                true
            ),
            welcome_action(
                Icon::Document,
                "Open File…",
                platform_key("O"),
                Message::OpenFile,
                false
            ),
            welcome_action(
                Icon::Plus,
                "New File",
                platform_key("N"),
                Message::New,
                false
            ),
            welcome_action(
                Icon::Command,
                "Command Palette",
                platform_key("⇧ P"),
                Message::OpenPalette(true),
                false
            ),
        ]
        .spacing(7)
        .width((size.width - 32.).clamp(200., 350.));
        if !compact {
            welcome = welcome.push(space().height(22)).push(
                text("01  EDIT     02  RUN     03  MAKE")
                    .font(iced::Font::MONOSPACE)
                    .size(11)
                    .color(c.muted),
            );
        }
        if size.height < 300. {
            scrollable(container(welcome).padding(16).center_x(Length::Fill))
                .height(Length::Fill)
                .into()
        } else {
            container(welcome).center(Length::Fill).into()
        }
    }

    fn editor_view(&self) -> Element<'_, Message> {
        let c = self.settings.theme.colors();
        if self.tabs.is_empty() {
            return widget::responsive(move |size| self.welcome_view(size)).into();
        }
        let tab = &self.tabs[self.active];
        let mut tabs = row![].spacing(1);
        for (index, document) in self.tabs.iter().enumerate() {
            let selected = index == self.active;
            let file_icon = document
                .path
                .as_deref()
                .map(icons::file_icon)
                .unwrap_or(Icon::Document);
            let dirty: Element<'_, Message> = if document.editor.dirty() {
                text("●").size(8).color(c.accent).into()
            } else {
                space().width(0).into()
            };
            let label = row![
                icons::icon_tinted(file_icon, if selected { c.ink } else { c.muted }),
                text(document.name())
                    .size(12)
                    .wrapping(widget::text::Wrapping::None),
                dirty,
            ]
            .spacing(8)
            .align_y(alignment::Vertical::Center);
            let select = widget::tooltip(
                button(label)
                    .on_press(Message::Tab(index))
                    .style(quiet_button)
                    .padding([9, 12]),
                container(
                    text(
                        document
                            .path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "Unsaved file".into()),
                    )
                    .size(11),
                )
                .padding([6, 9])
                .style(tooltip_style),
                widget::tooltip::Position::Bottom,
            );
            tabs = tabs.push(
                container(column![
                    container(space())
                        .width(Length::Fill)
                        .height(2)
                        .style(
                            move |_| container::Style::default().background(if selected {
                                c.accent
                            } else {
                                c.panel
                            })
                        ),
                    row![
                        select,
                        icon_button("Close file", Icon::Close, Message::Close(document.id)),
                        space().width(5)
                    ]
                    .align_y(alignment::Vertical::Center),
                ])
                .style(move |_| {
                    container::Style::default().background(if selected { c.bg } else { c.panel })
                }),
            );
        }
        let tabs = scrollable(tabs).direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(2),
        ));
        let mut content = column![
            container(
                row![
                    tabs.width(Length::Fill),
                    icon_button("New file", Icon::Plus, Message::New),
                    space().width(8)
                ]
                .align_y(alignment::Vertical::Center)
            )
            .style(move |_| container::Style::default().background(c.panel)),
            hairline(c.edge, false)
        ];
        if self.find {
            let count = if self.query.is_empty() {
                String::new()
            } else {
                let matches = tab.editor.content.text().matches(&self.query).count();
                format!(
                    "{matches} {}",
                    if matches == 1 { "match" } else { "matches" }
                )
            };
            content = content.push(
                container(
                    column![
                        row![
                            icons::icon_tinted(Icon::Search, c.muted),
                            text_input("Find in file", &self.query)
                                .id("find")
                                .on_input(Message::Query)
                                .on_submit(Message::FindNext)
                                .padding([7, 9])
                                .size(12)
                                .style(input_style),
                            container(text(count).size(11).color(c.muted)).width(76),
                            tool("Find next", Message::FindNext),
                            icon_button("Close find and replace", Icon::Close, Message::HideFind),
                        ]
                        .spacing(8)
                        .align_y(alignment::Vertical::Center),
                        row![
                            space().width(16),
                            text_input("Replace with", &self.replacement)
                                .on_input(Message::Replacement)
                                .on_submit(Message::Replace)
                                .padding([7, 9])
                                .size(12)
                                .style(input_style),
                            tool("Replace", Message::Replace),
                            tool("Replace all", Message::ReplaceAll),
                            space().width(28),
                        ]
                        .spacing(8)
                        .align_y(alignment::Vertical::Center),
                    ]
                    .spacing(6),
                )
                .padding([10, 16])
                .style(move |_| container::Style::default().background(c.panel)),
            );
            content = content.push(hairline(c.edge, false));
        }
        let path = tab.path.as_ref().map(|p| {
            p.strip_prefix(self.folder.as_deref().unwrap_or(Path::new("")))
                .unwrap_or(p)
        });
        let mut breadcrumb = row![].spacing(5).align_y(alignment::Vertical::Center);
        if let Some(path) = path {
            let segments: Vec<_> = path.components().collect();
            let last = segments.len().saturating_sub(1);
            for (index, segment) in segments.iter().enumerate() {
                if index > 0 {
                    breadcrumb = breadcrumb.push(icons::icon_sized_tinted(
                        Icon::ChevronRight,
                        12.,
                        c.subtle,
                    ));
                }
                if index == last {
                    breadcrumb =
                        breadcrumb.push(icons::icon_tinted(icons::file_icon(path), c.muted));
                }
                breadcrumb = breadcrumb.push(
                    text(segment.as_os_str().to_string_lossy().into_owned())
                        .size(11)
                        .color(if index == last { c.ink } else { c.muted })
                        .wrapping(widget::text::Wrapping::None),
                );
            }
        } else {
            breadcrumb = breadcrumb.push(text("Untitled").size(11).color(c.muted));
        }
        let mut breadcrumb_row =
            row![container(breadcrumb).width(Length::Fill)].align_y(alignment::Vertical::Center);
        if tab
            .path
            .as_deref()
            .is_some_and(crate::preview::is_supported)
        {
            breadcrumb_row = breadcrumb_row.push(self.preview_mode_controls(tab.preview.mode));
        }
        content = content.push(container(breadcrumb_row).padding([6, 16]));
        let surface: Element<'_, Message> = match tab.preview.mode {
            DocumentMode::Code => self.code_view(tab),
            DocumentMode::Preview => self.document_preview(tab),
            DocumentMode::Split => row![
                container(self.code_view(tab)).width(Length::FillPortion(1)),
                hairline(c.edge, true),
                container(self.document_preview(tab)).width(Length::FillPortion(1))
            ]
            .height(Length::Fill)
            .into(),
        };
        content.push(surface).height(Length::Fill).into()
    }

    fn code_view<'a>(&'a self, tab: &'a Tab) -> Element<'a, Message> {
        let c = self.settings.theme.colors();
        let editor = text_editor(&tab.editor.content)
            .id("editor")
            .cursor_blink(false)
            .on_action(Message::Edit)
            .font(iced::Font::MONOSPACE)
            .size(self.settings.font_size)
            .line_height(1.55)
            .padding(iced::Padding {
                top: 8.,
                right: 16.,
                bottom: 8.,
                left: 0.,
            })
            .height(Length::Fill)
            .wrapping(iced::widget::text::Wrapping::None)
            .key_binding(|press| {
                if !matches!(press.status, text_editor::Status::Focused { .. }) {
                    return None;
                }
                if let Some(message) = shortcut(&press.key, press.modifiers) {
                    return Some(text_editor::Binding::Custom(message));
                }
                if matches!(
                    press.key,
                    keyboard::Key::Named(keyboard::key::Named::Escape)
                ) {
                    return Some(text_editor::Binding::Custom(Message::HideFind));
                }
                text_editor::Binding::from_key_press(press)
            })
            .highlight(tab.syntax(), self.settings.theme.syntax())
            .style(move |_, _| text_editor::Style {
                background: c.bg.into(),
                border: iced::Border::default(),
                placeholder: c.muted,
                value: c.ink,
                selection: c.selection,
            });
        row![
            crate::gutter::gutter(&tab.editor.content, self.settings.font_size),
            editor
        ]
        .height(Length::Fill)
        .into()
    }

    fn terminal_view(&self) -> Element<'_, Message> {
        let c = self.settings.theme.colors();
        let mut tabs = row![].spacing(3).align_y(alignment::Vertical::Center);
        for (index, session) in self.terminal.sessions.iter().enumerate() {
            let selected = index == self.terminal.active;
            let name: String = session.title().chars().take(22).collect();
            let label = format!(
                "{}{}",
                name,
                if session.error().is_some() {
                    " · error"
                } else if session.is_running() {
                    ""
                } else {
                    " · exited"
                }
            );
            tabs = tabs.push(
                container(
                    row![
                        button(
                            row![
                                icons::icon_tinted(
                                    Icon::Terminal,
                                    if selected { c.ink } else { c.muted }
                                ),
                                text(label)
                                    .size(11)
                                    .color(if selected { c.ink } else { c.muted })
                            ]
                            .spacing(7)
                            .align_y(alignment::Vertical::Center)
                        )
                        .on_press(Message::SelectTerminal(index))
                        .style(quiet_button)
                        .padding([5, 8]),
                        icon_button(
                            "Close terminal session",
                            Icon::Close,
                            Message::CloseTerminal(index)
                        ),
                    ]
                    .align_y(alignment::Vertical::Center),
                )
                .style(move |_| {
                    container::Style::default()
                        .background(if selected { c.raised } else { c.panel })
                        .border(iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        })
                }),
            );
        }
        let header = container(
            row![
                text("TERMINAL").size(10).color(c.muted),
                scrollable(tabs)
                    .direction(scrollable::Direction::Horizontal(
                        scrollable::Scrollbar::new().width(2)
                    ))
                    .width(Length::Fill),
                icon_button("New terminal", Icon::Plus, Message::NewTerminal),
                icon_button(
                    "Restart terminal in workspace",
                    Icon::Refresh,
                    Message::RestartTerminal
                ),
                icon_button(
                    "Hide terminal panel",
                    Icon::ChevronDown,
                    Message::ToggleTerminal
                ),
            ]
            .spacing(12)
            .align_y(alignment::Vertical::Center),
        )
        .padding([5, 14])
        .height(38)
        .style(move |_| container::Style::default().background(c.panel));
        column![
            hairline(c.edge, false),
            header,
            container(self.terminal.view().map(Message::Terminal))
                .padding([8, 16])
                .height(Length::Fill)
        ]
        .height(Length::Fill)
        .into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let c = self.settings.theme.colors();
        let mut entries = column![].spacing(0);
        if self.folder.is_some() {
            for entry in &self.tree {
                let selected =
                    self.tabs.get(self.active).and_then(|t| t.path.as_ref()) == Some(&entry.path);
                let disclosure: Element<'_, Message> = if entry.directory {
                    icons::icon_sized_tinted(
                        if self.expanded.contains(&entry.path) {
                            Icon::ChevronDown
                        } else {
                            Icon::ChevronRight
                        },
                        12.,
                        c.muted,
                    )
                } else {
                    space().width(12).into()
                };
                let item = button(
                    row![
                        disclosure,
                        icons::icon_tinted(
                            if entry.directory {
                                Icon::Folder
                            } else {
                                icons::file_icon(&entry.path)
                            },
                            if selected { c.ink } else { c.muted }
                        ),
                        text(&entry.name)
                            .size(12)
                            .wrapping(widget::text::Wrapping::None),
                    ]
                    .spacing(6)
                    .align_y(alignment::Vertical::Center),
                )
                .on_press(if entry.directory {
                    Message::Expand(entry.path.clone())
                } else {
                    Message::OpenPath(entry.path.clone())
                })
                .width(Length::Fill)
                .padding(iced::Padding {
                    top: 4.,
                    right: 8.,
                    bottom: 4.,
                    left: 8. + entry.depth as f32 * 14.,
                })
                .style(move |_, status| button::Style {
                    background: if selected {
                        Some(c.selection.into())
                    } else if status == button::Status::Hovered {
                        Some(c.hover.into())
                    } else {
                        None
                    },
                    text_color: if selected { c.ink } else { c.muted },
                    border: iced::Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                });
                entries = entries.push(item);
            }
            if self.tree.is_empty() {
                entries = entries.push(
                    container(text("This folder is empty").size(12).color(c.muted)).padding(14),
                );
            }
        } else {
            entries = entries.push(
                container(
                    column![
                        text("No folder open").size(13).color(c.ink),
                        text("Open a project folder to explore your files.")
                            .size(12)
                            .color(c.muted),
                        button(
                            row![
                                icons::icon_tinted(Icon::Folder, c.accent_ink),
                                text("Open Folder").size(12)
                            ]
                            .spacing(8)
                            .align_y(alignment::Vertical::Center)
                        )
                        .on_press(Message::OpenFolder)
                        .padding([8, 12])
                        .style(primary_button),
                    ]
                    .spacing(14),
                )
                .padding([16, 12]),
            );
        }
        let header = container(widget::responsive(move |size| {
            let mut actions = row![].spacing(1).align_y(alignment::Vertical::Center);
            if size.width >= 135. {
                actions = actions.push(text("EXPLORER").size(10).color(c.muted));
            }
            actions = actions
                .push(space::horizontal())
                .push(icon_button("New file", Icon::Plus, Message::New))
                .push(icon_button(
                    "Open folder",
                    Icon::Folder,
                    Message::OpenFolder,
                ));
            if size.width > 210. {
                actions = actions
                    .push(icon_button(
                        "Refresh files",
                        Icon::Refresh,
                        Message::RefreshTree,
                    ))
                    .push(icon_button(
                        "Collapse folders",
                        Icon::Minus,
                        Message::CollapseTree,
                    ));
            }
            container(actions).padding([6, 10]).into()
        }))
        .height(38);
        let mut content = column![header];
        if self.folder.is_some() {
            content = content.push(
                container(
                    row![
                        icons::icon_sized_tinted(Icon::ChevronDown, 12., c.muted),
                        text(self.project_name())
                            .size(11)
                            .color(c.ink)
                            .wrapping(widget::text::Wrapping::None)
                    ]
                    .spacing(7)
                    .align_y(alignment::Vertical::Center),
                )
                .padding([9, 12]),
            );
        }
        container(content.push(scrollable(container(entries).padding([0, 6])).height(Length::Fill)))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style::default().background(c.panel))
            .into()
    }

    fn statusbar(&self) -> Element<'_, Message> {
        let c = self.settings.theme.colors();
        let mut status = row![
            icons::icon_sized_tinted(Icon::Check, 12., c.muted),
            text(&self.status)
                .size(10)
                .color(c.muted)
                .wrapping(widget::text::Wrapping::None),
            space::horizontal()
        ]
        .spacing(14)
        .align_y(alignment::Vertical::Center);
        if let Some(tab) = self.tabs.get(self.active) {
            let cursor = tab.editor.content.cursor().position;
            let column = tab
                .editor
                .content
                .line(cursor.line)
                .map(|line| {
                    line.text
                        .get(..cursor.column)
                        .unwrap_or(&line.text)
                        .chars()
                        .count()
                        + 1
                })
                .unwrap_or(1);
            status = status
                .push(
                    text(format!("Ln {}, Col {}", cursor.line + 1, column))
                        .size(10)
                        .color(c.muted),
                )
                .push(
                    text(if tab.bom { "UTF-8 BOM" } else { "UTF-8" })
                        .size(10)
                        .color(c.muted),
                )
                .push(text(tab.line_ending.label()).size(10).color(c.muted))
                .push(text(language_name(tab.syntax())).size(10).color(c.muted));
        }
        status = status.push(
            button(text(self.settings.app_zoom.to_string()).size(10))
                .on_press(Message::ToggleSettings)
                .padding([0, 3])
                .style(quiet_button),
        );
        container(status)
            .padding([3, 12])
            .height(24)
            .style(move |_| container::Style::default().background(c.panel))
            .into()
    }
}

fn blend(base: Color, tint: Color, amount: f32) -> Color {
    Color::from_rgb(
        base.r + (tint.r - base.r) * amount,
        base.g + (tint.g - base.g) * amount,
        base.b + (tint.b - base.b) * amount,
    )
}
fn ui_colors(theme: &Theme) -> crate::appearance::Colors {
    crate::appearance::theme_colors(theme)
}
fn hairline<'a>(color: Color, vertical: bool) -> Element<'a, Message> {
    container(if vertical {
        space().width(1).height(Length::Fill)
    } else {
        space().width(Length::Fill).height(1)
    })
    .style(move |_| container::Style::default().background(color))
    .into()
}
fn quiet_button(theme: &Theme, status: button::Status) -> button::Style {
    let c = ui_colors(theme);
    button::Style {
        background: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            Some(c.hover.into())
        } else {
            None
        },
        text_color: if status == button::Status::Disabled {
            c.subtle
        } else {
            c.ink
        },
        border: iced::Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
fn primary_button(theme: &Theme, status: button::Status) -> button::Style {
    let c = ui_colors(theme);
    button::Style {
        background: Some(
            if status == button::Status::Hovered {
                blend(c.accent, c.ink, 0.12)
            } else {
                c.accent
            }
            .into(),
        ),
        text_color: c.accent_ink,
        border: iced::Border {
            radius: 5.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
fn input_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let c = ui_colors(theme);
    text_input::Style {
        background: c.bg.into(),
        border: iced::Border {
            color: if matches!(status, text_input::Status::Focused { .. }) {
                c.accent
            } else {
                c.edge
            },
            width: 1.,
            radius: 4.0.into(),
        },
        icon: c.muted,
        placeholder: c.subtle,
        value: c.ink,
        selection: c.selection,
    }
}
fn dialog_style(theme: &Theme) -> container::Style {
    let c = ui_colors(theme);
    container::Style::default()
        .background(c.raised)
        .color(c.ink)
        .border(iced::Border {
            color: c.edge,
            width: 1.,
            radius: 9.0.into(),
        })
        .shadow(iced::Shadow {
            color: Color::from_rgba(0., 0., 0., 0.4),
            offset: iced::Vector::new(0., 12.),
            blur_radius: 36.,
        })
}
fn tooltip_style(theme: &Theme) -> container::Style {
    let c = ui_colors(theme);
    container::Style::default()
        .background(c.raised)
        .color(c.ink)
        .border(iced::Border {
            color: c.edge,
            width: 1.,
            radius: 4.0.into(),
        })
}
fn platform_key(key: &str) -> String {
    format!(
        "{} {key}",
        if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Ctrl"
        }
    )
}
fn keycap(shortcut: String) -> Element<'static, Message> {
    container(
        text(shortcut)
            .size(10)
            .wrapping(widget::text::Wrapping::None),
    )
    .padding([2, 5])
    .style(|theme: &Theme| {
        let c = ui_colors(theme);
        container::Style::default()
            .background(c.panel)
            .color(c.muted)
            .border(iced::Border {
                color: c.edge,
                width: 1.,
                radius: 3.0.into(),
            })
    })
    .into()
}
fn tool<'a>(label: &'a str, message: Message) -> iced::widget::Button<'a, Message> {
    button(text(label).size(11))
        .on_press(message)
        .style(quiet_button)
        .padding([6, 9])
}
fn icon_button<'a>(label: &'static str, icon: Icon, message: Message) -> Element<'a, Message> {
    widget::tooltip(
        button(icons::icon(icon))
            .on_press(message)
            .padding(5)
            .style(quiet_button),
        container(text(label).size(11))
            .padding([6, 9])
            .style(tooltip_style),
        widget::tooltip::Position::Bottom,
    )
    .delay(iced::time::Duration::from_millis(450))
    .snap_within_viewport(true)
    .into()
}
fn activity_button<'a>(
    label: &'static str,
    icon: Icon,
    message: Message,
    selected: bool,
) -> Element<'a, Message> {
    widget::tooltip(
        button(icons::icon_sized(icon, 20.))
            .on_press(message)
            .padding(7)
            .style(move |theme, status| {
                let c = ui_colors(theme);
                button::Style {
                    background: if selected {
                        Some(c.selection.into())
                    } else if status == button::Status::Hovered {
                        Some(c.hover.into())
                    } else {
                        None
                    },
                    text_color: if selected { c.ink } else { c.muted },
                    border: iced::Border {
                        radius: 5.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }),
        container(text(label).size(11))
            .padding([6, 9])
            .style(tooltip_style),
        widget::tooltip::Position::Right,
    )
    .delay(iced::time::Duration::from_millis(450))
    .snap_within_viewport(true)
    .into()
}
fn welcome_action(
    icon: Icon,
    label: &'static str,
    shortcut: String,
    message: Message,
    primary: bool,
) -> Element<'static, Message> {
    button(
        row![
            icons::icon(icon),
            text(label).size(13),
            space::horizontal(),
            keycap(shortcut)
        ]
        .spacing(10)
        .align_y(alignment::Vertical::Center),
    )
    .on_press(message)
    .width(Length::Fill)
    .padding([11, 12])
    .style(move |theme, status| {
        let c = ui_colors(theme);
        button::Style {
            background: Some(
                if status == button::Status::Hovered {
                    c.hover
                } else if primary {
                    c.raised
                } else {
                    c.bg
                }
                .into(),
            ),
            text_color: c.ink,
            border: iced::Border {
                color: if primary { c.accent } else { c.bg },
                width: 1.,
                radius: 5.0.into(),
            },
            ..Default::default()
        }
    })
    .into()
}
fn setting_row<'a>(
    title: &'a str,
    detail: &'a str,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    row![
        column![
            text(title).size(13),
            text(detail).size(11).style(|theme: &Theme| text::Style {
                color: Some(ui_colors(theme).muted)
            })
        ]
        .spacing(5)
        .width(Length::Fill),
        control
    ]
    .spacing(16)
    .padding([7, 0])
    .align_y(alignment::Vertical::Center)
    .into()
}
fn preference_toggle(enabled: bool, message: Message) -> Element<'static, Message> {
    button(
        row![
            icons::icon(if enabled { Icon::Check } else { Icon::Minus }),
            text(if enabled { "Shown" } else { "Hidden" }).size(11)
        ]
        .spacing(7)
        .align_y(alignment::Vertical::Center),
    )
    .on_press(message)
    .padding([7, 10])
    .width(88)
    .style(move |theme, status| {
        let c = ui_colors(theme);
        button::Style {
            background: Some(
                if status == button::Status::Hovered {
                    c.hover
                } else if enabled {
                    c.selection
                } else {
                    c.panel
                }
                .into(),
            ),
            text_color: if enabled { c.accent } else { c.muted },
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .into()
}
fn language_name(extension: &str) -> &str {
    match extension {
        "rs" => "Rust",
        "ts" => "TypeScript",
        "tsx" => "TypeScript React",
        "js" => "JavaScript",
        "jsx" => "JavaScript React",
        "py" => "Python",
        "go" => "Go",
        "json" => "JSON",
        "toml" => "TOML",
        "md" | "markdown" | "mdown" => "Markdown",
        "mmd" | "mermaid" => "Mermaid",
        "txt" => "Plain Text",
        "html" => "HTML",
        "css" => "CSS",
        "yaml" | "yml" => "YAML",
        "sh" => "Shell",
        "swift" => "Swift",
        "c" => "C",
        "cpp" => "C++",
        other => other,
    }
}
fn command_message(command: palette::Command) -> Message {
    use palette::Command::*;
    match command {
        New => Message::New,
        OpenFile => Message::OpenFile,
        OpenFolder => Message::OpenFolder,
        Save => Message::Save,
        SaveAs => Message::SaveAs,
        Close => Message::CloseCurrent,
        Find => Message::ToggleFind,
        Sidebar => Message::ToggleSidebar,
        Terminal => Message::ToggleTerminal,
        Restart => Message::RestartTerminal,
        NewTerminal => Message::NewTerminal,
        FocusTerminal => Message::FocusTerminal,
        FocusEditor => Message::FocusEditor,
        ZoomIn => Message::Zoom(1.),
        ZoomOut => Message::Zoom(-1.),
        Refresh => Message::RefreshTree,
        Preferences => Message::ToggleSettings,
        Preview => Message::TogglePreview,
        AppZoomIn => Message::AppZoom(1),
        AppZoomOut => Message::AppZoom(-1),
        AppZoomReset => Message::AppZoomReset,
    }
}
fn focus_editor() -> Task<Message> {
    widget::operation::focus("editor")
}
fn load_file(path: PathBuf) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || document::load(&path))
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
        },
        Message::Loaded,
    )
}
fn read_tree(path: PathBuf, depth: usize, generation: u64) -> Task<Message> {
    Task::perform(
        async move {
            let output = path.clone();
            let result =
                tokio::task::spawn_blocking(move || workspace::read_directory(&path, depth))
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()));
            (output, depth, generation, result)
        },
        |(path, depth, generation, result)| Message::TreeRead(path, depth, generation, result),
    )
}
fn shortcut(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Message> {
    use keyboard::{Key, key::Named};
    if matches!(key, Key::Character(c) if c == "`") && modifiers.control() {
        return Some(Message::ToggleTerminal);
    }
    if matches!(key, Key::Named(Named::Tab)) && modifiers.control() {
        return Some(Message::NextTab(modifiers.shift()));
    }
    if matches!(key, Key::Named(Named::F6)) {
        return Some(Message::FocusEditor);
    }
    if matches!(key, Key::Named(Named::F1)) {
        return Some(Message::OpenPalette(true));
    }
    if modifiers.alt() && !modifiers.command() {
        match key.as_ref() {
            Key::Character("+") | Key::Character("=") => return Some(Message::Zoom(1.)),
            Key::Character("-") | Key::Character("−") => return Some(Message::Zoom(-1.)),
            _ => {}
        }
    }
    if !modifiers.command() {
        return None;
    }
    match key.as_ref() {
        Key::Character("n") if modifiers.shift() => Some(Message::NewTerminal),
        Key::Character("n") => Some(Message::New),
        Key::Character("p") => Some(Message::OpenPalette(modifiers.shift())),
        Key::Character("o") if modifiers.shift() => Some(Message::OpenFolder),
        Key::Character("o") => Some(Message::OpenFile),
        Key::Character("s") if modifiers.shift() => Some(Message::SaveAs),
        Key::Character("s") => Some(Message::Save),
        Key::Character("v") if modifiers.shift() => Some(Message::TogglePreview),
        Key::Character("f") => Some(Message::ToggleFind),
        Key::Character("b") => Some(Message::ToggleSidebar),
        Key::Character("j") => Some(Message::ToggleTerminal),
        Key::Character("z") if modifiers.shift() => Some(Message::Redo),
        Key::Character("z") => Some(Message::Undo),
        Key::Character("y") => Some(Message::Redo),
        Key::Character("+") | Key::Character("=") => Some(Message::AppZoom(1)),
        Key::Character("-") => Some(Message::AppZoom(-1)),
        Key::Character("0") => Some(Message::AppZoomReset),
        Key::Character("q") => Some(Message::Quit),
        Key::Character("w") => Some(Message::CloseCurrent),
        Key::Character(",") => Some(Message::ToggleSettings),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn app_zoom_is_independent_of_editor_font_and_can_reset_from_settings() {
        let mut app = fixture(PathBuf::from("test.txt"));
        let font = app.settings.font_size;
        let _ = app.update(Message::AppZoom(2));
        assert_eq!(app.settings.app_zoom.percent(), 125);
        assert_eq!(app.scale_factor(), 1.25);
        assert_eq!(app.settings.font_size, font);
        let _ = app.update(Message::Zoom(1.));
        assert_eq!(app.settings.font_size, font + 1.);
        assert_eq!(app.settings.app_zoom.percent(), 125);
        let _ = app.update(Message::ToggleSettings);
        let _ = app.update(Message::Key(
            keyboard::Key::Character("0".into()),
            keyboard::Modifiers::COMMAND,
        ));
        assert_eq!(app.scale_factor(), 1.);
        assert_eq!(app.settings.font_size, font + 1.);
    }

    #[test]
    fn titlebar_search_is_centered_at_each_app_zoom_and_window_size() {
        use iced::advanced::widget::{Id, Operation};
        #[derive(Default)]
        struct Bounds(Option<iced::Rectangle>);
        impl Operation for Bounds {
            fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
                visit(self);
            }
            fn container(&mut self, id: Option<&Id>, bounds: iced::Rectangle) {
                if id == Some(&Id::new("titlebar-search")) {
                    self.0 = Some(bounds);
                }
            }
        }
        for size in [iced::Size::new(800., 520.), iced::Size::new(1280., 800.)] {
            for zoom in [AppZoom::ALL[0], AppZoom::default(), AppZoom::ALL[6]] {
                let mut app = fixture(PathBuf::from("test.txt"));
                app.settings.app_zoom = zoom;
                let scale = app.scale_factor();
                let logical = iced::Size::new(size.width / scale, size.height / scale);
                let mut renderer =
                    iced_tiny_skia::Renderer::new(iced::Font::default(), 14.0.into());
                let mut interface = iced_runtime::UserInterface::build(
                    app.view(),
                    logical,
                    iced_runtime::user_interface::Cache::new(),
                    &mut renderer,
                );
                let mut operation = Bounds::default();
                interface.operate(&renderer, &mut operation);
                let bounds = operation.0.expect("search field has native bounds");
                assert!(
                    ((bounds.x + bounds.width / 2.) * scale - size.width / 2.).abs() <= 1.,
                    "{size:?}, {zoom}: {bounds:?}"
                );
                let header_height = 44. / scale.min(1.);
                assert!((bounds.y + bounds.height / 2. - header_height / 2.).abs() <= 1.);
            }
        }
    }

    #[test]
    fn preview_worker_updates_the_native_cache_after_debounced_edits() {
        use iced::futures::StreamExt;
        async fn finish(app: &mut App, task: Task<Message>) {
            let mut pending = vec![task];
            while let Some(task) = pending.pop() {
                if let Some(mut actions) = iced_runtime::task::into_stream(task) {
                    while let Some(action) = actions.next().await {
                        if let iced_runtime::Action::Output(message) = action {
                            pending.push(app.update(message));
                        }
                    }
                }
            }
        }
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut app = fixture(PathBuf::from("README.md"));
                app.tabs[0].editor =
                    Editor::new("# Preview\n\n```mermaid\nflowchart LR\nA-->B\n```\n");
                let task = app.update(Message::DocumentMode(DocumentMode::Split));
                finish(&mut app, task).await;
                assert!(app.tabs[0].preview.rendered.is_some());
                assert!(!app.tabs[0].preview.loading);
                let previous = app.tabs[0].preview.key.clone().unwrap();
                let task = app.update(Message::Edit(text_editor::Action::Edit(
                    text_editor::Edit::Insert('x'),
                )));
                assert!(app.tabs[0].preview.loading);
                finish(&mut app, task).await;
                assert!(!app.tabs[0].preview.loading);
                assert!(app.tabs[0].preview.error.is_none());
                assert!(app.tabs[0].preview.key.as_ref().unwrap().version > previous.version);
                assert_eq!(
                    app.tabs[0].preview.key.as_ref().unwrap().version,
                    app.tabs[0].editor.version
                );
            });
    }

    #[test]
    fn preview_can_resume_after_canceling_a_pending_debounce() {
        let mut app = fixture(PathBuf::from("README.md"));
        let _ = app.update(Message::DocumentMode(DocumentMode::Split));
        let old = app.tabs[0].preview.generation;
        assert!(app.tabs[0].preview.loading);
        let _ = app.update(Message::DocumentMode(DocumentMode::Code));
        assert_eq!(app.update(Message::PreviewTick(1, old)).units(), 0);
        let _ = app.update(Message::DocumentMode(DocumentMode::Preview));
        assert!(app.tabs[0].preview.generation > old);
        assert!(app.tabs[0].preview.loading);
        assert!(
            app.update(Message::PreviewTick(1, app.tabs[0].preview.generation))
                .units()
                > 0
        );
    }

    #[test]
    fn preview_rejects_old_revisions_and_caches_background_theme_results() {
        let mut app = fixture(PathBuf::from("README.md"));
        let _ = app.update(Message::DocumentMode(DocumentMode::Split));
        let generation = app.tabs[0].preview.generation;
        let key = app.tabs[0].preview.key.clone().unwrap();
        let result = Arc::new(crate::preview::prepare("# Original", "md", key.appearance));
        let _ = app.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Insert('x'),
        )));
        let _ = app.update(Message::PreviewReady(1, generation, key, Ok(result)));
        assert!(app.tabs[0].preview.rendered.is_none());
        let generation = app.tabs[0].preview.generation;
        let key = app.tabs[0].preview.key.clone().unwrap();
        let result = Arc::new(crate::preview::prepare("# Current", "md", key.appearance));
        let _ = app.update(Message::New);
        let _ = app.update(Message::ThemeSelected(
            crate::appearance::Appearance::Porcelain,
        ));
        let _ = app.update(Message::PreviewReady(1, generation, key, Ok(result)));
        assert!(app.tabs[0].preview.rendered.is_some());
        assert!(!app.tabs[0].preview.loading);
        let _ = app.update(Message::Tab(0));
        assert!(app.tabs[0].preview.loading);
        assert!(app.tabs[0].preview.generation > generation);
    }

    #[test]
    fn unsupported_files_keep_editor_focus_and_save_as_returns_to_code() {
        let mut app = fixture(PathBuf::from("main.rs"));
        assert_eq!(app.update(Message::TogglePreview).units(), 0);
        assert_eq!(app.tabs[0].preview.mode, DocumentMode::Code);
        app.tabs[0].path = Some(PathBuf::from("README.md"));
        let _ = app.update(Message::DocumentMode(DocumentMode::Preview));
        let _ = app.update(Message::Saved(
            1,
            0,
            PathBuf::from("notes.txt"),
            Ok(blake3::hash(b"notes")),
        ));
        assert_eq!(app.tabs[0].preview.mode, DocumentMode::Code);
        assert!(!app.tabs[0].preview.loading);
    }

    #[test]
    fn overlays_keep_focus_during_background_loads_and_panel_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loaded.txt");
        std::fs::write(&path, "loaded in background").unwrap();
        let mut app = fixture(PathBuf::from("test.txt"));
        let _ = app.update(Message::ToggleSettings);
        assert!(app.settings_open);
        assert_eq!(app.update(Message::ToggleTerminal).units(), 0);
        assert_eq!(app.update(Message::ToggleTerminal).units(), 0);
        assert_eq!(
            app.update(Message::Loaded(document::load(&path))).units(),
            0
        );
        let before = app.tabs[app.active].editor.content.text();
        let _ = app.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Insert('x'),
        )));
        assert_eq!(app.tabs[app.active].editor.content.text(), before);
        let _ = app.update(Message::Key(
            keyboard::Key::Named(keyboard::key::Named::Escape),
            keyboard::Modifiers::NONE,
        ));
        assert!(!app.settings_open);
        let _ = app.update(Message::OpenPalette(true));
        assert_eq!(
            app.update(Message::Loaded(document::load(&path))).units(),
            0
        );
        assert!(app.palette.is_some());
    }

    #[test]
    fn palette_keeps_global_shortcuts_from_focusing_the_workspace() {
        let mut app = fixture(PathBuf::from("test.txt"));
        let _ = app.update(Message::OpenPalette(true));
        assert_eq!(
            app.update(Message::Key(
                keyboard::Key::Named(keyboard::key::Named::F6),
                keyboard::Modifiers::NONE
            ))
            .units(),
            0
        );
        let tabs = app.tabs.len();
        let _ = app.update(Message::Key(
            keyboard::Key::Character("n".into()),
            keyboard::Modifiers::COMMAND,
        ));
        assert_eq!(app.tabs.len(), tabs);
        assert!(app.palette.is_some());
        let _ = app.update(Message::Key(
            keyboard::Key::Named(keyboard::key::Named::Escape),
            keyboard::Modifiers::NONE,
        ));
        assert!(app.palette.is_none());
        let _ = app.update(Message::Key(
            keyboard::Key::Character(",".into()),
            keyboard::Modifiers::COMMAND,
        ));
        assert!(app.settings_open);
    }

    #[test]
    fn focused_editor_does_not_schedule_idle_repaints() {
        let app = fixture(PathBuf::from("test.txt"));
        let mut renderer = iced_tiny_skia::Renderer::new(iced::Font::default(), 14.0.into());
        let mut interface = iced_runtime::UserInterface::build(
            app.view(),
            iced::Size::new(1280., 800.),
            iced_runtime::user_interface::Cache::new(),
            &mut renderer,
        );
        interface.operate(
            &renderer,
            &mut iced::advanced::widget::operation::focusable::focus::<()>(
                iced::advanced::widget::Id::new("editor"),
            ),
        );
        let now = iced::time::Instant::now();
        for offset in [0, 500, 1000, 2500] {
            let (state, _) = interface.update(
                &[iced::Event::Window(window::Event::RedrawRequested(
                    now + std::time::Duration::from_millis(offset),
                ))],
                iced::mouse::Cursor::Unavailable,
                &mut renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Vec::new(),
            );
            assert!(matches!(
                state,
                iced_runtime::user_interface::State::Updated {
                    redraw_request: window::RedrawRequest::Wait,
                    ..
                }
            ));
        }
    }
    fn render(app: &mut App, path: &str) {
        render_size(app, path, iced::Size::new(1280., 800.));
    }
    fn render_size(app: &mut App, path: &str, size: iced::Size) {
        let zoom = app.scale_factor();
        let logical = iced::Size::new(size.width / zoom, size.height / zoom);
        let mut renderer = iced_tiny_skia::Renderer::new(iced::Font::default(), 14.0.into());
        let mut interface = iced_runtime::UserInterface::build(
            app.view(),
            logical,
            iced_runtime::user_interface::Cache::new(),
            &mut renderer,
        );
        // Deliver layout events just as the live runtime does, including PTY resize.
        let mut messages = Vec::new();
        let _ = interface.update(
            &[iced::Event::Window(window::Event::RedrawRequested(
                iced::time::Instant::now(),
            ))],
            iced::mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        drop(interface);
        for message in messages {
            let _ = app.update(message);
        }
        let mut interface = iced_runtime::UserInterface::build(
            app.view(),
            logical,
            iced_runtime::user_interface::Cache::new(),
            &mut renderer,
        );
        interface.draw(
            &mut renderer,
            &app.theme(),
            &iced::advanced::renderer::Style {
                text_color: app.settings.theme.colors().ink,
            },
            iced::mouse::Cursor::Unavailable,
        );
        let mut pixels =
            tiny_skia::Pixmap::new((size.width * 2.) as u32, (size.height * 2.) as u32).unwrap();
        let mut mask =
            tiny_skia::Mask::new((size.width * 2.) as u32, (size.height * 2.) as u32).unwrap();
        let viewport = iced::advanced::graphics::Viewport::with_physical_size(
            iced::Size::new((size.width * 2.) as u32, (size.height * 2.) as u32),
            2.0 * zoom,
        );
        renderer.draw(
            &mut pixels.as_mut(),
            &mut mask,
            &viewport,
            &[iced::Rectangle::with_size(logical)],
            app.settings.theme.colors().bg,
        );
        // The native renderer writes BGRA; PNG expects RGBA.
        for pixel in pixels.data_mut().chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        pixels.save_png(path).unwrap();
    }

    #[test]
    fn unwrapped_source_cannot_paint_into_the_adjacent_preview() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = fixture(PathBuf::from("clip.md"));
        app.settings.sidebar = false;
        app.settings.terminal = false;
        app.tabs[0].preview.mode = DocumentMode::Split;
        app.tabs[0].preview.rendered = Some(crate::preview::Preview::new(crate::preview::prepare(
            "# Preview\n\nThe rendered document stays isolated from the source.",
            "md",
            app.settings.theme,
        )));
        app.tabs[0].editor = Editor::new(&format!("{}\n", "Unwrapped source text ".repeat(80)));
        let actual_path = dir.path().join("long.png");
        let reference_path = dir.path().join("short.png");
        let size = iced::Size::new(1280., 520.);
        render_size(&mut app, actual_path.to_str().unwrap(), size);
        app.tabs[0].editor = Editor::new("Short source\n");
        render_size(&mut app, reference_path.to_str().unwrap(), size);
        let actual = tiny_skia::Pixmap::load_png(actual_path).unwrap();
        let reference = tiny_skia::Pixmap::load_png(reference_path).unwrap();
        for y in 240..900 {
            for x in 1450..2500 {
                assert_eq!(
                    actual.pixel(x, y),
                    reference.pixel(x, y),
                    "source text escaped into preview at {x}, {y}"
                );
            }
        }
    }

    #[test]
    #[ignore = "renders native widgets to preview artifacts without a desktop session"]
    fn render_native_preview() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("kite");
        std::fs::create_dir_all(root.join("src")).unwrap();
        let source = "use std::time::Instant;\n\nfn main() {\n    let started = Instant::now();\n    let message = \"Hello, kite\";\n\n    println!(\"{message}\");\n    println!(\"Ready in {:?}\", started.elapsed());\n}\n";
        std::fs::write(root.join("src/main.rs"), source).unwrap();
        std::fs::write(root.join("Cargo.toml"), "").unwrap();
        std::fs::write(root.join("README.md"), "").unwrap();
        let mut app = fixture(root.join("src/main.rs"));
        app.folder = Some(root.clone());
        app.tabs[0].editor = Editor::new(source);
        app.status = "Ready".into();
        app.tree = workspace::read_directory(&root, 0).unwrap();
        app.tree.splice(
            1..1,
            workspace::read_directory(&root.join("src"), 1).unwrap(),
        );
        app.expanded.insert(root.join("src"));
        app.terminal = Terminals::new(root);
        app.terminal.set_appearance(app.settings.theme);
        let (mut panes, editor) = pane_grid::State::new(Pane::Editor);
        let (_, split) = panes
            .split(pane_grid::Axis::Horizontal, editor, Pane::Terminal)
            .unwrap();
        panes.resize(split, 0.72);
        app.panes = panes;
        let (mut columns, sidebar) = pane_grid::State::new(WorkspacePane::Sidebar);
        let (_, split) = columns
            .split(pane_grid::Axis::Vertical, sidebar, WorkspacePane::Editor)
            .unwrap();
        columns.resize(split, 0.19);
        app.columns = columns;
        std::fs::create_dir_all(".artifacts").unwrap();
        if std::env::var_os("KITE_RENDER_PREVIEWS_ONLY").is_none() {
            render(&mut app, ".artifacts/kite-preview.png");
            for appearance in crate::appearance::Appearance::ALL {
                let _ = app.update(Message::ThemeSelected(appearance));
                render(
                    &mut app,
                    &format!(
                        ".artifacts/theme-{}.png",
                        appearance.to_string().to_lowercase()
                    ),
                );
            }
            let _ = app.update(Message::ThemeSelected(crate::appearance::Appearance::Dark));
            let tabs = std::mem::take(&mut app.tabs);
            render(&mut app, ".artifacts/kite-welcome.png");
            app.tabs = tabs;
            app.palette = Some(Palette::new(true));
            render(&mut app, ".artifacts/kite-palette-preview.png");
            app.palette = None;
            app.settings_open = true;
            render(&mut app, ".artifacts/kite-settings-preview.png");
            app.settings_open = false;
            app.find = true;
            app.query = "println".into();
            render(&mut app, ".artifacts/kite-find-preview.png");
            let compact = iced::Size::new(800., 520.);
            render_size(&mut app, ".artifacts/kite-compact-find.png", compact);
            app.find = false;
            app.settings_open = true;
            render_size(&mut app, ".artifacts/kite-compact-settings.png", compact);
            app.settings_open = false;
            app.palette = Some(Palette::new(true));
            render_size(&mut app, ".artifacts/kite-compact-palette.png", compact);
            app.palette = None;
            let saved_tabs = std::mem::take(&mut app.tabs);
            render_size(&mut app, ".artifacts/kite-compact-welcome.png", compact);
            app.tabs = saved_tabs;
            for zoom in [AppZoom::ALL[0], AppZoom::ALL[6]] {
                app.settings.app_zoom = zoom;
                render_size(
                    &mut app,
                    &format!(".artifacts/app-zoom-{}.png", zoom.percent()),
                    compact,
                );
                app.settings_open = true;
                render_size(
                    &mut app,
                    &format!(".artifacts/settings-zoom-{}.png", zoom.percent()),
                    compact,
                );
                app.settings_open = false;
            }
        }
        app.settings.theme = crate::appearance::Appearance::Dark;
        app.settings.app_zoom = AppZoom::default();
        app.settings.terminal = false;
        let markdown = include_str!("../docs/examples/native-preview.md");
        app.tabs[0].path = Some(PathBuf::from("preview.md"));
        app.tabs[0].editor = Editor::new(markdown);
        app.tabs[0].preview.mode = DocumentMode::Split;
        app.tabs[0].preview.rendered = Some(crate::preview::Preview::new(crate::preview::prepare(
            markdown,
            "md",
            app.settings.theme,
        )));
        render(&mut app, ".artifacts/native-markdown-split.png");
        app.tabs[0].preview.mode = DocumentMode::Preview;
        render(&mut app, ".artifacts/native-markdown-preview.png");
        app.tabs[0].path = Some(PathBuf::from("preview.mmd"));
        let mermaid = include_str!("../docs/examples/sequence.mmd");
        app.tabs[0].editor = Editor::new(mermaid);
        app.tabs[0].preview.rendered = Some(crate::preview::Preview::new(crate::preview::prepare(
            mermaid,
            "mmd",
            app.settings.theme,
        )));
        render(&mut app, ".artifacts/native-mermaid-preview.png");
    }
    fn fixture(path: PathBuf) -> App {
        let (panes, _) = pane_grid::State::new(Pane::Editor);
        let (columns, _) = pane_grid::State::new(WorkspacePane::Editor);
        App {
            tabs: vec![Tab {
                id: 1,
                path: Some(path),
                editor: Editor::new("hello world\nhello again"),
                line_ending: LineEnding::Lf,
                bom: false,
                fingerprint: None,
                saving: false,
                preview: DocumentPreview::default(),
            }],
            active: 0,
            next_id: 2,
            folder: None,
            tree: vec![],
            expanded: HashSet::new(),
            tree_generation: 0,
            terminal: Terminals::new(PathBuf::from("/kite-test-missing-directory")),
            panes,
            columns,
            settings: Settings::default(),
            error: None,
            status: String::new(),
            find: false,
            settings_open: false,
            query: String::new(),
            replacement: String::new(),
            match_end: 0,
            modal: false,
            pending: None,
            pending_save: None,
            quit_discarded: HashSet::new(),
            palette: None,
            file_index: FileIndex::default(),
            persist_session: false,
        }
    }
    #[test]
    fn save_on_close_blocks_edits_until_saved() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = fixture(dir.path().join("test.txt"));
        app.pending = Some(Pending::Close(1));
        let _ = app.update(Message::Save);
        assert!(app.modal);
        let _ = app.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Insert('x'),
        )));
        assert_eq!(
            app.tabs[0].editor.content.text(),
            "hello world\nhello again"
        );
    }
    #[test]
    fn typing_during_an_ordinary_save_stays_dirty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        let mut app = fixture(path.clone());
        let _ = app.update(Message::Save);
        assert!(!app.modal);
        let _ = app.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Insert('x'),
        )));
        let _ = app.update(Message::Saved(1, 0, path, Ok(blake3::hash(b"saved"))));
        assert!(app.tabs[0].editor.dirty());
        assert!(app.tabs[0].editor.content.text().starts_with('x'));
    }
    #[test]
    fn background_save_cannot_dismiss_another_tabs_close_prompt() {
        for result in [Ok(blake3::hash(b"saved")), Err("Disk full".into())] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("one.txt");
            let mut app = fixture(path.clone());
            let _ = app.update(Message::Save);
            let _ = app.update(Message::New);
            let _ = app.update(Message::Edit(text_editor::Action::Edit(
                text_editor::Edit::Insert('x'),
            )));
            let _ = app.update(Message::Close(2));
            assert!(app.modal);
            let _ = app.update(Message::Saved(1, 0, path, result));
            assert!(app.modal);
            assert!(matches!(app.pending, Some(Pending::Close(2))));
            assert_eq!(app.tabs.len(), 2);
            assert!(app.tabs[1].editor.dirty());
            let _ = app.update(Message::Confirmed(Decision::Cancel));
            assert!(!app.modal);
            assert_eq!(app.tabs.len(), 2);
        }
    }
    #[test]
    fn only_the_save_chosen_by_a_close_prompt_can_finish_that_close() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("one.txt");
        let second = dir.path().join("two.txt");
        let mut app = fixture(first.clone());
        let _ = app.update(Message::Save);
        let _ = app.update(Message::Loaded(Ok(LoadedFile {
            path: second.clone(),
            text: "second".into(),
            line_ending: LineEnding::Lf,
            bom: false,
            fingerprint: blake3::hash(b"second"),
        })));
        let _ = app.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Insert('x'),
        )));
        let _ = app.update(Message::Close(2));
        let _ = app.update(Message::Confirmed(Decision::Save));
        assert_eq!(app.pending_save, Some(2));
        let _ = app.update(Message::Saved(1, 0, first, Ok(blake3::hash(b"saved"))));
        assert!(app.modal);
        assert_eq!(app.tabs.len(), 2);
        let _ = app.update(Message::Saved(2, 1, second, Ok(blake3::hash(b"xsecond"))));
        assert!(!app.modal);
        assert!(app.pending.is_none());
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.tabs[0].id, 1);
    }
    #[test]
    fn find_replace_is_undoable() {
        let mut app = fixture(PathBuf::from("test.txt"));
        app.query = "hello".into();
        app.replacement = "hi".into();
        let _ = app.update(Message::FindNext);
        let _ = app.update(Message::Replace);
        assert_eq!(app.tabs[0].editor.content.text(), "hi world\nhello again");
        let _ = app.update(Message::Undo);
        assert_eq!(
            app.tabs[0].editor.content.text(),
            "hello world\nhello again"
        );
    }
    #[test]
    fn background_load_cannot_switch_a_confirmation_target() {
        let mut app = fixture(PathBuf::from("one.txt"));
        app.modal = true;
        app.pending = Some(Pending::Close(1));
        let _ = app.update(Message::Loaded(Ok(LoadedFile {
            path: PathBuf::from("two.txt"),
            text: "second".into(),
            line_ending: LineEnding::Lf,
            bom: false,
            fingerprint: blake3::hash(b"second"),
        })));
        assert_eq!(app.active, 0);
        assert_eq!(app.tabs[app.active].id, 1);
    }
}
