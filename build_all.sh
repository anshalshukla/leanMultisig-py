#!/bin/bash
# Build .so files for macOS arm64 (native) and Linux x86_64 (Docker)
# Produces prod and test variants for Python 3.12, 3.13, 3.14
#
# Prerequisites:
#   - Rust toolchain (rustup)
#   - maturin: cargo install maturin
#   - asdf with python plugin, versions 3.12.x 3.13.x 3.14.x installed
#   - Docker (for Linux builds)
#
# Usage:
#   ./build_all.sh              # build everything
#   ./build_all.sh --macos-only # skip Linux/Docker builds
#   ./build_all.sh --linux-only # skip macOS builds
#   ./build_all.sh --clean      # remove old .so files first

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PACKAGE_DIR="$SCRIPT_DIR/lean_multisig_py"
PYTHON_VERSIONS="3.12 3.13 3.14"
BUILD_MACOS=true
BUILD_LINUX=true
CLEAN=false
FAILED_BUILDS=()

# Parse args
for arg in "$@"; do
    case "$arg" in
        --macos-only) BUILD_LINUX=false ;;
        --linux-only) BUILD_MACOS=false ;;
        --clean) CLEAN=true ;;
        -h|--help)
            echo "Usage: $0 [--macos-only] [--linux-only] [--clean]"
            exit 0
            ;;
        *) echo "Unknown arg: $arg"; exit 1 ;;
    esac
done

# ── Prerequisite checks ──────────────────────────────────────────────

echo "Checking prerequisites..."

# Rust
if ! command -v rustc &>/dev/null; then
    echo "Error: Rust not found. Install via https://rustup.rs"
    exit 1
fi
echo "  Rust: $(rustc --version)"

# Maturin
MATURIN=""
if [ -f "$HOME/.cargo/bin/maturin" ]; then
    MATURIN="$HOME/.cargo/bin/maturin"
elif command -v maturin &>/dev/null; then
    MATURIN="maturin"
fi
if [ -z "$MATURIN" ]; then
    echo "  maturin not found — installing via cargo..."
    cargo install maturin
    MATURIN="$HOME/.cargo/bin/maturin"
fi
echo "  maturin: $MATURIN"

# Python versions (macOS) — checked after resolve_python is defined below
# (deferred to after helpers section)

# Docker (Linux)
if $BUILD_LINUX; then
    if ! command -v docker &>/dev/null; then
        echo "  Error: Docker not found. Install Docker Desktop for Mac."
        echo "  Skipping Linux builds."
        BUILD_LINUX=false
    elif ! docker info &>/dev/null 2>&1; then
        echo "  Error: Docker is not running. Start Docker Desktop."
        echo "  Skipping Linux builds."
        BUILD_LINUX=false
    else
        echo "  Docker: $(docker --version)"
        # Pre-pull the maturin image
        echo "  Pulling maturin Docker image (if needed)..."
        docker pull --platform linux/amd64 ghcr.io/pyo3/maturin:latest 2>/dev/null || true
    fi
fi

# ── Clean old .so files ──────────────────────────────────────────────

if $CLEAN; then
    echo ""
    echo "Cleaning old .so files..."
    rm -f "$PACKAGE_DIR"/lean_multisig*.cpython-*.so
    echo "  Done."
fi

# ── Helpers ──────────────────────────────────────────────────────────

resolve_python() {
    local PYVER="$1"
    local PYTHON_BIN=""
    # asdf installs look like ~/.asdf/installs/python/3.12.0/bin/python3.12
    # Try all installed patch versions for this minor version
    if command -v asdf &>/dev/null; then
        for dir in "$HOME"/.asdf/installs/python/${PYVER}*/bin; do
            if [ -x "$dir/python$PYVER" ]; then
                PYTHON_BIN="$dir/python$PYVER"
                break
            elif [ -x "$dir/python3" ]; then
                PYTHON_BIN="$dir/python3"
                break
            fi
        done
    fi
    # Fallback: resolve shim to real path
    if [ -z "$PYTHON_BIN" ]; then
        local SHIM
        SHIM=$(command -v "python$PYVER" 2>/dev/null || true)
        if [ -n "$SHIM" ] && [ -x "$SHIM" ]; then
            # If it's an asdf shim, resolve it
            local REAL
            REAL=$("$SHIM" -c "import sys; print(sys.executable)" 2>/dev/null || true)
            if [ -n "$REAL" ] && [ -x "$REAL" ]; then
                PYTHON_BIN="$REAL"
            else
                PYTHON_BIN="$SHIM"
            fi
        fi
    fi
    echo "$PYTHON_BIN"
}

# Deferred Python version check (uses resolve_python)
if $BUILD_MACOS; then
    for PYVER in $PYTHON_VERSIONS; do
        PYTHON_BIN=$(resolve_python "$PYVER")
        if [ -z "$PYTHON_BIN" ] || [ ! -x "$PYTHON_BIN" ]; then
            echo "  Warning: python$PYVER not found — macOS builds for $PYVER will be skipped"
            echo "    Install with: asdf install python ${PYVER}.0"
        else
            echo "  python$PYVER: $PYTHON_BIN"
        fi
    done
fi

# Temporarily patch [lib] name in Cargo.toml for test builds.
# maturin names the .so after [lib] name, but the #[pymodule] symbol
# for test builds is PyInit_lean_multisig_test.
patch_cargo_toml() {
    local LIB_NAME="$1"
    cp "$SCRIPT_DIR/Cargo.toml" "$SCRIPT_DIR/Cargo.toml.bak"
    # Only replace the [lib] name line, not dependency names
    sed -i.tmp '/^\[lib\]/,/^\[/{s/^name = "lean_multisig"/name = "'"$LIB_NAME"'"/;}' "$SCRIPT_DIR/Cargo.toml"
    rm -f "$SCRIPT_DIR/Cargo.toml.tmp"
}

restore_cargo_toml() {
    if [ -f "$SCRIPT_DIR/Cargo.toml.bak" ]; then
        mv "$SCRIPT_DIR/Cargo.toml.bak" "$SCRIPT_DIR/Cargo.toml"
    fi
}

# Ensure Cargo.toml is restored on any exit
trap restore_cargo_toml EXIT

extract_so() {
    local WHEEL="$1"
    local DEST_NAME="$2"

    if [ -z "$WHEEL" ] || [ ! -f "$WHEEL" ]; then
        echo "  Warning: No wheel found"
        return 1
    fi

    local TMP_DIR
    TMP_DIR=$(mktemp -d)
    unzip -o "$WHEEL" -d "$TMP_DIR" > /dev/null

    local SO_FILE
    # Match exactly the expected .so by name to avoid picking up bundled .so files
    SO_FILE=$(find "$TMP_DIR" -name "$DEST_NAME" -type f | head -1)
    # Fallback: match by platform pattern
    if [ -z "$SO_FILE" ]; then
        if echo "$DEST_NAME" | grep -q linux; then
            SO_FILE=$(find "$TMP_DIR" -name "*linux*.so" -type f | head -1)
        else
            SO_FILE=$(find "$TMP_DIR" -name "*darwin*.so" -type f | head -1)
        fi
    fi
    # Last resort
    if [ -z "$SO_FILE" ]; then
        SO_FILE=$(find "$TMP_DIR" -name "*.so" -type f | head -1)
    fi
    if [ -n "$SO_FILE" ]; then
        cp "$SO_FILE" "$PACKAGE_DIR/$DEST_NAME"
        local SIZE
        SIZE=$(du -h "$PACKAGE_DIR/$DEST_NAME" | cut -f1)
        echo "  Created: $DEST_NAME ($SIZE)"
    else
        echo "  Warning: No .so file found in wheel"
        rm -rf "$TMP_DIR"
        return 1
    fi
    rm -rf "$TMP_DIR"
}

# ── Build function ───────────────────────────────────────────────────

build_variant() {
    local PYVER="$1"
    local VARIANT="$2"   # "prod" or "test"
    local PLATFORM="$3"  # "macos" or "linux"

    local PYVER_SHORT="${PYVER//./}"
    local FEATURES=""
    local LIB_NAME="lean_multisig"

    if [ "$VARIANT" = "test" ]; then
        FEATURES="--features test-config"
        LIB_NAME="lean_multisig_test"
    fi

    echo ""
    echo "── $LIB_NAME | $PLATFORM | Python $PYVER ──"

    # Patch Cargo.toml [lib] name for test builds
    if [ "$VARIANT" = "test" ]; then
        patch_cargo_toml "$LIB_NAME"
    fi

    local BUILD_OK=false

    if [ "$PLATFORM" = "macos" ]; then
        local PYTHON_BIN
        PYTHON_BIN=$(resolve_python "$PYVER")
        if [ -z "$PYTHON_BIN" ] || [ ! -x "$PYTHON_BIN" ]; then
            echo "  Skipped: python$PYVER not found"
            [ "$VARIANT" = "test" ] && restore_cargo_toml
            FAILED_BUILDS+=("$LIB_NAME-$PLATFORM-py$PYVER (python not found)")
            return
        fi
        echo "  Interpreter: $PYTHON_BIN"
        if RUSTFLAGS="-C target-cpu=native" $MATURIN build --release $FEATURES -i "$PYTHON_BIN" 2>&1; then
            BUILD_OK=true
        fi
    else
        # Clean cargo target for the crate to force recompile when switching features.
        # Docker builds share the same target/ dir, so without this, cargo reuses
        # the prod build for test (or vice versa) since only a feature flag differs.
        docker run --rm \
            --platform linux/amd64 \
            --entrypoint bash \
            -v "$SCRIPT_DIR":/io \
            -w /io \
            ghcr.io/pyo3/maturin:latest \
            -c "rm -rf target/release/.fingerprint/lean-multisig-py-* target/release/.fingerprint/leansig_wrapper-* target/release/.fingerprint/rec_aggregation-* target/release/.fingerprint/lean-multisig-* target/release/deps/liblean_multisig* target/release/deps/libleansig_wrapper* target/release/deps/librec_aggregation* 2>/dev/null; maturin build --release $FEATURES -i python$PYVER" 2>&1 && BUILD_OK=true
    fi

    # Restore Cargo.toml before extracting
    if [ "$VARIANT" = "test" ]; then
        restore_cargo_toml
    fi

    if ! $BUILD_OK; then
        echo "  FAILED"
        FAILED_BUILDS+=("$LIB_NAME-$PLATFORM-py$PYVER")
        return
    fi

    # Find the wheel and extract .so
    local WHEEL=""
    local DEST_SUFFIX=""
    if [ "$PLATFORM" = "macos" ]; then
        WHEEL=$(ls -t "$SCRIPT_DIR/target/wheels/"*cp${PYVER_SHORT}*macos*.whl 2>/dev/null | head -1)
        DEST_SUFFIX="darwin.so"
    else
        WHEEL=$(ls -t "$SCRIPT_DIR/target/wheels/"*cp${PYVER_SHORT}*manylinux*.whl 2>/dev/null | grep -v aarch64 | head -1)
        if [ -z "$WHEEL" ]; then
            WHEEL=$(ls -t "$SCRIPT_DIR/target/wheels/"*cp${PYVER_SHORT}*linux*x86_64*.whl 2>/dev/null | head -1)
        fi
        DEST_SUFFIX="x86_64-linux-gnu.so"
    fi

    local DEST_NAME="${LIB_NAME}.cpython-${PYVER_SHORT}-${DEST_SUFFIX}"
    if ! extract_so "$WHEEL" "$DEST_NAME"; then
        FAILED_BUILDS+=("$LIB_NAME-$PLATFORM-py$PYVER (extraction failed)")
    fi
}

# ── Main ─────────────────────────────────────────────────────────────

START_TIME=$(date +%s)

if $BUILD_MACOS; then
    echo ""
    echo "=========================================="
    echo " macOS arm64 (native)"
    echo "=========================================="
    for PYVER in $PYTHON_VERSIONS; do
        build_variant "$PYVER" "prod" "macos"
        build_variant "$PYVER" "test" "macos"
    done
fi

if $BUILD_LINUX; then
    echo ""
    echo "=========================================="
    echo " Linux x86_64 (Docker)"
    echo "=========================================="
    echo " (May be slow on Apple Silicon due to emulation)"
    for PYVER in $PYTHON_VERSIONS; do
        build_variant "$PYVER" "prod" "linux"
        build_variant "$PYVER" "test" "linux"
    done
fi

END_TIME=$(date +%s)
ELAPSED=$(( END_TIME - START_TIME ))

echo ""
echo "=========================================="
echo " Build complete (${ELAPSED}s)"
echo "=========================================="

# Show results
echo ""
echo "Files in $PACKAGE_DIR:"
if ls "$PACKAGE_DIR/"*.so &>/dev/null; then
    printf "  %-60s %s\n" "FILE" "SIZE"
    for f in "$PACKAGE_DIR"/*.so; do
        SIZE=$(du -h "$f" | cut -f1)
        printf "  %-60s %s\n" "$(basename "$f")" "$SIZE"
    done
else
    echo "  (none)"
fi

MACOS_COUNT=$(ls "$PACKAGE_DIR/"*darwin*.so 2>/dev/null | wc -l | tr -d ' ')
LINUX_COUNT=$(ls "$PACKAGE_DIR/"*linux*.so 2>/dev/null | wc -l | tr -d ' ')
echo ""
echo "  macOS arm64:  $MACOS_COUNT / 6"
echo "  Linux x86_64: $LINUX_COUNT / 6"
echo "  Total:        $(( MACOS_COUNT + LINUX_COUNT )) / 12"

if [ ${#FAILED_BUILDS[@]} -gt 0 ]; then
    echo ""
    echo "Failed builds:"
    for f in "${FAILED_BUILDS[@]}"; do
        echo "  - $f"
    done
fi

echo ""
echo "Next steps:"
echo "  git add lean_multisig_py/*.so"
echo "  git commit -m 'update bindings for devnet4'"
echo "  git push"
