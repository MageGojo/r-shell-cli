#!/usr/bin/env bash
# Build a macOS .dmg for the R-Shell command-line tool.
#
# R-Shell is a CLI, so the DMG simply ships the `r-shell` binary plus a short
# install note. Users drag the binary somewhere on their PATH (or run the
# included hint). Usage:
#
#   create_dmg.sh <target-triple> <output.dmg>
#
# Example:
#   create_dmg.sh aarch64-apple-darwin dist/r-shell-macos-apple-silicon.dmg

set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <target-triple> <output.dmg>" >&2
  exit 2
fi

TARGET="$1"
OUTPUT_DMG="$2"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIST="$ROOT/dist"

VERSION="${GITHUB_REF_NAME:-0.0.0}"
VERSION="${VERSION#v}"
if [ "$VERSION" = "main" ] || [ -z "$VERSION" ]; then
  VERSION="0.0.0"
fi

# Locate the release binary (per-target dir first, then plain release dir).
BIN_DIR="$ROOT/cli/target/$TARGET/release"
if [ ! -d "$BIN_DIR" ]; then
  BIN_DIR="$ROOT/cli/target/release"
fi
MAIN_BIN="$BIN_DIR/r-shell"
if [ ! -x "$MAIN_BIN" ]; then
  echo "missing release binary at $MAIN_BIN" >&2
  exit 1
fi

WORK="$DIST/macos-$TARGET"
DMG_ROOT="$WORK/dmgroot"

rm -rf "$WORK" "$OUTPUT_DMG"
mkdir -p "$DMG_ROOT" "$DIST"

# Stage the binary and supporting files into the DMG root.
cp "$MAIN_BIN" "$DMG_ROOT/r-shell"
chmod +x "$DMG_ROOT/r-shell"
[ -f "$ROOT/README.md" ] && cp "$ROOT/README.md" "$DMG_ROOT/README.md"
[ -f "$ROOT/LICENSE" ] && cp "$ROOT/LICENSE" "$DMG_ROOT/LICENSE"

# A short, friendly install note shown inside the mounted DMG.
cat > "$DMG_ROOT/INSTALL.txt" <<EOF
R-Shell $VERSION ($TARGET)

R-Shell is a command-line tool. To install:

  1. Open Terminal.
  2. Copy the binary onto your PATH, for example:

       sudo cp /Volumes/r-shell/r-shell /usr/local/bin/

  3. Verify it works:

       r-shell --version
       r-shell --help

Note: macOS Gatekeeper may block an unsigned binary on first run. If so:

  xattr -d com.apple.quarantine /usr/local/bin/r-shell

Project: https://github.com/MageGojo/r-shell-cli
EOF

# Ad-hoc sign the binary so it at least runs locally after the quarantine note.
if command -v codesign >/dev/null 2>&1; then
  codesign --force --sign - "$DMG_ROOT/r-shell" || true
fi

# Build a compressed DMG.
hdiutil create \
  -volname "r-shell" \
  -srcfolder "$DMG_ROOT" \
  -ov \
  -format UDZO \
  "$OUTPUT_DMG"

rm -rf "$WORK"
echo "created $OUTPUT_DMG"
