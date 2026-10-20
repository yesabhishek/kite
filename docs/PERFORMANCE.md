# Initial performance measurements

These are local development measurements of Kite 0.1.0 on Apple Silicon, macOS 27.0.1, using Rust 1.88 and the optimized release profile. They are not a comparison with VS Code or Cursor.

| Measurement | Result |
| --- | --- |
| Stripped release executable | 7,844,704 bytes (7.48 MiB) |
| macOS ZIP | 4,032,444 bytes (3.85 MiB) |
| macOS DMG | 4,668,700 bytes (4.45 MiB) |
| Active editor and shell, physical footprint | 95.9 MiB |
| Peak physical footprint during this run | 111.7 MiB |
| RSS during this run | 161.9 MiB |
| Idle CPU, three successive five-second intervals | 0.0%, 0.0%, 0.0% of one core |

The workload was one small Rust source file, its folder explorer, and one idle `/bin/sh` PTY in a 1280 × 800 window. The process ran with `--temporary-session`; no user session preferences were changed. CPU samples used differences in `ps` cumulative process time divided by elapsed monotonic time. The time counter's resolution means 0.0% is a sampled result, not proof of zero instructions. Memory uses `vmmap -summary`; RSS and physical footprint measure different things. Shell child-process memory is not included in the app's row.

The first release profile with a focused, blinking editor caret showed approximately 22% CPU at idle. A process sample placed the work in repeated software rendering and CoreGraphics presentation. The editor now uses a steady caret, and an integration test verifies that the focused production widget tree requests no periodic redraws. Editor and terminal input still cause redraws normally.

An earlier development session retained approximately 647 MiB, mostly in allocator free caches. The short final run above does not establish a long-session memory ceiling. Larger files, sustained output, repeated resizing, many open tabs, IME input, startup latency, and Windows/Linux need separate measurements before broader speed or memory claims.

The native layout was inspected through renders of the production Rust widgets. Desktop interaction checks were blocked by ScreenCaptureKit capture error -3811. Automated document, undo, save-race, clipboard preparation, PTY input/output/resize, and idle-redraw checks pass; they do not replace native acceptance testing on each supported operating system.
