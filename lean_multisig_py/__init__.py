"""Python bindings for lean-multisig XMSS aggregation.

Two Rust extension modules are bundled:

- `lean_multisig_py.prod`: Production parameters with full security.
- `lean_multisig_py.test`: Fast test parameters for development.

You can import either module directly or call the top-level helpers with
`test_mode=True/False` (or `mode="test"/"prod"`) to select the desired config.
"""

from __future__ import annotations

from types import ModuleType
from typing import Optional

# Import prod and test modules
try:
    from lean_multisig_py import lean_multisig_prod as prod
except ImportError:  # pragma: no cover - exercised when one module is absent.
    prod = None

try:
    from lean_multisig_py import lean_multisig_test as test
except ImportError:  # pragma: no cover - exercised when one module is absent.
    test = None

DEFAULT_MODE: str
if test is not None:
    DEFAULT_MODE = "test"
elif prod is not None:
    DEFAULT_MODE = "prod"
else:
    DEFAULT_MODE = "test"

# Backwards-compatible constant for the default module.
MODE = DEFAULT_MODE


def available_modes() -> tuple[str, ...]:
    """Return a tuple describing which Rust modules are loaded."""
    modes = []
    if prod is not None:
        modes.append("prod")
    if test is not None:
        modes.append("test")
    return tuple(modes)


def _select_module(*, test_mode: Optional[bool] = None, mode: Optional[str] = None) -> ModuleType:
    """
    Select the prod or test Rust module.

    Args:
        test_mode: Legacy flag. True selects test config, False selects prod.
        mode: Case-insensitive string ("test"/"prod") to pick a module.

    Returns:
        The requested module object.

    Raises:
        ValueError: If both selection flags are provided or invalid.
        RuntimeError: If the requested module is unavailable.
    """
    if test_mode is not None and mode is not None:
        raise ValueError("Provide either `test_mode` or `mode`, not both.")

    normalized_mode: Optional[str] = None
    if mode is not None:
        normalized_mode = mode.lower()
        if normalized_mode not in ("test", "prod"):
            raise ValueError("mode must be 'test' or 'prod'")
    elif test_mode is not None:
        normalized_mode = "test" if test_mode else "prod"
    else:
        normalized_mode = DEFAULT_MODE

    if normalized_mode == "test":
        module = test
    elif normalized_mode == "prod":
        module = prod
    else:
        module = None

    if module is None:
        if not available_modes():
            raise RuntimeError("lean_multisig_py was built without any Rust modules")
        raise RuntimeError(f"Requested XMSS module '{normalized_mode}' is unavailable")

    return module


def get_multisig_module(*, test_mode: Optional[bool] = None, mode: Optional[str] = None) -> ModuleType:
    """Public helper used by leanSpec to grab the requested Rust module."""
    return _select_module(test_mode=test_mode, mode=mode)


def setup_prover(*, test_mode: Optional[bool] = None, mode: Optional[str] = None) -> None:
    """Pre-compute prover tables for the requested mode."""
    module = _select_module(test_mode=test_mode, mode=mode)
    module.setup_prover()


def setup_verifier(*, test_mode: Optional[bool] = None, mode: Optional[str] = None) -> None:
    """Pre-compute verifier tables for the requested mode."""
    module = _select_module(test_mode=test_mode, mode=mode)
    module.setup_verifier()


def aggregate_signatures(
    pub_keys_bytes,
    signatures_bytes,
    message_hash,
    epoch,
    *,
    test_mode: Optional[bool] = None,
    mode: Optional[str] = None,
):
    """
    Aggregate XMSS signatures using the requested configuration.

    `test_mode` mirrors the old API. The new `mode` kwarg accepts "test" or "prod".
    """
    module = _select_module(test_mode=test_mode, mode=mode)
    return module.aggregate_signatures(pub_keys_bytes, signatures_bytes, message_hash, epoch)


def verify_aggregated_signatures(
    pub_keys_bytes,
    message_hash,
    agg_signature_bytes,
    epoch,
    *,
    test_mode: Optional[bool] = None,
    mode: Optional[str] = None,
):
    """Verify aggregated XMSS signatures using the requested configuration."""
    module = _select_module(test_mode=test_mode, mode=mode)
    return module.verify_aggregated_signatures(pub_keys_bytes, message_hash, agg_signature_bytes, epoch)


def get_mode(*, test_mode: Optional[bool] = None, mode: Optional[str] = None) -> str:
    """Return the mode label ("test" or "prod") for the selected module."""
    module = _select_module(test_mode=test_mode, mode=mode)
    return module.get_mode()


__all__ = [
    "prod",
    "test",
    "MODE",
    "available_modes",
    "get_multisig_module",
    "setup_prover",
    "setup_verifier",
    "aggregate_signatures",
    "verify_aggregated_signatures",
    "get_mode",
]
