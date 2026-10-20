# Releasing Kite

Kite distributes native installers through GitHub Releases. `main` is the stable branch. `dev` is the integration branch and the source of prereleases. Feature pull requests target `dev`; reviewed release and hotfix pull requests target `main`.

## Downloads

| Platform | Target | Release assets |
| --- | --- | --- |
| macOS, Apple Silicon | `aarch64-apple-darwin` | `kite-VERSION-macos-arm64.dmg` and `.zip` |
| macOS, Intel | `x86_64-apple-darwin` | `kite-VERSION-macos-x64.dmg` and `.zip` |
| Windows, x64 | `x86_64-pc-windows-msvc` | `kite-VERSION-windows-x64-setup.exe` and `.zip` |
| Linux | Native source build | Install from the source checkout using the [README](../README.md) |

The macOS disk image contains `Kite.app` and an Applications shortcut. The Windows installer installs `kite.exe` for the current user, adds a Start menu shortcut, and includes an uninstaller. Administrator access is not required. The ZIP downloads contain the same app or executable for portable use.

Each platform includes a build-info file and SHA-256 manifest. A combined `SHA256SUMS.txt` covers all installers, archives, and build-info files in the release. GitHub also provides source archives for the release tag. Linux is compiled and tested in CI; no Linux binary package is published.

Installers belong in [GitHub Release assets](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases). [GitHub Packages](https://docs.github.com/en/packages/learn-github-packages/introduction-to-github-packages) supports package registries such as npm and containers; it does not provide a native DMG or EXE installer registry. Kite uses Releases for desktop downloads and does not publish a container or library package solely to populate the Packages section.

## Local packaging

macOS needs Xcode command line tools and Rust 1.88 or later. Build for the host or select an installed Rust target:

```sh
bash scripts/package.sh --target aarch64-apple-darwin
bash scripts/package.sh --target x86_64-apple-darwin
```

Package an existing release executable with `bash scripts/package.sh --binary target/release/kite`. Use `--skip-dmg` for an app and ZIP only. Output goes to `dist/TARGET/`; set `KITE_DIST_DIR` to change the output root.

On Windows, install the MSVC Rust toolchain, Visual Studio C++ Build Tools, and NSIS 3, then run:

```powershell
./scripts/package-windows.ps1
```

Use `-Binary target/release/kite.exe` for an existing executable. The PowerShell script must run on Windows. `KITE_DIST_DIR` also selects its output root.

Before packaging, regenerate `THIRD_PARTY_NOTICES.md` using `python3 scripts/third-party-notices.py --target TARGET`. The workflows do this automatically. Packages include the license and dependency notices. Refresh `assets/package/rust-stdlib-notices.txt` from the pinned compiler's `share/doc/rust/COPYRIGHT-library.html` and `licenses/` when updating the compiler.

## Signing

Stable public releases require signing credentials. The workflow also supports draft development previews without signing credentials; these are always marked as prereleases and include a signing warning. Configure these repository secrets before dispatching a signed release. Keep private keys and passwords out of source control.

| macOS secret | Value |
| --- | --- |
| `MACOS_CERTIFICATE_BASE64` | Base64-encoded Developer ID Application `.p12` |
| `MACOS_CERTIFICATE_PASSWORD` | Password for the `.p12` |
| `MACOS_SIGNING_IDENTITY` | Full Developer ID Application identity name |
| `APPLE_ID` | Apple developer account email |
| `APPLE_TEAM_ID` | Developer team identifier |
| `APPLE_APP_SPECIFIC_PASSWORD` | App-specific password for notarization |

The workflow imports the certificate into a temporary keychain, signs the app with hardened runtime and a timestamp, notarizes and staples the app and disk image, verifies the result, and deletes the keychain. The two macOS architectures use native [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

For Windows, set `WINDOWS_CERTIFICATE_BASE64` to a base64-encoded Authenticode `.pfx` and `WINDOWS_CERTIFICATE_PASSWORD` to its password. The packaging script signs and verifies both `kite.exe` and the installer with SHA-256 and a timestamp. `WINDOWS_TIMESTAMP_URL` can override the default timestamp service for local builds. Hardware-backed or cloud signing needs a provider-specific adapter; this workflow expects an exportable certificate.

Local macOS builds are ad-hoc signed by default. Local Windows builds are unsigned by default. These development artifacts are identified in their build-info files. On macOS, `MACOS_SIGNING_IDENTITY` and `MACOS_NOTARY_PROFILE` can use an existing keychain; `MACOS_SIGNING_KEYCHAIN` selects a specific keychain. `--require-signing` or `-RequireSigning` fails if the required credentials are absent. Choose `signed: true` for a signed release. Signing failures stop those builds and do not substitute unsigned artifacts. Choose `signed: false` to produce a draft preview with ad-hoc macOS signatures and unsigned Windows binaries. Preview tags use `vVERSION-preview`, preserving `vVERSION` for a later signed release.

## Release sequence

1. Set the version in `Cargo.toml`, update `Cargo.lock`, and update `CHANGELOG.md`. Stable versions such as `0.1.0` release from `main`. Versions such as `0.2.0-beta.1` release from `dev` and are marked as prereleases. Build metadata is not accepted in release versions.
2. Run CI and review installer smoke checks. Stable releases also require native GUI and installer acceptance on clean macOS and Windows systems. Preview notes must identify any outstanding manual platform testing. The destination tag must be new: `vVERSION` for signed builds or `vVERSION-preview` for unsigned previews.
3. Open Actions, select **Prepare Kite release**, select `main` or `dev`, enter the exact Cargo version without `v`, and select whether to sign. Unsigned builds always start as draft prereleases.
4. The workflow validates the channel, tests and builds the three native targets, applies the selected signing policy, and verifies asset completeness and checksums. It creates a draft release for the tested commit with the selected version tag and generated notes. Unsigned notes explicitly identify the macOS and Windows signing limitations.
5. Download the draft assets and verify installation, launch, file opening, terminal use, and uninstallation where supported. Run `shasum -a 256 -c SHA256SUMS.txt` on macOS or `Get-FileHash FILE -Algorithm SHA256` on Windows to verify downloads.
6. Review release notes and publish the draft when its acceptance requirements are met. Unsigned previews must remain marked as prereleases and retain the signing warning; they must not be advertised as stable. Stable releases require signing and native acceptance. Stable releases remain on `main`; merge release fixes and version changes back to `dev`.

Ordinary pushes and pull requests build development artifacts with a 14-day retention period. They do not publish releases. Release builds use the commit selected when the workflow was dispatched. Re-running the complete workflow for an existing version is rejected to prevent replacing a published tag; inspect an existing draft or failed run before retrying its failed jobs.

## Repository setup

Set `main` as the default branch and create `dev` from it. Protect both branches with pull request review and required CI checks. Configure signing secrets, enable Actions, and use the release workflow after the repository is connected. The workflow files prepare this behavior; they do not create remote branches, change repository settings, provision signing accounts, or publish a release by themselves.
