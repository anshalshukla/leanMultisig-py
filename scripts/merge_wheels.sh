#!/bin/bash
# Merge the test-config .so from a test wheel into a prod wheel.
#
# After: the prod wheel contains both lean_multisig.*.so AND
# lean_multisig_test.*.so, so `import lean_multisig_py` can offer both modes
# from a single installed wheel.
#
# Usage: merge_wheels.sh <prod_wheel.whl> <test_wheel.whl>
#   The prod wheel is rewritten in place (same filename, same metadata).

set -euo pipefail

PROD_WHEEL="$1"
TEST_WHEEL="$2"

# Resolve to absolute paths so `cd` into the work dir later doesn't break them.
case "$PROD_WHEEL" in /*) ;; *) PROD_WHEEL="$PWD/$PROD_WHEEL" ;; esac
case "$TEST_WHEEL" in /*) ;; *) TEST_WHEEL="$PWD/$TEST_WHEEL" ;; esac

if [ ! -f "$PROD_WHEEL" ]; then
    echo "Error: prod wheel not found: $PROD_WHEEL" >&2
    exit 1
fi
if [ ! -f "$TEST_WHEEL" ]; then
    echo "Error: test wheel not found: $TEST_WHEEL" >&2
    exit 1
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Unpack both wheels.
unzip -q "$PROD_WHEEL" -d "$WORK/prod"
unzip -q "$TEST_WHEEL" -d "$WORK/test"

# Find the test .so. Maturin's `module-name` in pyproject.toml fixes the filename
# at `lean_multisig.cpython-*.so` for *both* prod and test builds; the test build
# patches Cargo.toml's [lib] name so the PyInit symbol inside is
# `PyInit_lean_multisig_test`. We therefore look up the file inside the test
# wheel by its `lean_multisig.cpython-*` name and rename it to
# `lean_multisig_test.cpython-*` when copying into the prod wheel so Python's
# import machinery can locate the symbol.
TEST_SO=$(find "$WORK/test/lean_multisig_py" -maxdepth 1 -type f \( -name "lean_multisig.cpython-*.so" -o -name "lean_multisig.cp*-win_amd64.pyd" \) | head -1)
if [ -z "$TEST_SO" ]; then
    echo "Error: no lean_multisig.cpython-*.so found inside $TEST_WHEEL" >&2
    exit 1
fi

PROD_PKG=$(find "$WORK/prod" -type d -name "lean_multisig_py" | head -1)
if [ -z "$PROD_PKG" ]; then
    echo "Error: prod wheel does not contain a lean_multisig_py/ package" >&2
    exit 1
fi

TEST_SO_BASE=$(basename "$TEST_SO")
NEW_SO_NAME="${TEST_SO_BASE/lean_multisig./lean_multisig_test.}"
cp "$TEST_SO" "$PROD_PKG/$NEW_SO_NAME"

# Update RECORD (wheel-spec checksum manifest) so installers don't complain.
RECORD=$(find "$WORK/prod" -type f -path "*.dist-info/RECORD" | head -1)
if [ -n "$RECORD" ]; then
    NEW_SO_PATH="lean_multisig_py/$NEW_SO_NAME"
    NEW_SO_SIZE=$(wc -c < "$PROD_PKG/$NEW_SO_NAME" | tr -d ' ')
    NEW_SO_HASH=$(openssl dgst -sha256 -binary "$PROD_PKG/$NEW_SO_NAME" | openssl base64 -A | tr '+/' '-_' | tr -d '=')
    grep -v "^lean_multisig_py/lean_multisig_test" "$RECORD" > "$RECORD.tmp" || true
    mv "$RECORD.tmp" "$RECORD"
    printf '%s,sha256=%s,%s\n' "$NEW_SO_PATH" "$NEW_SO_HASH" "$NEW_SO_SIZE" >> "$RECORD"
fi

# Re-zip prod wheel in place.
(
    cd "$WORK/prod"
    rm -f "$PROD_WHEEL"
    zip -qr "$PROD_WHEEL" .
)

echo "Merged $NEW_SO_NAME into $(basename "$PROD_WHEEL")"
