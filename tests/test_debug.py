"""Test cases for lean-multisig-py bindings."""

import pytest


def test_module_import():
    """Test that the module can be imported."""
    import lean_multisig_py as lm

    assert hasattr(lm, "setup_prover")
    assert hasattr(lm, "setup_verifier")
    assert hasattr(lm, "aggregate_signatures")
    assert hasattr(lm, "verify_aggregated_signatures")


def test_setup():
    """Test setup functions don't raise."""
    import lean_multisig_py as lm

    if lm._module is None:
        pytest.skip("Rust module not built")

    lm.setup_prover()
    lm.setup_verifier()


def test_aggregate_signatures_validation():
    """Test input validation for aggregate_signatures."""
    import lean_multisig_py as lm

    if lm._module is None:
        pytest.skip("Rust module not built")

    # Test message_hash length validation
    with pytest.raises(ValueError, match="message_hash must be exactly 32 bytes"):
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

    if lm._module is None:
        pytest.skip("Rust module not built")

    # Test message_hash length validation
    with pytest.raises(ValueError, match="message_hash must be exactly 32 bytes"):
        lm.verify_aggregated_signatures(
            b"short",  # Not 32 bytes
            b"agg_sig",
            1,
        )


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
