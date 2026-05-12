"""Smoke tests for lean-multisig-py (devnet5) bindings."""

import pytest


def _has_module():
    import lean_multisig_py as lm
    return lm._module is not None or lm._test_module is not None


def test_module_import():
    import lean_multisig_py as lm

    assert hasattr(lm, "setup_prover")
    assert hasattr(lm, "setup_verifier")
    assert hasattr(lm, "aggregate_type_1")
    assert hasattr(lm, "verify_type_1")
    assert hasattr(lm, "merge_many_type_1")
    assert hasattr(lm, "verify_type_2")
    assert hasattr(lm, "split_type_2")
    assert hasattr(lm, "split_type_2_by_msg")
    assert hasattr(lm, "type1_compress_with_pubkeys")
    assert hasattr(lm, "type1_decompress_with_pubkeys")
    assert hasattr(lm, "type2_compress_with_pubkeys")
    assert hasattr(lm, "type2_decompress_with_pubkeys")
    assert hasattr(lm, "ssz_encode_type1_signature")
    assert hasattr(lm, "ssz_decode_type1_signature")
    assert hasattr(lm, "ssz_encode_type2_signature")
    assert hasattr(lm, "ssz_decode_type2_signature")
    assert hasattr(lm, "get_mode")
    assert hasattr(lm, "MODE")


def test_mode():
    import lean_multisig_py as lm

    if not _has_module():
        pytest.skip("Rust module not built")

    assert lm.MODE in ("prod", "test")
    assert lm.get_mode() in ("prod", "test")


def test_setup():
    import lean_multisig_py as lm

    if not _has_module():
        pytest.skip("Rust module not built")

    lm.setup_verifier()


def test_aggregate_type_1_validation():
    import lean_multisig_py as lm

    if not _has_module():
        pytest.skip("Rust module not built")

    with pytest.raises(ValueError, match="message_hash must be exactly"):
        lm.aggregate_type_1(
            [b"pubkey1"],
            [b"sig1"],
            b"short",
            1,
            2,
        )

    with pytest.raises(ValueError, match="must match"):
        lm.aggregate_type_1(
            [b"pubkey1", b"pubkey2"],
            [b"sig1"],
            b"a" * 32,
            1,
            2,
        )


def test_verify_type_1_validation():
    import lean_multisig_py as lm

    if not _has_module():
        pytest.skip("Rust module not built")

    with pytest.raises(ValueError, match="message_hash must be exactly"):
        lm.verify_type_1([b"pk"], b"short", 1, b"sig")


def test_ssz_roundtrip_type1():
    import lean_multisig_py as lm

    if not _has_module():
        pytest.skip("Rust module not built")

    payload = b"\x01\x02\x03\x04"
    encoded = lm.ssz_encode_type1_signature(payload)
    assert lm.ssz_decode_type1_signature(encoded) == payload


def test_ssz_roundtrip_type2():
    import lean_multisig_py as lm

    if not _has_module():
        pytest.skip("Rust module not built")

    payload = b"\x10\x20\x30"
    encoded = lm.ssz_encode_type2_signature(payload)
    assert lm.ssz_decode_type2_signature(encoded) == payload


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
