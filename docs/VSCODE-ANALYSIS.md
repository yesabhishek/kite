# VS Code investigation with REA

Target: the locally installed Visual Studio Code **1.142.0**, at `/Applications/Visual Studio Code.app/Contents/Resources/app`.

REA inspected the shipped package as inert JavaScript/HTML. No application code was executed by REA, no native decompiler was required, and no user profile, workspace history, credentials, or extension settings were inspected. The implementation is original Rust code using independently licensed libraries; it does not copy Microsoft branding or bundled source.

## Evidence and boundary

- Root artifact SHA-256: `14890596b2f6655b72f47ea35f2a4cc66f03a088bad40c505807ac9db0f2cfde`.
- Architecture analysis: `ev_661fbc83ed32a7e54e46fd8c10dc1eadedb34b5e76946985d6d1a00316c69e69`.
- Summary projection: `ev_34ef3fe08d55f7f5f47c0ccd940bc80935bf48e1768fe818f705cfda85d24db6`.
- Earlier bootstrap analysis: `ev_ea2c47626ea03d4976ee9a56a763af8ee9229af9cfa657eef0a766930b0f8993`.

The architecture pass inventoried 6,166 relevant files and selected six: `package.json`, `out/mainImpl.js`, the renderer preload, the PTY host, and the workbench HTML/bootstrap. It parsed four JavaScript files and approximately 1.78 MB of selected text. It reported three BrowserWindow constructions, one context bridge exposing six members, ten IPC observations, and a utility-process observation.

Coverage is **partial**. 6,160 inventoried files were outside the selected pass, dynamic IPC expressions remained unresolved, and bounded semantic retention left explicit unknowns. The approximately 20.7 MB desktop workbench bundle was identified by its loader but was not exhaustively analyzed. Static observations do not establish runtime reachability, memory use, authorization enforcement, or startup cost. The complete session was exported locally to ignored `.artifacts/vscode-rea-evidence.json` for follow-up; it is not included in application packages.

## What the shipped files show

| Subsystem | Observation | Implication for Kite |
| --- | --- | --- |
| Desktop window | The main bundle constructs Electron browser windows. The workbench HTML loads CSS and a module bootstrap which imports `workbench.desktop.main.js`. | Render the workspace directly with a native Rust toolkit. |
| Native bridge | The renderer preload exposes a `vscode` context bridge for configuration/process/IPC services. | Keep file/document state in the native application; a renderer-to-main web bridge is unnecessary for this scope. |
| Terminal | `mainImpl.js` starts a `ptyHost` utility process with entry point `vs/platform/terminal/node/ptyHostMain`. That host imports `node-pty` and `@xterm/headless`; the package declares `@xterm/xterm` and its renderer add-ons. | Preserve real PTY semantics while replacing the web terminal renderer and dedicated host with Alacritty and a native widget. |
| Extensions | The main bundle includes an extension-host starter and extension-host entry points. | Omit that subsystem entirely because extensibility is outside the requested product. |

These are observations of the shipped files, supplemented by bounded direct checks of the same artifacts. The simpler process and dependency structure is a design choice. It is reasonable to expect smaller packages after removing a browser engine and extension subsystem, but the investigation did not measure or prove a runtime speedup over VS Code.

## Resulting design

One native Rust UI manages the editor, lazy explorer, bounded quick-open index, and terminal widget. Native shell child processes remain necessary for an interactive terminal. Editor text, dirty state, undo, file I/O, and terminal lifetime are separate components so a noisy shell or disk operation cannot require rebuilding the whole application architecture.

The release binary and package sizes are recorded after building. Broader claims require comparable workloads and measurements on each target OS.
