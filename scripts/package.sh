#!/usr/bin/env bash
# Build and package the native app using the host platform's packaging tools.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/package.sh [--target TRIPLE] [--skip-build] [--binary PATH]
                          [--require-signing] [--skip-dmg]

macOS: creates a .app, ZIP, and DMG. Local builds are ad-hoc signed by default.
Linux: build and install from source; no Linux packages are produced.
Windows: use scripts/package-windows.ps1 instead.

--binary PATH       Package an existing executable; implies --skip-build.
--require-signing   On macOS, require Developer ID signing and notarization.
--skip-dmg          On macOS, create only the app and ZIP.

Optional environment:
  KITE_DIST_DIR            Output root (default: repository/dist).
  MACOS_SIGNING_IDENTITY    Developer ID Application identity in your keychain.
  MACOS_SIGNING_KEYCHAIN    Optional keychain containing the identity/profile.
  MACOS_NOTARY_PROFILE      Stored notarytool credentials for notarization.
  MACOSX_DEPLOYMENT_TARGET  Minimum macOS version (default: 12.0).
EOF
}

fail() { printf 'Error: %s\n' "$*" >&2; exit 1; }
require() { command -v "$1" >/dev/null 2>&1 || fail "Required tool is missing: $1"; }

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target=""
binary=""
skip_build=false
require_signing=false
skip_dmg=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --target) [[ $# -ge 2 ]] || fail '--target needs a value'; target="$2"; shift 2 ;;
    --binary) [[ $# -ge 2 ]] || fail '--binary needs a value'; binary="$2"; skip_build=true; shift 2 ;;
    --skip-build) skip_build=true; shift ;;
    --require-signing) require_signing=true; shift ;;
    --skip-dmg) skip_dmg=true; shift ;;
    --help|-h) usage; exit 0 ;;
    *) fail "Unknown option: $1" ;;
  esac
done

require cargo
require rustc
require python3
[[ -n "$target" ]] || target="$(rustc -vV | sed -n 's/^host: //p')"
case "$target" in
  aarch64-apple-darwin) platform=macos; arch=arm64 ;;
  x86_64-apple-darwin) platform=macos; arch=x64 ;;
  *-linux-*) fail 'Linux uses source installation. See README.md for cargo install instructions.' ;;
  *) fail "Unsupported target: $target" ;;
esac
case "$(uname -s):$platform" in
  Darwin:macos) ;;
  *) fail "Package $target on its native operating system" ;;
esac

for tool in codesign ditto hdiutil lipo plutil; do require "$tool"; done
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-12.0}"
if "$require_signing"; then
  [[ -n "${MACOS_SIGNING_IDENTITY:-}" ]] || fail 'MACOS_SIGNING_IDENTITY is required'
  [[ -n "${MACOS_NOTARY_PROFILE:-}" ]] || fail 'MACOS_NOTARY_PROFILE is required'
fi
if [[ -n "${MACOS_NOTARY_PROFILE:-}" && -z "${MACOS_SIGNING_IDENTITY:-}" ]]; then
  fail 'Notarization requires MACOS_SIGNING_IDENTITY'
fi

work="$(mktemp -d "${TMPDIR:-/tmp}/kite-package.XXXXXX")"
trap 'rm -rf "$work"' EXIT
cargo metadata --locked --no-deps --format-version 1 --manifest-path "$repo_root/Cargo.toml" > "$work/metadata.json"
version="$(python3 -c 'import json,sys; print(next(p["version"] for p in json.load(open(sys.argv[1]))["packages"] if p["name"] == "kite"))' "$work/metadata.json")"
target_root="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["target_directory"])' "$work/metadata.json")"
[[ -n "$binary" ]] || binary="$target_root/$target/release/kite"
if ! "$skip_build"; then
  cargo build --locked --release --target "$target" --manifest-path "$repo_root/Cargo.toml" --bin kite
fi
[[ -f "$binary" && -x "$binary" ]] || fail "Executable not found: $binary"
binary="$(python3 -c 'import os,sys; print(os.path.abspath(sys.argv[1]))' "$binary")"

out="${KITE_DIST_DIR:-$repo_root/dist}/$target"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
stem="kite-$version-$platform-$arch"
for suffix in zip dmg BUILD-INFO.txt SHA256SUMS.txt; do
  rm -f "$out/$stem.$suffix" "$out/$stem-$suffix"
done
signing_status='unsigned development artifact'

copy_docs() {
  local destination="$1"
  mkdir -p "$destination"
  for document in README.md LICENSE NOTICE THIRD_PARTY_NOTICES.md; do
    if [[ -f "$repo_root/$document" ]]; then
      cp "$repo_root/$document" "$destination/$document"
    fi
  done
}

bundle_version="${version%%-*}"
bundle_version="${bundle_version%%+*}"
app="$out/Kite.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/kite"
chmod 755 "$app/Contents/MacOS/kite"
copy_docs "$app/Contents/Resources"
[[ -f "$repo_root/assets/package/kite.icns" ]] || fail 'Missing app icon: assets/package/kite.icns'
cp "$repo_root/assets/package/kite.icns" "$app/Contents/Resources/kite.icns"
expected_arch=arm64
[[ "$arch" != x64 ]] || expected_arch=x86_64
lipo -verify_arch "$expected_arch" "$app/Contents/MacOS/kite"
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Kite</string>
<key>CFBundleDisplayName</key><string>Kite</string>
<key>CFBundleExecutable</key><string>kite</string>
<key>CFBundleIdentifier</key><string>io.github.kite.editor</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleIconFile</key><string>kite</string>
<key>CFBundleShortVersionString</key><string>$bundle_version</string>
<key>CFBundleVersion</key><string>$bundle_version</string>
<key>LSMinimumSystemVersion</key><string>$MACOSX_DEPLOYMENT_TARGET</string>
<key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict></plist>
EOF
plutil -lint "$app/Contents/Info.plist"
signing_args=()
notary_args=()
if [[ -n "${MACOS_SIGNING_KEYCHAIN:-}" ]]; then
  signing_args+=(--keychain "$MACOS_SIGNING_KEYCHAIN")
  notary_args+=(--keychain "$MACOS_SIGNING_KEYCHAIN")
fi
if [[ -n "${MACOS_SIGNING_IDENTITY:-}" ]]; then
  codesign --force --options runtime --timestamp --sign "$MACOS_SIGNING_IDENTITY" ${signing_args[@]+"${signing_args[@]}"} "$app"
  signing_status='Developer ID signed; not notarized'
else
  codesign --force --sign - "$app"
  signing_status='ad-hoc signed; not Developer ID signed or notarized'
fi
codesign --verify --deep --strict --verbose=2 "$app"
if [[ -n "${MACOS_NOTARY_PROFILE:-}" ]]; then
  ditto -c -k --sequesterRsrc --keepParent "$app" "$work/notarization.zip"
  xcrun notarytool submit "$work/notarization.zip" --keychain-profile "$MACOS_NOTARY_PROFILE" ${notary_args[@]+"${notary_args[@]}"} --wait
  xcrun stapler staple "$app"
  xcrun stapler validate "$app"
  spctl --assess --type execute --verbose=2 "$app"
  signing_status='Developer ID signed and notarized'
fi
ditto -c -k --sequesterRsrc --keepParent "$app" "$out/$stem.zip"
if ! "$skip_dmg"; then
  mkdir -p "$work/dmg"
  ditto "$app" "$work/dmg/Kite.app"
  ln -s /Applications "$work/dmg/Applications"
  hdiutil create -volname Kite -srcfolder "$work/dmg" -ov -format UDZO "$out/$stem.dmg"
  if [[ -n "${MACOS_SIGNING_IDENTITY:-}" ]]; then
    codesign --force --timestamp --sign "$MACOS_SIGNING_IDENTITY" ${signing_args[@]+"${signing_args[@]}"} "$out/$stem.dmg"
    codesign --verify --verbose=2 "$out/$stem.dmg"
  fi
  if [[ -n "${MACOS_NOTARY_PROFILE:-}" ]]; then
    xcrun notarytool submit "$out/$stem.dmg" --keychain-profile "$MACOS_NOTARY_PROFILE" ${notary_args[@]+"${notary_args[@]}"} --wait
    xcrun stapler staple "$out/$stem.dmg"
    xcrun stapler validate "$out/$stem.dmg"
  fi
  hdiutil verify "$out/$stem.dmg"
fi

source_revision="$(git -C "$repo_root" rev-parse HEAD 2>/dev/null || printf 'uncommitted workspace')"
cat > "$out/$stem-BUILD-INFO.txt" <<EOF
Product: Kite
Version: $version
Target: $target
Source: $source_revision
Signing: $signing_status
Built: $(date -u '+%Y-%m-%dT%H:%M:%SZ')
EOF
python3 - "$out" "$stem" <<'PY'
import hashlib
import pathlib
import sys

directory, stem = pathlib.Path(sys.argv[1]), sys.argv[2]
artifacts = sorted(p for p in directory.iterdir() if p.is_file() and p.name.startswith(stem) and not p.name.endswith('SHA256SUMS.txt'))
with (directory / f'{stem}-SHA256SUMS.txt').open('w', encoding='utf-8') as output:
    for artifact in artifacts:
        digest = hashlib.sha256()
        with artifact.open('rb') as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b''):
                digest.update(chunk)
        output.write(f'{digest.hexdigest()}  {artifact.name}\n')
PY
printf '\nPackages: %s\nSigning: %s\n' "$out" "$signing_status"
