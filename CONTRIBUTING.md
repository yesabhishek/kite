# Contributing to Kite

Kite is a small native editor and terminal. Contributions should make everyday work clearer, more reliable, or more direct. Keep changes focused and explain the concrete behavior they improve.

## Branches

- `main` holds stable release work.
- `dev` is the integration branch and the target for feature and fix pull requests.
- Create short lived branches from `dev`, such as `fix/terminal-resize` or `feature/quick-open`.
- Promote `dev` to `main` through a reviewed release pull request after CI and desktop checks pass.

## Getting started

Follow the platform dependencies in the [README](README.md#build-from-source), then build and run with Cargo. The local `vendor/` patches are intentional. Preserve upstream licenses and describe any changes to patched dependencies.

Before submitting, run:

```sh
cargo fmt --all -- --check
cargo fmt --manifest-path vendor/iced_term/Cargo.toml -- --check
cargo fmt --manifest-path vendor/iced_widget/Cargo.toml -- --check
cargo test --locked -p kite -p iced_term
cargo clippy --locked --all-targets -- -D warnings
```

Add a regression test when behavior changes or a bug needs coverage. UI changes should include a screenshot at normal and compact window sizes. Exercise keyboard navigation, light and dark themes, app zoom, and unsaved-change prompts where relevant.

The native widget renderer can generate screenshots without opening a desktop window:

```sh
SHELL=/bin/sh cargo test --locked -p kite render_native_preview -- --ignored
```

Generated images appear in `.artifacts/`. Packaging instructions and native desktop acceptance checks live in [RELEASING.md](docs/RELEASING.md).

## Design and writing

Use the [Kite identity guidelines](docs/design/application-identity.md). Prefer familiar words, quiet surfaces, visible focus, and compact controls with enough room to operate. Use a single accent with purpose. Do not add emojis or em dashes to project copy.

## Issues and pull requests

For bugs, include the operating system, Kite version, reproduction steps, expected result, and actual result. Remove private file paths, credentials, and document contents from logs and screenshots.

For features, describe the task you need to complete and how it fits Kite's small native workspace. A proposal or issue is useful before large architectural changes. Keep discussions respectful and review the work on its merits.

Contributions are licensed under the project's MIT license. Preserve existing copyright and dependency attribution.
