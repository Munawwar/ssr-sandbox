#!/usr/bin/env bash
set -euo pipefail

# SSR Bundle Builder
# Builds frontend code with esbuild, code-splitting enabled, for use with ssr-sandbox

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Hardcoded defaults for this project
ENTRY_POINT="example-src/entry.js"
OUT_DIR="example-dist"
FORMAT="esm"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() { echo -e "${GREEN}[build]${NC} $1"; }
warn() { echo -e "${YELLOW}[warn]${NC} $1"; }
error() { echo -e "${RED}[error]${NC} $1" >&2; }

# Check for esbuild
if ! command -v esbuild &> /dev/null; then
    if command -v npx &> /dev/null; then
        ESBUILD="npx esbuild"
        warn "Using npx esbuild (install globally for faster builds: npm i -g esbuild)"
    else
        error "esbuild not found. Install with: npm i -g esbuild"
        exit 1
    fi
else
    ESBUILD="esbuild"
fi

# Clean output directory
log "Cleaning $OUT_DIR..."
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

# Build with esbuild
log "Building SSR bundle from $ENTRY_POINT..."
log "Output: $OUT_DIR"
log "Format: $FORMAT"
log "Code splitting: enabled"

$ESBUILD "$ENTRY_POINT" \
    --bundle \
    --format="$FORMAT" \
    --splitting \
    --outdir="$OUT_DIR" \
    --platform=neutral \
    --target=es2023 \
    --minify \
    --sourcemap \
    --metafile="$OUT_DIR/meta.json" \
    --out-extension:.js=.js \
    --external:node:* \
    --log-level=info

# Entry chunk name
ENTRY_CHUNK="entry.js"

# Summary
echo ""
log "Build complete!"
echo ""
echo "Chunks generated:"
ls -lh "$OUT_DIR"/*.js 2>/dev/null | awk '{print "  " $9 " (" $5 ")"}'
echo ""
echo "Run SSR with:"
echo "  ./target/release/ssr-sandbox $OUT_DIR $OUT_DIR/$ENTRY_CHUNK '{\"page\":\"home\"}'"
