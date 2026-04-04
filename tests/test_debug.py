"""Test cases for lean-multisig-py bindings."""

import pytest


def _has_module():
    import lean_multisig_py as lm
    return lm._module is not None or lm._test_module is not None


def test_module_import():
    """Test that the module can be imported."""
    import lean_multisig_py as lm

    assert hasattr(lm, "setup_prover")
    assert hasattr(lm, "setup_verifier")
    assert hasattr(lm, "aggregate_signatures")
    assert hasattr(lm, "verify_aggregated_signatures")
    assert hasattr(lm, "get_mode")
    assert hasattr(lm, "MODE")


def test_mode():
    """Test that MODE is set correctly."""
    import lean_multisig_py as lm

    if not _has_module():
        pytest.skip("Rust module not built")

    assert lm.MODE in ("prod", "test")
    assert lm.get_mode() in ("prod", "test")


def test_setup():
    """Test setup functions don't raise."""
    import lean_multisig_py as lm

    if not _has_module():
        pytest.skip("Rust module not built")

    lm.setup_prover()
    lm.setup_verifier()


def test_aggregate_signatures_validation():
    """Test input validation for aggregate_signatures."""
    import lean_multisig_py as lm

    if not _has_module():
        pytest.skip("Rust module not built")

    # Test message_hash length validation
    with pytest.raises(ValueError, match="message_hash must be exactly"):
        lm.aggregate_signatures(
            [b"pubkey1"],
            [b"sig1"],
            b"short",  # Not 32 bytes
            1,
            2,
        )

    # Test mismatched pub_keys and signatures
    with pytest.raises(ValueError, match="must match"):
        lm.aggregate_signatures(
            [b"pubkey1", b"pubkey2"],
            [b"sig1"],  # Only one signature
            b"a" * 32,
            1,
            2,
        )


def test_verify_signatures_validation():
    """Test input validation for verify_aggregated_signatures."""
    import lean_multisig_py as lm

    if not _has_module():
        pytest.skip("Rust module not built")

    # Test message_hash length validation
    with pytest.raises(ValueError, match="message_hash must be exactly"):
        lm.verify_aggregated_signatures(
            [b"short"],  # Not 32 bytes
            b"agg_sig",
            [],
            1,
        )


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
