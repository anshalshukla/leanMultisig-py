#!/usr/bin/env python3
"""Build both prod and test variants of the lean-multisig extension for multiple Python versions.

Usage:
    python build_all.py                    # Build for all Python versions (3.12, 3.13, 3.14)
    python build_all.py --python 3.12      # Build for specific Python version only
    python build_all.py --wheel            # Build a distributable wheel (after building .so files)
"""

import argparse
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

# Python versions to build for
DEFAULT_PYTHON_VERSIONS = ["3.12", "3.13", "3.14"]


def find_maturin() -> str:
    """Find the maturin executable, bypassing asdf shims."""
    # Priority order:
    # 1. Cargo-installed maturin (most reliable, not affected by asdf)
    # 2. Direct path in asdf Python installations
    # 3. pipx installed maturin
    # 4. Fall back to shutil.which (might be asdf shim)
    
    candidates = [
        # Cargo install location
        Path.home() / ".cargo" / "bin" / "maturin",
        # pipx location
        Path.home() / ".local" / "bin" / "maturin",
        # Common system locations
        Path("/usr/local/bin/maturin"),
        Path("/opt/homebrew/bin/maturin"),
    ]
    
    # Also check asdf Python installations directly (bypass shim)
    asdf_python_dir = Path.home() / ".asdf" / "installs" / "python"
    if asdf_python_dir.exists():
        for python_dir in asdf_python_dir.iterdir():
            maturin_path = python_dir / "bin" / "maturin"
            if maturin_path.exists():
                candidates.insert(0, maturin_path)  # Prefer these
    
    for candidate in candidates:
        if candidate.exists():
            return str(candidate)
    
    # Last resort: use shutil.which but warn if it's an asdf shim
    maturin_path = shutil.which("maturin")
    if maturin_path:
        if ".asdf/shims" in maturin_path:
            print(f"WARNING: Found asdf shim at {maturin_path}, this may cause issues")
            print("Consider installing maturin via: cargo install maturin")
        return maturin_path
    
    raise RuntimeError(
        "maturin not found! Install it with one of:\n"
        "  cargo install maturin\n"
        "  pip install maturin\n"
        "  pipx install maturin"
    )


def find_python_interpreter(version: str) -> str | None:
    """Find a Python interpreter for the given version."""
    # Try common interpreter names
    candidates = [
        f"python{version}",
        f"python{version.replace('.', '')}",  # python312
    ]
    
    for candidate in candidates:
        try:
            result = subprocess.run(
                [candidate, "--version"],
                capture_output=True,
                text=True,
            )
            if result.returncode == 0:
                actual_version = result.stdout.strip() or result.stderr.strip()
                if version in actual_version:
                    return candidate
        except FileNotFoundError:
            # Interpreter not found, try next candidate
            continue
    
    return None


def get_so_extension_for_version(version: str) -> str:
    """Get the platform-specific shared library extension for a Python version."""
    major, minor = version.split(".")
    if sys.platform == "darwin":
        return f".cpython-{major}{minor}-darwin.so"
    elif sys.platform == "win32":
        return f".cp{major}{minor}-win_amd64.pyd"
    else:
        return f".cpython-{major}{minor}-x86_64-linux-gnu.so"


def update_cargo_lib_name(lib_name: str) -> str:
    """Update Cargo.toml [lib] name and return original content."""
    original = CARGO_TOML.read_text()
    # Handle both possible current names
    for current_name in ["lean_multisig", "lean_multisig_prod", "lean_multisig_test"]:
        if f'name = "{current_name}"' in original:
            updated = original.replace(
                f'name = "{current_name}"',
                f'name = "{lib_name}"'
            )
            CARGO_TOML.write_text(updated)
            return original
    
    # If no match found, try generic replacement
    import re
    updated = re.sub(r'name = "lean_multisig[^"]*"', f'name = "{lib_name}"', original)
    CARGO_TOML.write_text(updated)
    return original


def build_variant(features: list[str], lib_name: str, python_interpreter: str, python_version: str) -> Path | None:
    """Build a single variant with maturin for a specific Python version."""
    print(f"\n{'='*60}")
    print(f"Building {lib_name} for Python {python_version}...")
    print(f"{'='*60}\n")

    # Update Cargo.toml to use correct lib name
    original_cargo = update_cargo_lib_name(lib_name)

    try:
        # Find and use maturin directly (avoids asdf Python version switching issues)
        maturin = find_maturin()
        cmd = [maturin, "build", "--release", "-i", python_interpreter]
        if features:
            cmd.extend(["--features", ",".join(features)])

        env = os.environ.copy()
        env["RUSTFLAGS"] = env.get("RUSTFLAGS", "") + " -C target-cpu=native"

        result = subprocess.run(cmd, env=env)
        if result.returncode != 0:
            print(f"Failed to build {lib_name} for Python {python_version}")
            return None

        # Find the built wheel for this Python version
        version_tag = python_version.replace(".", "")
        wheels = sorted(
            [w for w in WHEELS_DIR.glob("*.whl") if f"cp{version_tag}" in w.name],
            key=lambda p: p.stat().st_mtime
        )
        if not wheels:
            print(f"No wheel found for Python {python_version}!")
            return None

        wheel = wheels[-1]
        print(f"Built wheel: {wheel}")

        # Extract the .so file from the wheel
        ext = get_so_extension_for_version(python_version)
        so_dest = PACKAGE_DIR / f"{lib_name}{ext}"

        with zipfile.ZipFile(wheel, "r") as zf:
            for name in zf.namelist():
                # Match .so or .pyd files
                if ".cpython-" in name or ".cp" in name:
                    if name.endswith(".so") or name.endswith(".pyd"):
                        with zf.open(name) as src:
                            so_dest.write_bytes(src.read())
                        print(f"Extracted: {so_dest}")
                        return so_dest

        print(f"Could not find .so file in wheel for Python {python_version}")
        return None

    finally:
        # Restore original Cargo.toml
        CARGO_TOML.write_text(original_cargo)


def create_combined_wheel():
    """Create a wheel containing all .so variants."""
    # Check that we have at least some .so files
    so_files = list(PACKAGE_DIR.glob("lean_multisig_*.so")) + list(PACKAGE_DIR.glob("lean_multisig_*.pyd"))
    
    if not so_files:
        print("No .so files found. Run without --wheel first.")
        sys.exit(1)
    
    print(f"Found {len(so_files)} .so files to include in wheel")
    
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
    parser = argparse.ArgumentParser(description="Build lean-multisig-py for multiple Python versions")
    parser.add_argument(
        "--python", "-p",
        type=str,
        action="append",
        help="Python version to build for (e.g., 3.12). Can be specified multiple times. Default: 3.12, 3.13, 3.14"
    )
    parser.add_argument(
        "--wheel",
        action="store_true",
        help="Create a distributable wheel after building"
    )
    parser.add_argument(
        "--clean",
        action="store_true",
        help="Clean all existing .so files before building"
    )
    args = parser.parse_args()
    
    # Show which maturin we're using
    maturin = find_maturin()
    print(f"Using maturin: {maturin}")
    
    python_versions = args.python if args.python else DEFAULT_PYTHON_VERSIONS
    
    # Find available Python interpreters
    available_interpreters = {}
    missing_versions = []
    
    for version in python_versions:
        interpreter = find_python_interpreter(version)
        if interpreter:
            available_interpreters[version] = interpreter
            print(f"Found Python {version}: {interpreter}")
        else:
            missing_versions.append(version)
            print(f"WARNING: Python {version} not found, skipping")
    
    if not available_interpreters:
        print("\nERROR: No Python interpreters found!")
        print("Please install Python 3.12, 3.13, or 3.14")
        sys.exit(1)
    
    if missing_versions:
        print(f"\nWill skip versions: {', '.join(missing_versions)}")
    
    # Clean existing .so files if requested
    if args.clean:
        print("\nCleaning existing .so files...")
        for so_file in PACKAGE_DIR.glob("lean_multisig_*.so"):
            so_file.unlink()
            print(f"  Removed: {so_file.name}")
        for so_file in PACKAGE_DIR.glob("lean_multisig_*.pyd"):
            so_file.unlink()
            print(f"  Removed: {so_file.name}")
    
    # Build for each Python version
    built_files = []
    failed_builds = []
    
    for version, interpreter in available_interpreters.items():
        # Build prod (no features)
        result = build_variant([], "lean_multisig_prod", interpreter, version)
        if result:
            built_files.append(result)
        else:
            failed_builds.append(f"lean_multisig_prod (Python {version})")
        
        # Build test (with test_config feature)
        result = build_variant(["test_config"], "lean_multisig_test", interpreter, version)
        if result:
            built_files.append(result)
        else:
            failed_builds.append(f"lean_multisig_test (Python {version})")
    
    # Summary
    print("\n" + "=" * 60)
    print("Build Summary")
    print("=" * 60)
    
    if built_files:
        print(f"\nSuccessfully built {len(built_files)} files:")
        for so_path in sorted(built_files):
            size_mb = so_path.stat().st_size / 1024 / 1024
            print(f"  {so_path.name} ({size_mb:.1f} MB)")
    
    if failed_builds:
        print(f"\nFailed builds ({len(failed_builds)}):")
        for name in failed_builds:
            print(f"  {name}")
    
    # List all .so files in package directory
    print("\nAll .so files in package:")
    all_so_files = sorted(PACKAGE_DIR.glob("lean_multisig_*.so")) + sorted(PACKAGE_DIR.glob("lean_multisig_*.pyd"))
    for so_file in all_so_files:
        size_mb = so_file.stat().st_size / 1024 / 1024
        print(f"  {so_file.name} ({size_mb:.1f} MB)")
    
    if args.wheel:
        print("\nCreating combined wheel...")
        create_combined_wheel()
    else:
        print("\nTo create a distributable wheel, run:")
        print("  python build_all.py --wheel")
        print("\nTo install locally for development:")
        print("  pip install -e .")


if __name__ == "__main__":
    main()
