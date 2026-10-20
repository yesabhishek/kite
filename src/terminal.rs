//! The integrated shell is a native PTY, with Alacritty parsing and iced rendering.
//!
//! Keep this pane's subscription alive while it is running, including when the
//! panel is hidden. Let the terminal widget handle keyboard events before global
//! application shortcuts so Ctrl-C reaches the shell instead of the editor.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use iced::keyboard::Modifiers;
use iced::widget::{column, container, text};
use iced::{Element, Length, Subscription, Task};
use iced_term::actions::Action;
use iced_term::bindings::{Binding, BindingAction, InputKind};
use iced_term::settings::{BackendSettings, FontSettings, Settings, ThemeSettings};
use iced_term::{AlacrittyEvent, BackendCommand, Command, Event, TermMode, Terminal, TerminalView};

// A fresh subscription identity prevents a restarted shell from receiving
// queued events that belonged to its predecessor.
static NEXT_TERMINAL_ID: AtomicU64 = AtomicU64::new(1);

pub struct TerminalPane {
    terminal: Option<Terminal>,
    cwd: PathBuf,
    title: String,
    running: bool,
    error: Option<String>,
    exit_code: Option<i32>,
}

impl TerminalPane {
    /// Start the platform shell in `cwd`. Startup errors stay visible in the pane.
    pub fn new(cwd: PathBuf) -> Self {
        let mut pane = Self {
            terminal: None,
            cwd,
            title: String::from("Terminal"),
            running: false,
            error: None,
            exit_code: None,
        };
        pane.start();
        pane
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn update(&mut self, event: Event) -> Task<Event> {
        let Event::BackendCall(id, command) = event;
        let Some(terminal) = self.terminal.as_mut() else {
            return Task::none();
        };
        if terminal.id != id || !self.running {
            return Task::none();
        }

        if let BackendCommand::ProcessAlacrittyEvent(AlacrittyEvent::ChildExit(code)) = &command {
            self.exit_code = Some(*code);
        }

        match terminal.handle(Command::ProxyToBackend(command)) {
            Action::Shutdown => self.running = false,
            Action::ChangeTitle(title) => {
                // Shell titles are terminal-controlled text, not markup. Keep
                // them compact and exclude invisible/control characters.
                let title: String = title
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(100)
                    .collect();
                if !title.trim().is_empty() {
                    self.title = title;
                }
            }
            Action::Ignore => {}
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Event> {
        if let Some(error) = &self.error {
            return container(
                container(
                    column![
                        text("Unable to start terminal").size(13),
                        text(error.as_str()).size(12),
                        text("Use Restart terminal in the toolbar to try again.")
                            .size(11)
                            .style(muted_text),
                    ]
                    .spacing(7),
                )
                .padding([14, 16])
                .max_width(680)
                .style(|theme: &iced::Theme| {
                    let danger = theme.palette().danger;
                    container::Style::default()
                        .background(danger.scale_alpha(0.06))
                        .border(iced::Border {
                            color: danger.scale_alpha(0.22),
                            width: 1.0,
                            radius: 5.0.into(),
                        })
                }),
            )
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
        }

        let Some(terminal) = &self.terminal else {
            return container(
                column![
                    text("Terminal stopped").size(13),
                    text("Use Restart terminal in the toolbar to start a shell.")
                        .size(12)
                        .style(muted_text),
                ]
                .spacing(7),
            )
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
        };

        if self.running {
            TerminalView::show(terminal)
        } else {
            let message = match self.exit_code {
                Some(code) => format!(
                    "Shell exited with status {code}. Use Restart terminal to start a new session."
                ),
                None => String::from("Shell exited. Use Restart terminal to start a new session."),
            };
            column![
                TerminalView::show(terminal),
                container(text(message).size(11).style(muted_text))
                    .padding([8, 12])
                    .width(Length::Fill)
                    .style(|theme: &iced::Theme| {
                        container::Style::default()
                            .background(theme.palette().text.scale_alpha(0.035))
                    })
            ]
            .height(Length::Fill)
            .into()
        }
    }

    pub fn subscription(&self) -> Subscription<Event> {
        match (&self.terminal, self.running) {
            (Some(terminal), true) => terminal.subscription(),
            _ => Subscription::none(),
        }
    }

    pub fn focus(&self) -> Task<Event> {
        match (&self.terminal, self.running) {
            (Some(terminal), true) => TerminalView::focus(terminal.widget_id().clone()),
            _ => Task::none(),
        }
    }

    /// Terminate the old PTY and create a fresh shell in its original directory.
    pub fn restart(&mut self) -> Task<Event> {
        self.start();
        self.focus()
    }

    /// Explicitly restart in a new workspace; never inject `cd` into a live shell.
    pub fn restart_in(&mut self, cwd: PathBuf) -> Task<Event> {
        self.cwd = cwd;
        self.restart()
    }

    fn start(&mut self) {
        // iced_term::Backend::drop sends Shutdown to its PTY event loop.
        // Alacritty then hangs up/reaps the Unix shell, or closes Windows ConPTY.
        self.terminal = None;
        self.running = false;
        self.error = None;
        self.exit_code = None;

        match std::fs::canonicalize(&self.cwd) {
            Ok(cwd) if cwd.is_dir() => self.cwd = cwd,
            Ok(_) => {
                self.error = Some(format!("{} is not a directory.", self.cwd.display()));
                return;
            }
            Err(error) => {
                self.error = Some(format!("Cannot open {}: {error}", self.cwd.display()));
                return;
            }
        }

        let program = default_shell();
        self.title = Path::new(&program)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Terminal")
            .to_owned();

        let settings = Settings {
            font: FontSettings {
                size: 13.0,
                ..FontSettings::default()
            },
            theme: ThemeSettings::new(Box::new(iced_term::ColorPalette {
                background: String::from("#181818"),
                foreground: String::from("#CCCCCC"),
                ..iced_term::ColorPalette::default()
            })),
            backend: BackendSettings {
                args: shell_arguments(&program),
                program,
                env: HashMap::from([
                    (String::from("TERM"), String::from("xterm-256color")),
                    (String::from("COLORTERM"), String::from("truecolor")),
                    (String::from("TERM_PROGRAM"), String::from("kite")),
                ]),
                working_directory: Some(self.cwd.clone()),
            },
        };

        match Terminal::new(NEXT_TERMINAL_ID.fetch_add(1, Ordering::Relaxed), settings) {
            Ok(mut terminal) => {
                terminal.handle(Command::AddBindings(shell_bindings()));
                self.terminal = Some(terminal);
                self.running = true;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}

fn muted_text(theme: &iced::Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().text.scale_alpha(0.64)),
    }
}

fn default_shell() -> String {
    #[cfg(target_os = "windows")]
    {
        // ComSpec is the installed Windows command processor. Do not require
        // WSL, PowerShell 7, or any optional package just to open a terminal.
        std::env::var("ComSpec")
            .ok()
            .filter(|shell| !shell.trim().is_empty())
            .unwrap_or_else(|| String::from("cmd.exe"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("SHELL")
            .ok()
            .filter(|shell| !shell.is_empty() && Path::new(shell).is_file())
            .unwrap_or_else(|| {
                if cfg!(target_os = "macos") {
                    String::from("/bin/zsh")
                } else {
                    String::from("/bin/sh")
                }
            })
    }
}

fn shell_arguments(program: &str) -> Vec<String> {
    // A login shell obtains the user's PATH when started from Finder. Restrict
    // this flag to shells known to accept it; custom shells keep their defaults.
    let name = Path::new(program)
        .file_name()
        .and_then(|name| name.to_str());
    if cfg!(target_os = "macos") && matches!(name, Some("bash" | "zsh" | "sh" | "fish")) {
        vec![String::from("-l")]
    } else {
        Vec::new()
    }
}

fn shell_bindings() -> Vec<(Binding<InputKind>, BindingAction)> {
    let binding = |key: &str, modifiers, action| {
        (
            Binding {
                target: InputKind::Char(key.to_owned()),
                modifiers,
                terminal_mode_include: TermMode::empty(),
                terminal_mode_exclude: TermMode::empty(),
            },
            action,
        )
    };
    vec![
        // Preserve terminal interrupt even on platforms where Ctrl-C copies in
        // other widgets. Copy/paste use Cmd-C/V (macOS) or Ctrl-Shift-C/V.
        binding("c", Modifiers::CTRL, BindingAction::Char('\x03')),
        // iced_term 0.8 accidentally maps Ctrl-U to ASCII 'Q'.
        binding("u", Modifiers::CTRL, BindingAction::Char('\x15')),
        binding(
            "u",
            Modifiers::CTRL | Modifiers::SHIFT,
            BindingAction::Char('\x15'),
        ),
    ]
}

/// Keep every PTY subscribed while only the selected session is visible.
pub struct Terminals {
    pub sessions: Vec<TerminalPane>,
    pub active: usize,
    appearance: crate::appearance::Appearance,
    cwd: PathBuf,
}
impl Terminals {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            sessions: vec![TerminalPane::new(cwd.clone())],
            active: 0,
            appearance: Default::default(),
            cwd,
        }
    }
    pub fn cwd(&self) -> &Path {
        self.sessions
            .get(self.active)
            .map(|p| p.cwd())
            .unwrap_or(&self.cwd)
    }
    pub fn focus(&self) -> Task<Event> {
        self.sessions
            .get(self.active)
            .map(|p| p.focus())
            .unwrap_or_else(Task::none)
    }
    pub fn restart_in(&mut self, cwd: PathBuf) -> Task<Event> {
        let task = if self.sessions.is_empty() {
            self.add(cwd)
        } else {
            self.sessions[self.active].restart_in(cwd)
        };
        self.set_appearance(self.appearance);
        task
    }
    pub fn view(&self) -> Element<'_, Event> {
        if let Some(pane) = self.sessions.get(self.active) {
            pane.view()
        } else {
            container(
                column![
                    text("No terminal sessions").size(13),
                    text("Use the + button above to start a shell.")
                        .size(12)
                        .style(muted_text),
                ]
                .spacing(7),
            )
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        }
    }
    pub fn add(&mut self, cwd: PathBuf) -> Task<Event> {
        self.sessions.push(TerminalPane::new(cwd));
        self.active = self.sessions.len() - 1;
        self.set_appearance(self.appearance);
        self.focus()
    }
    pub fn close(&mut self, index: usize) -> Task<Event> {
        if index >= self.sessions.len() {
            return Task::none();
        }
        self.sessions.remove(index);
        if self.sessions.is_empty() {
            self.active = 0;
            return Task::none();
        }
        if index < self.active {
            self.active -= 1;
        }
        self.active = self.active.min(self.sessions.len() - 1);
        self.focus()
    }
    pub fn set_appearance(&mut self, appearance: crate::appearance::Appearance) {
        self.appearance = appearance;
        for pane in &mut self.sessions {
            if let Some(terminal) = &mut pane.terminal {
                terminal.handle(Command::ChangeTheme(Box::new(appearance.terminal())));
            }
        }
    }
    pub fn update(&mut self, event: Event) -> Task<Event> {
        let Event::BackendCall(id, _) = &event;
        if let Some(pane) = self
            .sessions
            .iter_mut()
            .find(|p| p.terminal.as_ref().is_some_and(|t| t.id == *id))
        {
            return pane.update(event);
        }
        Task::none()
    }
    pub fn subscription(&self) -> Subscription<Event> {
        Subscription::batch(self.sessions.iter().map(TerminalPane::subscription))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn restarted_sessions_resize_with_the_existing_native_widget_cache() {
        let directory = tempfile::tempdir().unwrap();
        let mut pane = TerminalPane::new(directory.path().to_owned());
        let mut renderer = iced_tiny_skia::Renderer::new(iced::Font::default(), 14.0.into());
        let mut cache = iced_runtime::user_interface::Cache::new();
        let mut previous_id = None;
        for restart in 0..5 {
            if restart > 0 {
                let _ = pane.restart();
            }
            let id = pane.terminal.as_ref().unwrap().id;
            if let Some(previous) = previous_id {
                assert_ne!(id, previous);
                let title = pane.title().to_owned();
                let _ = pane.update(Event::BackendCall(
                    previous,
                    BackendCommand::ProcessAlacrittyEvent(AlacrittyEvent::Exit),
                ));
                let _ = pane.update(Event::BackendCall(
                    previous,
                    BackendCommand::ProcessAlacrittyEvent(AlacrittyEvent::Title(String::from(
                        "stale shell",
                    ))),
                ));
                assert!(pane.is_running());
                assert_eq!(pane.title(), title);
            }
            let mut interface = iced_runtime::UserInterface::build(
                pane.view(),
                iced::Size::new(800.0, 320.0),
                cache,
                &mut renderer,
            );
            let mut messages = Vec::new();
            let _ = interface.update(
                &[iced::Event::Window(iced::window::Event::RedrawRequested(
                    iced::time::Instant::now(),
                ))],
                iced::mouse::Cursor::Unavailable,
                &mut renderer,
                &mut iced::advanced::clipboard::Null,
                &mut messages,
            );
            let mut idle_messages = Vec::new();
            let (idle_state, _) = interface.update(
                &[iced::Event::Window(iced::window::Event::RedrawRequested(
                    iced::time::Instant::now(),
                ))],
                iced::mouse::Cursor::Unavailable,
                &mut renderer,
                &mut iced::advanced::clipboard::Null,
                &mut idle_messages,
            );
            assert!(idle_messages.is_empty());
            assert!(matches!(
                idle_state,
                iced_runtime::user_interface::State::Updated {
                    redraw_request: iced::window::RedrawRequest::Wait,
                    ..
                }
            ));
            cache = interface.into_cache();
            let resizes: Vec<_> = messages
                .iter()
                .filter_map(|event| match event {
                    Event::BackendCall(
                        event_id,
                        BackendCommand::Resize(Some(size), Some(font)),
                    ) => Some((*event_id, *size, *font)),
                    _ => None,
                })
                .collect();
            assert_eq!(resizes.len(), 1, "restart {restart} did not resize its PTY");
            assert_eq!(resizes[0].0, id);
            assert_eq!(resizes[0].1, iced::Size::new(800.0, 320.0));
            assert!(resizes[0].2.width > 1.0 && resizes[0].2.height > 1.0);
            for event in messages {
                let _ = pane.update(event);
            }
            previous_id = Some(id);
        }
    }

    #[test]
    fn backend_events_are_routed_to_their_own_session() {
        let dir = tempfile::tempdir().unwrap();
        let mut terminals = Terminals::new(dir.path().to_owned());
        let _ = terminals.add(dir.path().to_owned());
        let id = terminals.sessions[0].terminal.as_ref().unwrap().id;
        let other_title = terminals.sessions[1].title().to_owned();
        let _ = terminals.update(Event::BackendCall(
            id,
            BackendCommand::ProcessAlacrittyEvent(AlacrittyEvent::Title("first shell".into())),
        ));
        assert_eq!(terminals.sessions[0].title(), "first shell");
        assert_eq!(terminals.sessions[1].title(), other_title);
        assert_eq!(terminals.active, 1);
    }

    #[test]
    fn closing_sessions_keeps_selection_valid_and_allows_an_empty_panel() {
        let cwd = PathBuf::from("/kite-test-missing-directory");
        let mut terminals = Terminals::new(cwd.clone());
        let _ = terminals.add(cwd.clone());
        let _ = terminals.add(cwd.clone());
        assert_eq!(terminals.active, 2);
        let _ = terminals.close(0);
        assert_eq!(terminals.active, 1);
        let _ = terminals.close(1);
        assert_eq!(terminals.active, 0);
        let _ = terminals.close(0);
        assert!(terminals.sessions.is_empty());
        assert_eq!(terminals.cwd(), cwd);
        let _ = terminals.focus();
        let _ = terminals.add(cwd);
        assert_eq!(terminals.sessions.len(), 1);
        assert_eq!(terminals.active, 0);
    }

    #[test]
    fn terminal_control_keys_remain_control_bytes() {
        let bindings = shell_bindings();
        for (key, expected) in [("c", '\x03'), ("u", '\x15')] {
            let (_, action) = bindings
                .iter()
                .find(|(binding, _)| {
                    binding.target == InputKind::Char(key.to_owned())
                        && binding.modifiers == Modifiers::CTRL
                })
                .unwrap();
            assert_eq!(action, &BindingAction::Char(expected));
        }
    }

    #[test]
    fn custom_shells_do_not_receive_assumed_login_flags() {
        assert!(shell_arguments("/custom/shell").is_empty());
    }

    #[test]
    fn missing_workspace_reports_an_error_without_starting_a_shell() {
        let pane = TerminalPane::new(PathBuf::from("/kite-nonexistent-workspace/child"));
        assert!(!pane.is_running());
        assert!(pane.error().is_some());
        assert!(pane.terminal.is_none());
    }
}
