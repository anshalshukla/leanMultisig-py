"""Python bindings for lean-multisig XMSS aggregation (devnet5).

Provides single-message proofs (single message + slot) and multi-message
proofs (multiple messages) of XMSS signatures via a Rust extension module.
Wheels ship both prod and test variants in the same package; pick at call time
with `mode=`.
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


def aggregate_single_message(
    pub_keys_bytes,
    signatures_bytes,
    message_hash,
    slot,
    log_inv_rate,
    children_bytes=None,
    *,
    mode=None,
):
    """Aggregate raw XMSS signatures (and optional prior single-message proof children) into a single-message proof.

    Args:
        pub_keys_bytes: List of SSZ-encoded XMSS public keys, paired with signatures_bytes.
        signatures_bytes: List of SSZ-encoded raw XMSS signatures.
        message_hash: 32-byte message hash.
        slot: Slot number.
        log_inv_rate: Inverse rate exponent for the proof.
        children_bytes: Optional list of `(child_pks_ssz, child_single_message_proof_bytes)` tuples.
        mode: 'prod', 'test', or None.

    Returns:
        Tuple `(sorted_pks_ssz, single_message_proof_bytes)`.
    """
    return _get_module(mode).aggregate_single_message(
        pub_keys_bytes, signatures_bytes, message_hash, slot, log_inv_rate, children_bytes
    )


def verify_single_message_proof(pub_keys_bytes, message_hash, slot, sig_bytes, *, mode=None):
    """Verify a single-message proof. Raises ValueError on any failure."""
    return _get_module(mode).verify_single_message_proof(pub_keys_bytes, message_hash, slot, sig_bytes)


def merge_many_single_message_proof(single_message_proof_entries, log_inv_rate, *, mode=None):
    """Merge multiple single-message proofs into a single multi-message proof.

    Args:
        single_message_proof_entries: List of `(pub_keys_ssz, single_message_proof_bytes)` tuples, one per component.
        log_inv_rate: Inverse rate exponent for the proof.
        mode: 'prod', 'test', or None.

    Returns:
        Tuple `(pks_per_component_ssz, multi_message_proof_bytes)`.
    """
    return _get_module(mode).merge_many_single_message_proof(single_message_proof_entries, log_inv_rate)


def verify_multi_message_proof(pub_keys_per_component, sig_bytes, *, mode=None):
    """Verify a multi-message proof. Raises ValueError on any failure."""
    return _get_module(mode).verify_multi_message_proof(pub_keys_per_component, sig_bytes)


def verify_multi_message_proof_with_messages(
    pub_keys_per_component, expected_messages, sig_bytes, *, mode=None
):
    """Verify a multi-message proof and bind each component to a (message_hash, slot).

    Args:
        pub_keys_per_component: List of SSZ-encoded pubkey lists, one per component.
        expected_messages: List of `(message_hash, slot)` tuples, one per component,
            in the same order as `pub_keys_per_component`. `message_hash` is 32 bytes.
        sig_bytes: multi-message proof bytes (compressed without pubkeys).
        mode: 'prod', 'test', or None.

    Raises:
        ValueError if the SNARK fails, the component count mismatches, or any
        component's (message, slot) does not match the expected pair.
    """
    return _get_module(mode).verify_multi_message_proof_with_messages(
        pub_keys_per_component, expected_messages, sig_bytes
    )


def split_multi_message_proof(pub_keys_per_component, sig_bytes, index, log_inv_rate, *, mode=None):
    """Extract component `index` from a multi-message proof as an independent single-message proof."""
    return _get_module(mode).split_multi_message_proof(pub_keys_per_component, sig_bytes, index, log_inv_rate)


def split_multi_message_proof_by_message(pub_keys_per_component, sig_bytes, message_hash, log_inv_rate, *, mode=None):
    """Extract the component with `message_hash` from a multi-message proof as an independent single-message proof."""
    return _get_module(mode).split_multi_message_proof_by_message(
        pub_keys_per_component, sig_bytes, message_hash, log_inv_rate
    )


def single_message_proof_compress_with_pubkeys(pub_keys_bytes, sig_bytes, *, mode=None):
    """Re-serialize a single-message proof with pubkeys bundled into the blob.

    Input is the `(pks_ssz, single_message_proof_bytes)` shape returned by `aggregate_single_message`;
    output is a single self-contained blob (the upstream `compress()` form).
    """
    return _get_module(mode).single_message_proof_compress_with_pubkeys(pub_keys_bytes, sig_bytes)


def single_message_proof_decompress_with_pubkeys(sig_bytes, *, mode=None):
    """Split a self-contained single-message proof blob back into (pks_ssz, no-pubkeys-blob)."""
    return _get_module(mode).single_message_proof_decompress_with_pubkeys(sig_bytes)


def single_message_proof_compress_without_pubkeys(sig_bytes, *, mode=None):
    """Strip pubkeys from a self-contained single-message proof blob, returning only the compact wire form."""
    return _get_module(mode).single_message_proof_compress_without_pubkeys(sig_bytes)


def multi_message_proof_compress_with_pubkeys(pub_keys_per_component, sig_bytes, *, mode=None):
    """Re-serialize a multi-message proof with pubkeys bundled into the blob."""
    return _get_module(mode).multi_message_proof_compress_with_pubkeys(pub_keys_per_component, sig_bytes)


def multi_message_proof_decompress_with_pubkeys(sig_bytes, *, mode=None):
    """Split a self-contained multi-message proof blob back into (pks_per_component_ssz, no-pubkeys-blob)."""
    return _get_module(mode).multi_message_proof_decompress_with_pubkeys(sig_bytes)


def multi_message_proof_compress_without_pubkeys(sig_bytes, *, mode=None):
    """Strip pubkeys from a self-contained multi-message proof blob, returning only the compact wire form."""
    return _get_module(mode).multi_message_proof_compress_without_pubkeys(sig_bytes)


def ssz_encode_single_message_proof(sig_bytes, *, mode=None):
    """SSZ-encode raw single-message proof bytes into the Devnet5SingleMessageProof container."""
    return _get_module(mode).ssz_encode_single_message_proof(sig_bytes)


def ssz_decode_single_message_proof(ssz_bytes, *, mode=None):
    """SSZ-decode a Devnet5SingleMessageProof container back to raw single-message proof bytes."""
    return _get_module(mode).ssz_decode_single_message_proof(ssz_bytes)


def ssz_encode_multi_message_proof(sig_bytes, *, mode=None):
    """SSZ-encode raw multi-message proof bytes into the Devnet5MultiMessageProof container."""
    return _get_module(mode).ssz_encode_multi_message_proof(sig_bytes)


def ssz_decode_multi_message_proof(ssz_bytes, *, mode=None):
    """SSZ-decode a Devnet5MultiMessageProof container back to raw multi-message proof bytes."""
    return _get_module(mode).ssz_decode_multi_message_proof(ssz_bytes)


__all__ = [
    "MODE",
    "get_mode",
    "setup_prover",
    "setup_verifier",
    "aggregate_single_message",
    "verify_single_message_proof",
    "merge_many_single_message_proof",
    "verify_multi_message_proof",
    "verify_multi_message_proof_with_messages",
    "split_multi_message_proof",
    "split_multi_message_proof_by_message",
    "single_message_proof_compress_with_pubkeys",
    "single_message_proof_decompress_with_pubkeys",
    "single_message_proof_compress_without_pubkeys",
    "multi_message_proof_compress_with_pubkeys",
    "multi_message_proof_decompress_with_pubkeys",
    "multi_message_proof_compress_without_pubkeys",
    "ssz_encode_single_message_proof",
    "ssz_decode_single_message_proof",
    "ssz_encode_multi_message_proof",
    "ssz_decode_multi_message_proof",
]
