#!/bin/bash
# Download ALL pre-built .so files from a GitHub release.
#
# Downloads prod + test .so files for every platform and Python version:
#   2 platforms (macOS arm64, Linux x86_64) × 3 Python versions (3.12, 3.13, 3.14) × 2 modes = 12 files
#
# Usage:
#   ./download_binaries.sh              # latest release
#   ./download_binaries.sh v0.1.0       # specific release tag
#   ./download_binaries.sh --tag v0.1.0

set -euo pipefail

REPO="anshalshukla/leanMultisig-py"
TAG=""
PYTHON_VERSIONS="3.12 3.13 3.14"
LIB_NAMES="lean_multisig lean_multisig_test"

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        --tag) TAG="$2"; shift 2 ;;
        -h|--help)
            echo "Usage: $0 [--tag TAG]"
            echo ""
            echo "Downloads all pre-built .so files (all platforms, Python versions, prod+test)"
            echo "from a GitHub release into lean_multisig_py/."
            exit 0
            ;;
        v*) TAG="$1"; shift ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

# Check for gh CLI
if ! command -v gh &>/dev/null; then
    echo "Error: GitHub CLI (gh) is required. Install from https://cli.github.com"
    exit 1
fi

# Get release tag
if [ -z "$TAG" ]; then
    TAG=$(gh release view --repo "$REPO" --json tagName -q .tagName 2>/dev/null || true)
    if [ -z "$TAG" ]; then
        echo "Error: No releases found. Specify a tag with --tag."
        exit 1
    fi
fi
echo "Release: $TAG"
echo ""

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEST_DIR="$SCRIPT_DIR/lean_multisig_py"
mkdir -p "$DEST_DIR"

# Remove old .so files
echo "Cleaning old .so files..."
rm -f "$DEST_DIR"/lean_multisig*.cpython-*.so
echo ""

DOWNLOADED=0
FAILED=0
FAILED_FILES=()

for PYVER in $PYTHON_VERSIONS; do
    PYVER_SHORT="${PYVER//./}"
    for LIB_NAME in $LIB_NAMES; do
        for SO_SUFFIX in "darwin.so" "x86_64-linux-gnu.so"; do
            FILENAME="${LIB_NAME}.cpython-${PYVER_SHORT}-${SO_SUFFIX}"
            echo -n "  $FILENAME ... "
            if gh release download "$TAG" --repo "$REPO" --pattern "$FILENAME" --dir "$DEST_DIR" --clobber 2>/dev/null; then
                SIZE=$(du -h "$DEST_DIR/$FILENAME" | cut -f1)
                echo "OK ($SIZE)"
                DOWNLOADED=$((DOWNLOADED + 1))
            else
                echo "MISSING"
                FAILED=$((FAILED + 1))
                FAILED_FILES+=("$FILENAME")
            fi
        done
    done
done

echo ""
echo "=========================================="
echo " Downloaded: $DOWNLOADED / $((DOWNLOADED + FAILED))"
echo "=========================================="

if [ ${#FAILED_FILES[@]} -gt 0 ]; then
    echo ""
    echo "Missing files:"
    for f in "${FAILED_FILES[@]}"; do
        echo "  - $f"
    done
fi

echo ""
echo "Files in $DEST_DIR:"
ls -lh "$DEST_DIR"/*.so 2>/dev/null || echo "  (none)"
