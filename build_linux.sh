#!/bin/bash
# Build Linux x86_64 .so files using Docker
# Run this on macOS to generate Linux binaries for GitHub Actions

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PARENT_DIR="$(dirname "$SCRIPT_DIR")"

# Check if leanMultisig exists
if [ ! -d "$PARENT_DIR/leanMultisig" ]; then
    echo "Error: ../leanMultisig not found"
    echo "Make sure leanMultisig repo is at $PARENT_DIR/leanMultisig"
    exit 1
fi

echo "Building Linux x86_64 binaries using Docker..."
echo "(This may take a while on Apple Silicon due to emulation)"

# Python versions to build for
PYTHON_VERSIONS="3.12 3.13 3.14"

# Build for each Python version and variant
for PYVER in $PYTHON_VERSIONS; do
  PYVER_SHORT="${PYVER//./}"  # 3.12 -> 312
  
  for VARIANT in prod test; do
    echo ""
    echo "=============================================="
    echo "Building lean_multisig_$VARIANT for Linux x86_64 (Python $PYVER)..."
    echo "=============================================="
    
    if [ "$VARIANT" = "test" ]; then
        FEATURES="--features test_config"
        LIB_NAME="lean_multisig_test"
    else
        FEATURES=""
        LIB_NAME="lean_multisig_prod"
    fi
    
    # Create a temporary Cargo.toml with the right lib name
    cp "$SCRIPT_DIR/Cargo.toml" "$SCRIPT_DIR/Cargo.toml.bak"
    sed -i.tmp "s/name = \"lean_multisig\"/name = \"$LIB_NAME\"/" "$SCRIPT_DIR/Cargo.toml"
    rm -f "$SCRIPT_DIR/Cargo.toml.tmp"
    
    # Run maturin in Docker with x86_64 platform (for GitHub Actions compatibility)
    docker run --rm \
        --platform linux/amd64 \
        -v "$SCRIPT_DIR":/io \
        -v "$PARENT_DIR/leanMultisig":/leanMultisig \
        -w /io \
        ghcr.io/pyo3/maturin:latest \
        build --release $FEATURES -i "python$PYVER"
    
    # Restore original Cargo.toml
    mv "$SCRIPT_DIR/Cargo.toml.bak" "$SCRIPT_DIR/Cargo.toml"
    
    # Find the wheel (look for x86_64 linux wheel for this Python version)
    WHEEL=$(ls -t "$SCRIPT_DIR/target/wheels/"*cp${PYVER_SHORT}*linux*x86_64*.whl 2>/dev/null | head -1)
    if [ -z "$WHEEL" ]; then
        # Try manylinux pattern
        WHEEL=$(ls -t "$SCRIPT_DIR/target/wheels/"*cp${PYVER_SHORT}*manylinux*.whl 2>/dev/null | grep -v aarch64 | head -1)
    fi
    
    if [ -n "$WHEEL" ] && [ -f "$WHEEL" ]; then
        echo "Extracting from $WHEEL..."
        
        # Extract .so file - it might be in a subdirectory named after the lib
        TMP_DIR=$(mktemp -d)
        unzip -o "$WHEEL" -d "$TMP_DIR"
        
        # Find and copy the .so file
        SO_FILE=$(find "$TMP_DIR" -name "*.so" -type f | head -1)
        if [ -n "$SO_FILE" ]; then
            # Construct correct destination filename
            DEST_NAME="${LIB_NAME}.cpython-${PYVER_SHORT}-x86_64-linux-gnu.so"
            cp "$SO_FILE" "$SCRIPT_DIR/lean_multisig_py/$DEST_NAME"
            echo "Created: lean_multisig_py/$DEST_NAME"
        else
            echo "Warning: No .so file found in wheel"
        fi
        
        rm -rf "$TMP_DIR"
    else
        echo "Warning: No x86_64 Linux wheel found for $VARIANT Python $PYVER"
        echo "Available wheels:"
        ls -la "$SCRIPT_DIR/target/wheels/"*.whl 2>/dev/null || echo "  (none)"
    fi
  done
done

echo ""
echo "=============================================="
echo "Build complete! All .so files:"
echo "=============================================="
ls -la "$SCRIPT_DIR/lean_multisig_py/"*.so 2>/dev/null || echo "No .so files found"

echo ""
echo "To commit and push:"
echo "  git add lean_multisig_py/*.so"
echo "  git commit -m 'Add Linux x86_64 binaries'"
echo "  git push"
