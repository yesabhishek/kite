# Architecture and implementation plan

Kite implements a small native workspace: file explorer, document tabs, editor, and terminal. Kite's visual hierarchy uses warm paper and graphite surfaces, a restrained orange accent, compact title and tab bars, precise icon controls, subtle borders, and keyboard navigation. Quick open and a command palette expose actions without a permanent toolbar of large buttons.

## Components

- **UI:** Iced 0.14 with the Tiny Skia software renderer. No browser engine, Node runtime, or GPU backend is shipped. The sidebar and terminal use independently resizable pane grids. Icons are native vector paths.
- **Editor:** Iced's Cosmic Text editing surface, incremental Syntect highlighting, and a line-number gutter derived from the renderer's actual visible layout runs. Undo records contain changed lines and cursor positions, with a 2 MiB target and 512-entry cap per document; the latest atomic edit remains undoable even when larger than that target.
- **Files:** Blocking filesystem work runs on worker tasks. Saves write and sync a temporary file beside the destination, then replace it. Content fingerprints detect external modifications before replacement. Existing file permissions, UTF-8 BOM, and the document's selected line-ending style are retained. Mixed line endings normalize to the detected style when saved.
- **Workspace:** The explorer reads only expanded directories. Quick open performs a separate bounded, on-demand traversal respecting ignore files; it does not start a permanent indexer or watcher.
- **Terminal:** A direct native PTY managed by Alacritty and painted by a pinned Iced terminal widget. macOS/Linux use the user's shell, Windows uses ComSpec/ConPTY. Hiding a terminal keeps the session running. Changing folders preserves the active shell; restarting explicitly starts in the workspace folder.
- **State:** Dirty buffers stay in memory until saved or explicitly discarded. A save initiated by Close/Quit prevents subsequent edits until completion. Ordinary asynchronous saves leave any newer edits dirty. Background file loads cannot switch an active confirmation target.

## Local dependency patches

`vendor/iced_term` is pinned to published 0.8.0 with its MIT license preserved. Changes cap scrollback at 2,000 lines, honor bracketed paste, use Alacritty's selection-to-string logic, stop closed subscriptions, avoid repeated font measurement and nonvisual grid copies, and avoid crashing when a link cannot open. The wrapper corrects Ctrl-U and handles startup errors, fresh subscription IDs, shell exit, and restart.

`vendor/iced_widget` is pinned to published 0.14.2 with Iced's MIT license preserved. A read-only `Content::with_editor` accessor lets the gutter inspect shaped visible runs without cloning or guessing scroll position. An opt-in steady-caret setting removes periodic editor redraws; Kite enables it because the macOS software surface otherwise repaints the whole window on each blink.

## Validation and remaining release gates

Automated coverage includes UTF-8/BOM and CRLF round trips, conflict-safe saves, file rejection, Unicode search, undo/redo, replace, save/close races, ignore handling, palette filtering, terminal control bytes, bracketed paste, and a real PTY input/output/resize test. Preview tests render the production widgets offscreen.

The initial macOS package and resource measurements are recorded in [PERFORMANCE.md](PERFORMANCE.md), including the idle-redraw fix and limits of the measurements.

Before publication, complete desktop checks on each OS for IME/Unicode input, clipboard selection, rapid output with Ctrl-C, repeated terminal restarts, pane resizing, dirty-close prompts, and installation/uninstallation. Initial scope omits large-file paging, accessibility integration beyond the framework, filesystem watching, and crash recovery. Package size, RSS/physical footprint, startup latency, and idle CPU must be measured separately; Rust alone does not establish a speed claim.
