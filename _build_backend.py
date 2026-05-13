"""PEP 517 build backend that produces both prod and test variants.

Wraps the maturin build backend with a two-pass build:

1. Patch Cargo.toml's [lib] name to "lean_multisig_test".
2. Run maturin with `--features test-config` into a scratch wheel.
3. Extract the test .so, rename it to "lean_multisig_test.cpython-*.so",
   stage it inside lean_multisig_py/ so the next build picks it up.
4. Restore Cargo.toml.
5. Run maturin normally for the prod build. Its `[tool.maturin].include`
   directive bundles the staged test .so into the produced wheel.
6. Clean up the staged test .so.

The end result is a single wheel containing both Rust modules, even for
builds driven by `pip install` from a git source. The release CI in
.github/workflows/release.yml does an equivalent dance for binary
releases; this backend gives source installers the same artifact.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile
from collections.abc import Mapping
from contextlib import contextmanager
from pathlib import Path

import maturin

_REPO_ROOT: Path = Path(__file__).resolve().parent
_CARGO_TOML: Path = _REPO_ROOT / "Cargo.toml"
_PKG_DIR: Path = _REPO_ROOT / "lean_multisig_py"
_TEST_FEATURE: str = "test-config"


def _is_truthy(value: str | None) -> bool:
    """Treat the common shell-truthy strings as on."""
    if value is None:
        return False
    return value.strip().lower() in {"1", "true", "yes", "on"}


def _two_pass_disabled() -> bool:
    """Allow opt-out via env var for debugging or stripped-down builds."""
    return _is_truthy(os.environ.get("LEAN_MULTISIG_PY_PROD_ONLY"))


def _normalize_config_settings(
    config_settings: Mapping[str, object] | None,
) -> dict[str, list[str]]:
    """Normalize PEP 517 config_settings into a maturin-friendly shape.

    maturin reads `build-args` and `editable-args` keys; values may arrive
    as a single string or a list of strings depending on the front end.
    """
    normalized: dict[str, list[str]] = {}
    if not config_settings:
        return normalized
    for key, value in config_settings.items():
        if isinstance(value, str):
            normalized[key] = [value]
        elif isinstance(value, (list, tuple)):
            normalized[key] = [str(item) for item in value]
        else:
            normalized[key] = [str(value)]
    return normalized


def _flatten_args(args: list[str]) -> list[str]:
    """Split shell-style argument strings on whitespace.

    pip and uv tend to pass `build-args` as a single string like
    `"--release --features foo"` rather than a tokenised list.
    """
    flat: list[str] = []
    for arg in args:
        flat.extend(arg.split())
    return flat


def _ensure_test_feature(args: list[str]) -> list[str]:
    """Return args with `--features test-config` merged into any existing --features."""
    out: list[str] = []
    consumed_features = False

    i = 0
    while i < len(args):
        token = args[i]
        if token == "--features" and i + 1 < len(args):
            existing = args[i + 1].split(",") if args[i + 1] else []
            if _TEST_FEATURE not in existing:
                existing.append(_TEST_FEATURE)
            out.extend([token, ",".join(filter(None, existing))])
            i += 2
            consumed_features = True
            continue
        if token.startswith("--features="):
            existing = token.removeprefix("--features=").split(",")
            if _TEST_FEATURE not in existing:
                existing.append(_TEST_FEATURE)
            out.append("--features=" + ",".join(filter(None, existing)))
            i += 1
            consumed_features = True
            continue
        out.append(token)
        i += 1

    if not consumed_features:
        out.extend(["--features", _TEST_FEATURE])
    return out


@contextmanager
def _patched_lib_name(new_name: str):
    """Temporarily rewrite the Cargo.toml [lib] name to `new_name`.

    The release workflow does the same edit so that the test build's
    PyInit symbol becomes `PyInit_<new_name>`. We restore the original
    file on exit even if the build raises.
    """
    original = _CARGO_TOML.read_text()
    patched = re.sub(
        r'(\[lib\][^\[]*?name\s*=\s*)"lean_multisig"',
        rf'\1"{new_name}"',
        original,
        count=1,
        flags=re.DOTALL,
    )
    if patched == original:
        raise RuntimeError(
            "Custom build backend could not locate the [lib] name to patch in Cargo.toml. "
            "The default-feature build will run without the test variant."
        )
    _CARGO_TOML.write_text(patched)
    try:
        yield
    finally:
        _CARGO_TOML.write_text(original)


def _find_test_so_in_wheel(wheel_path: Path) -> tuple[bytes, str]:
    """Read the test .so from a built wheel, returning (bytes, new filename).

    The wheel's internal filename follows the prod naming convention
    (`lean_multisig.cpython-*.so` or `lean_multisig.cp*-win_amd64.pyd`)
    because `module-name` in pyproject.toml is shared. We rename it on
    the way out so Python's import machinery looks up the test
    PyInit symbol.
    """
    with zipfile.ZipFile(wheel_path) as zf:
        for name in zf.namelist():
            base = os.path.basename(name)
            if base.startswith("lean_multisig.") and (
                base.endswith(".so") or base.endswith(".pyd")
            ):
                data = zf.read(name)
                new_base = base.replace("lean_multisig.", "lean_multisig_test.", 1)
                return data, new_base

    raise RuntimeError(f"No lean_multisig.* native module found in test wheel {wheel_path}.")


def _stage_test_so(scratch_dir: Path) -> Path | None:
    """Locate the test wheel under scratch_dir and stage its .so for prod inclusion.

    Returns the staged path so the caller can clean up after the prod build.
    """
    wheels = sorted(scratch_dir.glob("*.whl"))
    if not wheels:
        raise RuntimeError(
            f"Test build produced no wheel under {scratch_dir}. "
            "Inspect maturin's output for the underlying error."
        )

    data, new_filename = _find_test_so_in_wheel(wheels[-1])
    staged = _PKG_DIR / new_filename
    staged.write_bytes(data)
    return staged


def _run_test_build(config_settings: dict[str, list[str]]) -> Path:
    """Run a maturin build of the test variant into a scratch directory.

    Returns the scratch directory path so the caller can find the wheel.
    """
    scratch = Path(tempfile.mkdtemp(prefix="lean-multisig-py-test-"))

    args = _flatten_args(config_settings.get("build-args", []))
    test_args = _ensure_test_feature(args)
    test_args = ["--release", *test_args] if "--release" not in test_args else test_args
    test_args += ["--out", str(scratch)]

    cmd = [
        sys.executable,
        "-m",
        "maturin",
        "build",
        *test_args,
    ]
    env = os.environ.copy()
    env.setdefault("CARGO_TERM_COLOR", "always")
    subprocess.run(cmd, check=True, cwd=str(_REPO_ROOT), env=env)
    return scratch


def _build_test_variant_and_stage(
    config_settings: dict[str, list[str]],
) -> Path | None:
    """Run the test build inside the patched lib name and stage the .so.

    Returns the staged .so path so it can be removed after the prod
    build completes. None if the two-pass build is disabled.
    """
    if _two_pass_disabled():
        return None

    with _patched_lib_name("lean_multisig_test"):
        scratch = _run_test_build(config_settings)
    try:
        return _stage_test_so(scratch)
    finally:
        shutil.rmtree(scratch, ignore_errors=True)


def _cleanup_staged(staged: Path | None) -> None:
    if staged is not None and staged.exists():
        staged.unlink()


def build_wheel(
    wheel_directory: str,
    config_settings: Mapping[str, object] | None = None,
    metadata_directory: str | None = None,
) -> str:
    """PEP 517 entrypoint: build prod wheel with the test .so merged in.

    The order matters: the prod build must run last so that the
    `[tool.maturin].include` directive picks up the staged test .so
    file. Skipping the test build (LEAN_MULTISIG_PY_PROD_ONLY=1) yields
    a prod-only wheel identical to a bare maturin build.
    """
    settings = _normalize_config_settings(config_settings)
    staged = _build_test_variant_and_stage(settings)
    try:
        return maturin.build_wheel(wheel_directory, config_settings, metadata_directory)
    finally:
        _cleanup_staged(staged)


def build_sdist(
    sdist_directory: str,
    config_settings: Mapping[str, object] | None = None,
) -> str:
    """Forward sdist generation to maturin unchanged."""
    return maturin.build_sdist(sdist_directory, config_settings)


def build_editable(
    wheel_directory: str,
    config_settings: Mapping[str, object] | None = None,
    metadata_directory: str | None = None,
) -> str:
    """Build an editable wheel with the same two-pass staging logic.

    Editable installs typically don't need the test variant but staging
    it costs little and keeps developer environments closer to release
    behaviour.
    """
    settings = _normalize_config_settings(config_settings)
    staged = _build_test_variant_and_stage(settings)
    try:
        return maturin.build_editable(wheel_directory, config_settings, metadata_directory)
    finally:
        _cleanup_staged(staged)


def get_requires_for_build_wheel(
    config_settings: Mapping[str, object] | None = None,
) -> list[str]:
    return maturin.get_requires_for_build_wheel(config_settings)


def get_requires_for_build_sdist(
    config_settings: Mapping[str, object] | None = None,
) -> list[str]:
    return maturin.get_requires_for_build_sdist(config_settings)


def get_requires_for_build_editable(
    config_settings: Mapping[str, object] | None = None,
) -> list[str]:
    return maturin.get_requires_for_build_editable(config_settings)


def prepare_metadata_for_build_wheel(
    metadata_directory: str,
    config_settings: Mapping[str, object] | None = None,
) -> str:
    return maturin.prepare_metadata_for_build_wheel(metadata_directory, config_settings)


def prepare_metadata_for_build_editable(
    metadata_directory: str,
    config_settings: Mapping[str, object] | None = None,
) -> str:
    return maturin.prepare_metadata_for_build_editable(metadata_directory, config_settings)
