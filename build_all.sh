#!/bin/bash
# Build wheels for macOS arm64 (native) and Linux x86_64 (Docker).
# Produces one wheel per (python_version, platform) cell containing BOTH
# the prod and test-config .so files.
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
#   ./build_all.sh --clean      # remove old wheels first

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WHEELS_DIR="$SCRIPT_DIR/target/wheels"
PYTHON_VERSIONS="3.12 3.13 3.14"
BUILD_MACOS=true
BUILD_LINUX=true
CLEAN=false
FAILED_BUILDS=()

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

if ! command -v rustc &>/dev/null; then
    echo "Error: Rust not found. Install via https://rustup.rs"
    exit 1
fi
echo "  Rust: $(rustc --version)"

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
        echo "  Pulling maturin Docker image (if needed)..."
        docker pull --platform linux/amd64 ghcr.io/pyo3/maturin:latest 2>/dev/null || true
    fi
fi

if $CLEAN; then
    echo ""
    echo "Cleaning old wheels..."
    rm -rf "$WHEELS_DIR"
    echo "  Done."
fi

mkdir -p "$WHEELS_DIR"

# ── Helpers ──────────────────────────────────────────────────────────

resolve_python() {
    local PYVER="$1"
    local PYTHON_BIN=""
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
    if [ -z "$PYTHON_BIN" ]; then
        local SHIM
        SHIM=$(command -v "python$PYVER" 2>/dev/null || true)
        if [ -n "$SHIM" ] && [ -x "$SHIM" ]; then
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
    sed -i.tmp '/^\[lib\]/,/^\[/{s/^name = "lean_multisig"/name = "'"$LIB_NAME"'"/;}' "$SCRIPT_DIR/Cargo.toml"
    rm -f "$SCRIPT_DIR/Cargo.toml.tmp"
}

restore_cargo_toml() {
    if [ -f "$SCRIPT_DIR/Cargo.toml.bak" ]; then
        mv "$SCRIPT_DIR/Cargo.toml.bak" "$SCRIPT_DIR/Cargo.toml"
    fi
}

trap restore_cargo_toml EXIT

# ── Build one cell (prod wheel, then test wheel, then merge) ─────────

build_cell() {
    local PYVER="$1"
    local PLATFORM="$2"  # "macos" or "linux"
    local PYVER_SHORT="${PYVER//./}"

    echo ""
    echo "── Python $PYVER | $PLATFORM ──"

    local PYTHON_BIN=""
    if [ "$PLATFORM" = "macos" ]; then
        PYTHON_BIN=$(resolve_python "$PYVER")
        if [ -z "$PYTHON_BIN" ] || [ ! -x "$PYTHON_BIN" ]; then
            echo "  Skipped: python$PYVER not found"
            FAILED_BUILDS+=("py$PYVER-$PLATFORM (python not found)")
            return
        fi
        echo "  Interpreter: $PYTHON_BIN"
    fi

    # 1) Build prod wheel
    echo "  Building prod wheel..."
    local PROD_OK=false
    if [ "$PLATFORM" = "macos" ]; then
        if RUSTFLAGS="-C target-cpu=native" $MATURIN build --release -i "$PYTHON_BIN" 2>&1; then
            PROD_OK=true
        fi
    else
        docker run --rm --platform linux/amd64 \
            --entrypoint bash \
            -e RUSTFLAGS="-C target-cpu=x86-64-v3" \
            -v "$SCRIPT_DIR":/io -w /io \
            ghcr.io/pyo3/maturin:latest \
            -c "rm -rf target/release/.fingerprint/lean-multisig-py-* target/release/.fingerprint/leansig_wrapper-* target/release/.fingerprint/rec_aggregation-* target/release/deps/liblean_multisig* target/release/deps/libleansig_wrapper* target/release/deps/librec_aggregation* 2>/dev/null; maturin build --release -i python$PYVER" 2>&1 \
            && PROD_OK=true
    fi
    if ! $PROD_OK; then
        echo "  FAILED (prod)"
        FAILED_BUILDS+=("py$PYVER-$PLATFORM-prod")
        return
    fi

    # 2) Build test wheel into a separate scratch dir so it doesn't overwrite
    #    the prod wheel under target/wheels/.
    echo "  Building test wheel..."
    patch_cargo_toml "lean_multisig_test"
    local TEST_OK=false
    local TEST_SCRATCH="$SCRIPT_DIR/target/wheels-test"
    rm -rf "$TEST_SCRATCH"
    mkdir -p "$TEST_SCRATCH"
    if [ "$PLATFORM" = "macos" ]; then
        if RUSTFLAGS="-C target-cpu=native" $MATURIN build --release --features test-config -i "$PYTHON_BIN" --out "$TEST_SCRATCH" 2>&1; then
            TEST_OK=true
        fi
    else
        docker run --rm --platform linux/amd64 \
            --entrypoint bash \
            -e RUSTFLAGS="-C target-cpu=x86-64-v3" \
            -v "$SCRIPT_DIR":/io -w /io \
            ghcr.io/pyo3/maturin:latest \
            -c "rm -rf target/release/.fingerprint/lean-multisig-py-* target/release/.fingerprint/leansig_wrapper-* target/release/.fingerprint/rec_aggregation-* target/release/deps/liblean_multisig* target/release/deps/libleansig_wrapper* target/release/deps/librec_aggregation* 2>/dev/null; maturin build --release --features test-config -i python$PYVER --out target/wheels-test" 2>&1 \
            && TEST_OK=true
    fi
    restore_cargo_toml
    if ! $TEST_OK; then
        echo "  FAILED (test)"
        FAILED_BUILDS+=("py$PYVER-$PLATFORM-test")
        return
    fi

    # 3) Merge test .so into prod wheel
    local PROD_WHEEL TEST_WHEEL
    if [ "$PLATFORM" = "macos" ]; then
        PROD_WHEEL=$(ls -t "$WHEELS_DIR/"*cp${PYVER_SHORT}*macos*.whl 2>/dev/null | head -1)
        TEST_WHEEL=$(ls -t "$TEST_SCRATCH/"*cp${PYVER_SHORT}*macos*.whl 2>/dev/null | head -1)
    else
        PROD_WHEEL=$(ls -t "$WHEELS_DIR/"*cp${PYVER_SHORT}*manylinux*.whl 2>/dev/null | grep -v aarch64 | head -1)
        TEST_WHEEL=$(ls -t "$TEST_SCRATCH/"*cp${PYVER_SHORT}*manylinux*.whl 2>/dev/null | grep -v aarch64 | head -1)
    fi
    if [ -z "$PROD_WHEEL" ] || [ -z "$TEST_WHEEL" ]; then
        echo "  FAILED (wheel not found: prod=$PROD_WHEEL test=$TEST_WHEEL)"
        FAILED_BUILDS+=("py$PYVER-$PLATFORM-merge")
        return
    fi

    "$SCRIPT_DIR/scripts/merge_wheels.sh" "$PROD_WHEEL" "$TEST_WHEEL"
    rm -rf "$TEST_SCRATCH"
}

# ── Main ─────────────────────────────────────────────────────────────

START_TIME=$(date +%s)

if $BUILD_MACOS; then
    echo ""
    echo "=========================================="
    echo " macOS arm64 (native)"
    echo "=========================================="
    for PYVER in $PYTHON_VERSIONS; do
        build_cell "$PYVER" "macos"
    done
fi

if $BUILD_LINUX; then
    echo ""
    echo "=========================================="
    echo " Linux x86_64 (Docker)"
    echo "=========================================="
    echo " (May be slow on Apple Silicon due to emulation)"
    for PYVER in $PYTHON_VERSIONS; do
        build_cell "$PYVER" "linux"
    done
fi

END_TIME=$(date +%s)
ELAPSED=$(( END_TIME - START_TIME ))

echo ""
echo "=========================================="
echo " Build complete (${ELAPSED}s)"
echo "=========================================="

echo ""
echo "Wheels in $WHEELS_DIR:"
if ls "$WHEELS_DIR/"*.whl &>/dev/null; then
    for f in "$WHEELS_DIR"/*.whl; do
        SIZE=$(du -h "$f" | cut -f1)
        printf "  %-70s %s\n" "$(basename "$f")" "$SIZE"
    done
else
    echo "  (none)"
fi

if [ ${#FAILED_BUILDS[@]} -gt 0 ]; then
    echo ""
    echo "Failed builds:"
    for f in "${FAILED_BUILDS[@]}"; do
        echo "  - $f"
    done
fi

echo ""
echo "Next steps:"
echo "  ls target/wheels/"
echo "  # Test locally:"
echo "  pip install --force-reinstall target/wheels/lean_multisig_py-*.whl"
echo "  # Release: tag a version, GitHub Actions builds and attaches wheels to the release."
