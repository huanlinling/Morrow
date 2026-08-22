#!/bin/bash
# Package a Morrow mod into a .mor file (ZIP archive).
# Usage: ./scripts/package-mod.sh <mod-directory>
set -euo pipefail

MOD_DIR="${1:?Usage: $0 <mod-directory>}"
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MOD_NAME=$(basename "$MOD_DIR")
# Cargo replaces hyphens with underscores in library names
CARGO_NAME="${MOD_NAME//-/_}"
OUTPUT="$PROJECT_ROOT/$MOD_NAME.mor"

# Detect platform
ARCH=$(uname -m)
case "$ARCH" in
    x86_64|amd64) ARCH="x86_64" ;;
    aarch64) ARCH="aarch64" ;;
esac

case "$(uname -s)" in
    Linux)  PLATFORM="linux-$ARCH"; LIB_EXT="so" ;;
    Darwin) PLATFORM="macos-$ARCH"; LIB_EXT="dylib" ;;
    MINGW*|MSYS*|CYGWIN*) PLATFORM="windows-$ARCH"; LIB_EXT="dll" ;;
    *) echo "Unknown platform: $(uname -s)"; exit 1 ;;
esac

echo "==> Packaging $MOD_NAME for $PLATFORM"

# Build the mod from workspace root (single target/ for workspace)
cd "$PROJECT_ROOT"
cargo build --release

# Find the built library in the workspace target directory
LIB_SRC=$(find target/release -name "lib${CARGO_NAME}*.${LIB_EXT}" -type f | head -1)
if [ -z "$LIB_SRC" ]; then
    echo "ERROR: Built library not found. Expected: lib${CARGO_NAME}*.${LIB_EXT}"
    exit 1
fi
LIB_NAME="lib${CARGO_NAME}.${LIB_EXT}"

# Create the .mor package (ZIP)
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

# Copy manifest, optional config
cp "$MOD_DIR"/manifest.toml "$TMPDIR"/
[ -f "$MOD_DIR"/config.toml ] && cp "$MOD_DIR"/config.toml "$TMPDIR"/

# Copy artifact to platform dir
mkdir -p "$TMPDIR/$PLATFORM"
cp "$LIB_SRC" "$TMPDIR/$PLATFORM/$LIB_NAME"

# Create ZIP using Python3 (always available)
rm -f "$OUTPUT"
cd "$TMPDIR"
python3 -c "
import zipfile, os
with zipfile.ZipFile('$OUTPUT', 'w', zipfile.ZIP_STORED) as zf:
    for root, dirs, files in os.walk('.'):
        for f in files:
            path = os.path.join(root, f)
            arcname = path[2:] if path.startswith('./') else path
            zf.write(path, arcname)
"

echo "==> Created: $OUTPUT"
echo "    Contents:"
python3 -c "
import zipfile
with zipfile.ZipFile('$OUTPUT', 'r') as zf:
    for f in zf.infolist():
        print(f'  {f.filename:40s} {f.file_size:>8} bytes')
"
