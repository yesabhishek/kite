# Native terminal

`src/terminal.rs` integrates `iced_term` 0.8.0 with iced 0.14.0. The shell runs in a real pseudo-terminal (PTY); Alacritty parses terminal output and iced paints the grid. There is no browser, terminal webview, extension host, or command-output simulation.

## Integration

Construct `TerminalPane::new(workspace_directory)` once. Map `view()`, `subscription()`, `update(event)`, `focus()`, and `restart()` through the application's `Terminal(iced_term::Event)` message. Keep the subscription active when hiding a running panel, otherwise a noisy shell can fill its event queue. The application owns the panel header and Restart button.

Keyboard focus belongs to the terminal widget. Application keyboard listeners must respect captured events; in particular, do not turn Ctrl-C into an editor copy action. Native shortcuts are Cmd-C/V on macOS and Ctrl-Shift-C/V on Windows/Linux. Ctrl-C sends ETX to interrupt the foreground program. The wrapper fixes iced_term 0.8.0's incorrect Ctrl-U mapping.

The selected shell is `$SHELL` when it identifies an existing file, falling back to `/bin/zsh` on macOS and `/bin/sh` on other Unix systems. On Windows it is `%ComSpec%`, falling back to `cmd.exe`; WSL is not required. Common macOS shells receive `-l` so Finder launches load the user's login PATH. `TERM`, `COLORTERM`, and `TERM_PROGRAM` identify the terminal, and other environment variables are inherited.

`restart_in(path)` explicitly replaces the old session. Opening another folder must not inject `cd` or any other command into an existing interactive shell. Each session has a unique ID so queued events cannot modify a replacement shell. Terminal initialization and workspace-directory errors render inside the pane without terminating the editor. Normal shell exit preserves the final rendered screen, shows its exit status if available, and stops its subscription; Restart creates a fresh session.

## Resource lifetime

The local pinned widget limits scrollback to 2,000 lines and adds no secondary output log. iced_term uses bounded internal event channels and a PTY worker thread. Dropping or restarting the pane drops iced_term's backend, which sends the worker a Shutdown message. Alacritty then sends SIGHUP and reaps its Unix shell, or closes Windows ConPTY. Closed subscriptions now stop instead of polling indefinitely.

The upstream worker join handle is detached, so shutdown is requested rather than synchronously joined by the application. Intentionally detached child processes can outlive their shell, just as in a conventional terminal. Native behavior on each target must be tested before distributing that platform's package.

## Local widget patches and remaining limits

The local MIT-licensed copy of iced_term 0.8.0 honors bracketed-paste mode, strips clipboard escape sequences that could close that mode early, and converts ordinary pasted newlines to terminal Enter. Selection copy uses Alacritty's native line/wrapping logic. Font measurement is no longer repeated on every event, and nonvisual input/title events avoid grid copies. Clipboard/paste regression tests cover these changes.

The widget still snapshots its bounded grid on visual changes, and high-output throughput needs target-specific measurement. The final screen retained after exit is read-only. Complex grapheme rendering and full terminal compatibility require native acceptance testing. The widget is an early implementation, not a claim of full VS Code terminal compatibility.

## Verification

Automated tests cover workspace startup failure, the Ctrl-C/Ctrl-U control-byte regression, custom-shell arguments, bracketed paste, and actual PTY input, ANSI parsing, and resizing on Unix. Run `cargo test --locked -p kite -p iced_term` to include the local widget tests.

For each packaged platform, verify:

- Start in a folder whose path contains spaces and Unicode; check `pwd` or `cd` output.
- Print ANSI colors and Unicode; resize the pane while running a full-screen terminal program.
- Run a long foreground process, press Ctrl-C, and confirm the prompt returns.
- Switch focus between editor and terminal; typing and copy/paste affect only the focused surface.
- Generate more than the scrollback limit, then confirm memory settles instead of growing without bound.
- Exit the shell, restart repeatedly, and check that old shells/PTY workers disappear.
- Close the application during active output and check that its ordinary foreground shell exits.

Source references: [iced_term 0.8.0](https://docs.rs/crate/iced_term/0.8.0/source/), [Alacritty terminal 0.25.1](https://docs.rs/crate/alacritty_terminal/0.25.1/source/).
