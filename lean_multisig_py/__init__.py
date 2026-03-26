"""Python bindings for lean-multisig XMSS aggregation (devnet4).

Provides XMSS signature aggregation and verification via a Rust extension module.
Supports both prod and test configurations.

When installed via `maturin develop`, the module is always `lean_multisig`.
In release packages, both `lean_multisig` (prod) and `lean_multisig_test` (test)
.so files are included. Use `get_mode()` or `MODE` to check which config is active.
"""

from __future__ import annotations

_module = None
_test_module = None

try:
    from lean_multisig_py import lean_multisig as _module
except ImportError:
    pass

try:
    from lean_multisig_py import lean_multisig_test as _test_module
except ImportError:
    pass

# For backwards compat: if only test module is available (e.g. maturin develop --features test-config),
# it's named lean_multisig too, so _module will have it with MODE="test".
MODE = getattr(_module, "MODE", None) or getattr(_test_module, "MODE", None)


def get_mode() -> str:
    """Return the mode this module was compiled with: 'prod' or 'test'."""
    if _module is None and _test_module is None:
        raise RuntimeError("lean_multisig_py Rust module is not available")
    mod = _module or _test_module
    return mod.get_mode()


def _get_module(mode=None):
    """Get the appropriate Rust module.

    Args:
        mode: 'prod', 'test', or None (uses prod if available, else test).
    """
    if mode == "test":
        m = _test_module or (_module if _module is not None and _module.MODE == "test" else None)
        if m is None:
            raise RuntimeError("test-config module not available")
        return m
    if mode == "prod":
        m = _module if _module is not None and _module.MODE == "prod" else None
        if m is None:
            raise RuntimeError("prod module not available")
        return m
    # Default: prefer prod, fall back to test
    m = _module or _test_module
    if m is None:
        raise RuntimeError("lean_multisig_py Rust module is not available")
    return m


def setup_prover(*, mode=None) -> None:
    """Pre-compute prover tables. Call once before first aggregation."""
    _get_module(mode).setup_prover()


def setup_verifier(*, mode=None) -> None:
    """Pre-compute verifier tables. Call once before first verification."""
    _get_module(mode).setup_verifier()


def aggregate_signatures(
    pub_keys_bytes,
    signatures_bytes,
    message_hash,
    slot,
    log_inv_rate,
    *,
    children_bytes=None,
    mode=None,
):
    """
    Aggregate XMSS signatures.

    Args:
        pub_keys_bytes: List of serialized public keys (postcard format).
        signatures_bytes: List of serialized signatures (postcard format).
        message_hash: 32-byte message hash.
        slot: Slot number.
        log_inv_rate: Inverse rate exponent (1-4, lower = faster but bigger proofs).
        children_bytes: Optional list of serialized AggregatedXMSS for hierarchical aggregation.
        mode: 'prod', 'test', or None (default).

    Returns:
        Serialized aggregated signature as bytes.
    """
    return _get_module(mode).aggregate_signatures(
        pub_keys_bytes, signatures_bytes, message_hash, slot, log_inv_rate, children_bytes
    )


def verify_aggregated_signatures(
    message_hash,
    agg_signature_bytes,
    slot,
    *,
    mode=None,
):
    """
    Verify aggregated XMSS signatures.

    Args:
        message_hash: 32-byte message hash.
        agg_signature_bytes: Serialized aggregated signature as bytes.
        slot: Slot number.
        mode: 'prod', 'test', or None (default).

    Raises:
        ValueError: If verification fails.
    """
    return _get_module(mode).verify_aggregated_signatures(message_hash, agg_signature_bytes, slot)


def ssz_encode_aggregate_signature(agg_signature_bytes, *, mode=None):
    """SSZ-encode an aggregated signature."""
    return _get_module(mode).ssz_encode_aggregate_signature(agg_signature_bytes)


def ssz_decode_aggregate_signature(ssz_bytes, *, mode=None):
    """SSZ-decode an aggregated signature."""
    return _get_module(mode).ssz_decode_aggregate_signature(ssz_bytes)


__all__ = [
    "MODE",
    "get_mode",
    "setup_prover",
    "setup_verifier",
    "aggregate_signatures",
    "verify_aggregated_signatures",
    "ssz_encode_aggregate_signature",
    "ssz_decode_aggregate_signature",
]
