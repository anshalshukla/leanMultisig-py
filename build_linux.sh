#!/bin/bash
# Build .so files for both macOS and Linux x86_64
# Builds for Python 3.12, 3.13, 3.14

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PARENT_DIR="$(dirname "$SCRIPT_DIR")"
PACKAGE_DIR="$SCRIPT_DIR/lean_multisig_py"

# Check if leanMultisig exists
if [ ! -d "$PARENT_DIR/leanMultisig" ]; then
    echo "Error: ../leanMultisig not found"
    echo "Make sure leanMultisig repo is at $PARENT_DIR/leanMultisig"
    exit 1
fi

# Python versions to build for
PYTHON_VERSIONS="3.12 3.13 3.14"

# Find maturin (prefer cargo-installed version to avoid asdf issues)
find_maturin() {
    if [ -f "$HOME/.cargo/bin/maturin" ]; then
        echo "$HOME/.cargo/bin/maturin"
    elif command -v maturin &> /dev/null; then
        echo "maturin"
    else
        echo ""
    fi
}

MATURIN=$(find_maturin)
if [ -z "$MATURIN" ]; then
    echo "Error: maturin not found. Install with: cargo install maturin"
    exit 1
fi
echo "Using maturin: $MATURIN"

# Function to build a variant
build_variant() {
    local PYVER="$1"
    local VARIANT="$2"
    local PLATFORM="$3"  # "macos" or "linux"
    
    local PYVER_SHORT="${PYVER//./}"  # 3.12 -> 312
    
    if [ "$VARIANT" = "test" ]; then
        FEATURES="--features test_config"
        LIB_NAME="lean_multisig_test"
    else
        FEATURES=""
        LIB_NAME="lean_multisig_prod"
    fi
    
    echo ""
    echo "=============================================="
    echo "Building $LIB_NAME for $PLATFORM (Python $PYVER)..."
    echo "=============================================="
    
    # Update Cargo.toml with correct lib name
    cp "$SCRIPT_DIR/Cargo.toml" "$SCRIPT_DIR/Cargo.toml.bak"
    sed -i.tmp "s/name = \"lean_multisig\"/name = \"$LIB_NAME\"/" "$SCRIPT_DIR/Cargo.toml"
    rm -f "$SCRIPT_DIR/Cargo.toml.tmp"
    
    local BUILD_SUCCESS=false
    
    if [ "$PLATFORM" = "macos" ]; then
        # Build for macOS locally
        if command -v "python$PYVER" &> /dev/null; then
            RUSTFLAGS="-C target-cpu=native" $MATURIN build --release $FEATURES -i "python$PYVER" && BUILD_SUCCESS=true
        else
            echo "Warning: python$PYVER not found, skipping macOS build"
        fi
    else
        # Build for Linux using Docker
        docker run --rm \
            --platform linux/amd64 \
            -v "$SCRIPT_DIR":/io \
            -v "$PARENT_DIR/leanMultisig":/leanMultisig \
            -w /io \
            ghcr.io/pyo3/maturin:latest \
            build --release $FEATURES -i "python$PYVER" && BUILD_SUCCESS=true
    fi
    
    # Restore original Cargo.toml
    mv "$SCRIPT_DIR/Cargo.toml.bak" "$SCRIPT_DIR/Cargo.toml"
    
    if [ "$BUILD_SUCCESS" = true ]; then
        # Find and extract the wheel
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
        
        if [ -n "$WHEEL" ] && [ -f "$WHEEL" ]; then
            echo "Extracting from $WHEEL..."
            
            TMP_DIR=$(mktemp -d)
            unzip -o "$WHEEL" -d "$TMP_DIR" > /dev/null
            
            SO_FILE=$(find "$TMP_DIR" -name "*.so" -type f | head -1)
            if [ -n "$SO_FILE" ]; then
                DEST_NAME="${LIB_NAME}.cpython-${PYVER_SHORT}-${DEST_SUFFIX}"
                cp "$SO_FILE" "$PACKAGE_DIR/$DEST_NAME"
                echo "Created: $DEST_NAME"
            else
                echo "Warning: No .so file found in wheel"
            fi
            
            rm -rf "$TMP_DIR"
        else
            echo "Warning: No wheel found for $PLATFORM Python $PYVER"
        fi
    fi
}

# Build all combinations
echo "=========================================="
echo "Building for macOS (native)..."
echo "=========================================="

for PYVER in $PYTHON_VERSIONS; do
    build_variant "$PYVER" "prod" "macos"
    build_variant "$PYVER" "test" "macos"
done

echo ""
echo "=========================================="
echo "Building for Linux x86_64 (Docker)..."
echo "=========================================="
echo "(This may take a while on Apple Silicon due to emulation)"

for PYVER in $PYTHON_VERSIONS; do
    build_variant "$PYVER" "prod" "linux"
    build_variant "$PYVER" "test" "linux"
done

echo ""
echo "=============================================="
echo "Build complete! All .so files:"
echo "=============================================="
ls -la "$PACKAGE_DIR/"*.so 2>/dev/null | awk '{print $NF, "(" $5/1024/1024 " MB)"}' | sed 's|.*/||' || echo "No .so files found"

echo ""
echo "Summary by platform:"
echo "  macOS (darwin):     $(ls "$PACKAGE_DIR/"*darwin*.so 2>/dev/null | wc -l | tr -d ' ') files"
echo "  Linux (x86_64):     $(ls "$PACKAGE_DIR/"*linux*.so 2>/dev/null | wc -l | tr -d ' ') files"

echo ""
echo "To commit and push:"
echo "  git add lean_multisig_py/*.so"
echo "  git commit -m 'Add macOS and Linux binaries for Python 3.12, 3.13, 3.14'"
echo "  git push"
