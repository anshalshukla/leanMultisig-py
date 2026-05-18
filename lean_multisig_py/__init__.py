"""Python bindings for lean-multisig XMSS aggregation (devnet5).

Provides Type-1 (single message + slot) and Type-2 (multiple messages)
XMSS multi-signatures via a Rust extension module. Wheels ship both prod
and test variants in the same package; pick at call time with `mode=`.
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

MODE = getattr(_module, "MODE", None) or getattr(_test_module, "MODE", None)


def get_mode() -> str:
    if _module is None and _test_module is None:
        raise RuntimeError("lean_multisig_py Rust module is not available")
    mod = _module or _test_module
    return mod.get_mode()


def _get_module(mode=None):
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


def aggregate_type_1(
    pub_keys_bytes,
    signatures_bytes,
    message_hash,
    slot,
    log_inv_rate,
    children_bytes=None,
    *,
    mode=None,
):
    """Aggregate raw XMSS signatures (and optional prior Type-1 children) into a Type-1 multi-signature.

    Args:
        pub_keys_bytes: List of SSZ-encoded XMSS public keys, paired with signatures_bytes.
        signatures_bytes: List of SSZ-encoded raw XMSS signatures.
        message_hash: 32-byte message hash.
        slot: Slot number.
        log_inv_rate: Inverse rate exponent for the proof.
        children_bytes: Optional list of `(child_pks_ssz, child_type1_bytes)` tuples.
        mode: 'prod', 'test', or None.

    Returns:
        Tuple `(sorted_pks_ssz, type1_bytes)`.
    """
    return _get_module(mode).aggregate_type_1(
        pub_keys_bytes, signatures_bytes, message_hash, slot, log_inv_rate, children_bytes
    )


def verify_type_1(pub_keys_bytes, message_hash, slot, sig_bytes, *, mode=None):
    """Verify a Type-1 multi-signature. Raises ValueError on any failure."""
    return _get_module(mode).verify_type_1(pub_keys_bytes, message_hash, slot, sig_bytes)


def merge_many_type_1(type1_entries, log_inv_rate, *, mode=None):
    """Merge multiple Type-1 multi-signatures into a single Type-2 multi-signature.

    Args:
        type1_entries: List of `(pub_keys_ssz, type1_bytes)` tuples, one per component.
        log_inv_rate: Inverse rate exponent for the proof.
        mode: 'prod', 'test', or None.

    Returns:
        Tuple `(pks_per_component_ssz, type2_bytes)`.
    """
    return _get_module(mode).merge_many_type_1(type1_entries, log_inv_rate)


def verify_type_2(pub_keys_per_component, sig_bytes, *, mode=None):
    """Verify a Type-2 multi-signature. Raises ValueError on any failure."""
    return _get_module(mode).verify_type_2(pub_keys_per_component, sig_bytes)


def verify_type_2_with_messages(
    pub_keys_per_component, expected_messages, sig_bytes, *, mode=None
):
    """Verify a Type-2 multi-signature and bind each component to a (message_hash, slot).

    Args:
        pub_keys_per_component: List of SSZ-encoded pubkey lists, one per component.
        expected_messages: List of `(message_hash, slot)` tuples, one per component,
            in the same order as `pub_keys_per_component`. `message_hash` is 32 bytes.
        sig_bytes: Type-2 signature bytes (compressed without pubkeys).
        mode: 'prod', 'test', or None.

    Raises:
        ValueError if the SNARK fails, the component count mismatches, or any
        component's (message, slot) does not match the expected pair.
    """
    return _get_module(mode).verify_type_2_with_messages(
        pub_keys_per_component, expected_messages, sig_bytes
    )


def split_type_2(pub_keys_per_component, sig_bytes, index, log_inv_rate, *, mode=None):
    """Extract component `index` from a Type-2 multi-signature as an independent Type-1."""
    return _get_module(mode).split_type_2(pub_keys_per_component, sig_bytes, index, log_inv_rate)


def split_type_2_by_msg(pub_keys_per_component, sig_bytes, message_hash, log_inv_rate, *, mode=None):
    """Extract the component with `message_hash` from a Type-2 multi-signature as an independent Type-1."""
    return _get_module(mode).split_type_2_by_msg(
        pub_keys_per_component, sig_bytes, message_hash, log_inv_rate
    )


def type1_compress_with_pubkeys(pub_keys_bytes, sig_bytes, *, mode=None):
    """Re-serialize a Type-1 multi-signature with pubkeys bundled into the blob.

    Input is the `(pks_ssz, type1_bytes)` shape returned by `aggregate_type_1`;
    output is a single self-contained blob (the upstream `compress()` form).
    """
    return _get_module(mode).type1_compress_with_pubkeys(pub_keys_bytes, sig_bytes)


def type1_decompress_with_pubkeys(sig_bytes, *, mode=None):
    """Split a self-contained Type-1 blob back into (pks_ssz, no-pubkeys-blob)."""
    return _get_module(mode).type1_decompress_with_pubkeys(sig_bytes)


def type1_compress_without_pubkeys(sig_bytes, *, mode=None):
    """Strip pubkeys from a self-contained Type-1 blob, returning only the compact wire form."""
    return _get_module(mode).type1_compress_without_pubkeys(sig_bytes)


def type2_compress_with_pubkeys(pub_keys_per_component, sig_bytes, *, mode=None):
    """Re-serialize a Type-2 multi-signature with pubkeys bundled into the blob."""
    return _get_module(mode).type2_compress_with_pubkeys(pub_keys_per_component, sig_bytes)


def type2_decompress_with_pubkeys(sig_bytes, *, mode=None):
    """Split a self-contained Type-2 blob back into (pks_per_component_ssz, no-pubkeys-blob)."""
    return _get_module(mode).type2_decompress_with_pubkeys(sig_bytes)


def type2_compress_without_pubkeys(sig_bytes, *, mode=None):
    """Strip pubkeys from a self-contained Type-2 blob, returning only the compact wire form."""
    return _get_module(mode).type2_compress_without_pubkeys(sig_bytes)


def ssz_encode_type1_signature(sig_bytes, *, mode=None):
    """SSZ-encode raw Type-1 signature bytes into the Devnet5Type1Signature container."""
    return _get_module(mode).ssz_encode_type1_signature(sig_bytes)


def ssz_decode_type1_signature(ssz_bytes, *, mode=None):
    """SSZ-decode a Devnet5Type1Signature container back to raw Type-1 signature bytes."""
    return _get_module(mode).ssz_decode_type1_signature(ssz_bytes)


def ssz_encode_type2_signature(sig_bytes, *, mode=None):
    """SSZ-encode raw Type-2 signature bytes into the Devnet5Type2Signature container."""
    return _get_module(mode).ssz_encode_type2_signature(sig_bytes)


def ssz_decode_type2_signature(ssz_bytes, *, mode=None):
    """SSZ-decode a Devnet5Type2Signature container back to raw Type-2 signature bytes."""
    return _get_module(mode).ssz_decode_type2_signature(ssz_bytes)


__all__ = [
    "MODE",
    "get_mode",
    "setup_prover",
    "setup_verifier",
    "aggregate_type_1",
    "verify_type_1",
    "merge_many_type_1",
    "verify_type_2",
    "verify_type_2_with_messages",
    "split_type_2",
    "split_type_2_by_msg",
    "type1_compress_with_pubkeys",
    "type1_decompress_with_pubkeys",
    "type1_compress_without_pubkeys",
    "type2_compress_with_pubkeys",
    "type2_decompress_with_pubkeys",
    "type2_compress_without_pubkeys",
    "ssz_encode_type1_signature",
    "ssz_decode_type1_signature",
    "ssz_encode_type2_signature",
    "ssz_decode_type2_signature",
]
