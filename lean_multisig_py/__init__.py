"""Python bindings for lean-multisig XMSS aggregation (devnet4).

Provides XMSS signature aggregation and verification via a Rust extension module.
"""

from __future__ import annotations

try:
    from lean_multisig_py import lean_multisig as _module
except ImportError:
    _module = None


def setup_prover() -> None:
    """Pre-compute prover tables. Call once before first aggregation."""
    if _module is None:
        raise RuntimeError("lean_multisig_py Rust module is not available")
    _module.setup_prover()


def setup_verifier() -> None:
    """Pre-compute verifier tables. Call once before first verification."""
    if _module is None:
        raise RuntimeError("lean_multisig_py Rust module is not available")
    _module.setup_verifier()


def aggregate_signatures(
    pub_keys_bytes,
    signatures_bytes,
    message_hash,
    slot,
    log_inv_rate,
    *,
    children_bytes=None,
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

    Returns:
        Serialized aggregated signature as bytes.
    """
    if _module is None:
        raise RuntimeError("lean_multisig_py Rust module is not available")
    return _module.aggregate_signatures(
        pub_keys_bytes, signatures_bytes, message_hash, slot, log_inv_rate, children_bytes
    )


def verify_aggregated_signatures(
    message_hash,
    agg_signature_bytes,
    slot,
):
    """
    Verify aggregated XMSS signatures.

    Args:
        message_hash: 32-byte message hash.
        agg_signature_bytes: Serialized aggregated signature as bytes.
        slot: Slot number.

    Raises:
        ValueError: If verification fails.
    """
    if _module is None:
        raise RuntimeError("lean_multisig_py Rust module is not available")
    return _module.verify_aggregated_signatures(message_hash, agg_signature_bytes, slot)


def ssz_encode_aggregate_signature(agg_signature_bytes):
    """SSZ-encode an aggregated signature."""
    if _module is None:
        raise RuntimeError("lean_multisig_py Rust module is not available")
    return _module.ssz_encode_aggregate_signature(agg_signature_bytes)


def ssz_decode_aggregate_signature(ssz_bytes):
    """SSZ-decode an aggregated signature."""
    if _module is None:
        raise RuntimeError("lean_multisig_py Rust module is not available")
    return _module.ssz_decode_aggregate_signature(ssz_bytes)


__all__ = [
    "setup_prover",
    "setup_verifier",
    "aggregate_signatures",
    "verify_aggregated_signatures",
    "ssz_encode_aggregate_signature",
    "ssz_decode_aggregate_signature",
]
