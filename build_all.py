#!/usr/bin/env python3
"""Build both prod and test variants of the lean-multisig extension.

Usage:
    python build_all.py          # Build .so files into lean_multisig_py/
    python build_all.py --wheel  # Build a distributable wheel
"""

import os
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

ROOT_DIR = Path(__file__).parent
PACKAGE_DIR = ROOT_DIR / "lean_multisig_py"
CARGO_TOML = ROOT_DIR / "Cargo.toml"
WHEELS_DIR = ROOT_DIR / "target" / "wheels"
DIST_DIR = ROOT_DIR / "dist"


def get_so_extension() -> str:
    """Get the platform-specific shared library extension."""
    import sysconfig
    ext = sysconfig.get_config_var("EXT_SUFFIX")
    if ext:
        return ext
    # Fallback
    if sys.platform == "darwin":
        return f".cpython-{sys.version_info.major}{sys.version_info.minor}-darwin.so"
    elif sys.platform == "win32":
        return f".cp{sys.version_info.major}{sys.version_info.minor}-win_amd64.pyd"
    else:
        return f".cpython-{sys.version_info.major}{sys.version_info.minor}-x86_64-linux-gnu.so"


def update_cargo_lib_name(lib_name: str) -> str:
    """Update Cargo.toml [lib] name and return original content."""
    original = CARGO_TOML.read_text()
    updated = original.replace(
        'name = "lean_multisig"',
        f'name = "{lib_name}"'
    )
    CARGO_TOML.write_text(updated)
    return original


def build_variant(features: list[str], lib_name: str) -> Path:
    """Build a single variant with maturin and return path to .so file."""
    print(f"\n{'='*60}")
    print(f"Building {lib_name}...")
    print(f"{'='*60}\n")

    # Update Cargo.toml to use correct lib name
    original_cargo = update_cargo_lib_name(lib_name)

    try:
        # Use maturin build (doesn't require venv)
        cmd = ["maturin", "build", "--release"]
        if features:
            cmd.extend(["--features", ",".join(features)])

        env = os.environ.copy()
        env["RUSTFLAGS"] = env.get("RUSTFLAGS", "") + " -C target-cpu=native"

        result = subprocess.run(cmd, env=env)
        if result.returncode != 0:
            print(f"Failed to build {lib_name}")
            sys.exit(1)

        # Find the built wheel
        wheels = sorted(WHEELS_DIR.glob("*.whl"), key=lambda p: p.stat().st_mtime)
        if not wheels:
            print("No wheel found!")
            sys.exit(1)

        wheel = wheels[-1]
        print(f"Built wheel: {wheel}")

        # Extract the .so file from the wheel
        ext = get_so_extension()
        so_dest = PACKAGE_DIR / f"{lib_name}{ext}"

        with zipfile.ZipFile(wheel, "r") as zf:
            for name in zf.namelist():
                if name.endswith(ext):
                    with zf.open(name) as src:
                        so_dest.write_bytes(src.read())
                    print(f"Extracted: {so_dest}")
                    return so_dest

        print(f"Could not find .so file in wheel")
        sys.exit(1)

    finally:
        # Restore original Cargo.toml
        CARGO_TOML.write_text(original_cargo)


def create_combined_wheel():
    """Create a wheel containing both .so variants."""
    ext = get_so_extension()
    
    # Check both .so files exist
    prod_so = PACKAGE_DIR / f"lean_multisig_prod{ext}"
    test_so = PACKAGE_DIR / f"lean_multisig_test{ext}"
    
    if not prod_so.exists() or not test_so.exists():
        print("Both .so files must exist. Run without --wheel first.")
        sys.exit(1)
    
    # Use setuptools to build the wheel with the .so files included
    DIST_DIR.mkdir(exist_ok=True)
    
    # Build with setuptools
    result = subprocess.run([
        sys.executable, "-m", "build", "--wheel", "--outdir", str(DIST_DIR)
    ])
    
    if result.returncode != 0:
        print("Failed to build wheel")
        sys.exit(1)
    
    print(f"\nWheel created in {DIST_DIR}/")


def main():
    build_wheel = "--wheel" in sys.argv
    
    # Clean any previous .so files
    ext = get_so_extension()
    for variant in ["lean_multisig_prod", "lean_multisig_test", "lean_multisig"]:
        so_path = PACKAGE_DIR / f"{variant}{ext}"
        if so_path.exists():
            so_path.unlink()
            print(f"Removed old: {so_path}")

    # Build prod (no features)
    build_variant([], "lean_multisig_prod")

    # Build test (with test_config feature)
    build_variant(["test_config"], "lean_multisig_test")

    print("\n" + "=" * 60)
    print("Both variants built successfully!")
    print("=" * 60)

    # List what we built
    for variant in ["lean_multisig_prod", "lean_multisig_test"]:
        so_path = PACKAGE_DIR / f"{variant}{ext}"
        if so_path.exists():
            size_mb = so_path.stat().st_size / 1024 / 1024
            print(f"  {so_path.name} ({size_mb:.1f} MB)")
        else:
            print(f"  {variant} - NOT FOUND")

    if build_wheel:
        print("\nCreating combined wheel...")
        create_combined_wheel()
    else:
        print("\nTo create a distributable wheel, run:")
        print("  python build_all.py --wheel")
        print("\nTo install locally for development:")
        print("  pip install -e .")


if __name__ == "__main__":
    main()
