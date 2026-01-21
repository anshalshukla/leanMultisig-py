"""Test cases for lean-multisig-py bindings."""

import pytest


def test_module_selection_prefers_test_when_available():
    """Default selection should prefer the test module."""
    import lean_multisig_py as lm

    if lm.test is None:
        pytest.skip("test module not built")

    assert lm.get_multisig_module() is lm.test
    assert lm.get_mode() == "test"
    assert lm.MODE == "test"


def test_module_selection_prod_flag():
    """Explicit prod selection should return the prod module."""
    import lean_multisig_py as lm

    if lm.prod is None:
        pytest.skip("prod module not built")

    assert lm.get_multisig_module(mode="prod") is lm.prod
    assert lm.get_multisig_module(test_mode=False) is lm.prod


def test_module_selection_invalid_inputs():
    """Invalid selection parameters should raise ValueError."""
    import lean_multisig_py as lm

    with pytest.raises(ValueError):
        lm.get_multisig_module(mode="invalid")

    with pytest.raises(ValueError):
        lm.get_multisig_module(mode="prod", test_mode=True)


def test_module_selection_missing_mode(monkeypatch):
    """Requesting a missing module should raise RuntimeError."""
    import lean_multisig_py as lm

    if lm.prod is None and lm.test is None:
        pytest.skip("no modules built")

    if lm.prod is None:
        missing = "prod"
    elif lm.test is None:
        missing = "test"
    else:
        missing = "prod"
        monkeypatch.setattr(lm, "prod", None)

    with pytest.raises(RuntimeError, match=missing):
        lm.get_multisig_module(mode=missing)


def test_prod_mode_import():
    """Test that prod mode can be imported and has correct MODE."""
    from lean_multisig_py import prod

    if prod is None:
        pytest.skip("prod module not built")

    assert prod.MODE == "prod"
    assert prod.get_mode() == "prod"


def test_test_mode_import():
    """Test that test mode can be imported and has correct MODE."""
    from lean_multisig_py import test

    if test is None:
        pytest.skip("test module not built")

    assert test.MODE == "test"
    assert test.get_mode() == "test"


def test_prod_setup():
    """Test prod mode setup functions."""
    from lean_multisig_py import prod

    if prod is None:
        pytest.skip("prod module not built")

    # These should not raise
    prod.setup_prover()
    prod.setup_verifier()


def test_test_setup():
    """Test test mode setup functions."""
    from lean_multisig_py import test

    if test is None:
        pytest.skip("test module not built")

    # These should not raise
    test.setup_prover()
    test.setup_verifier()


def test_aggregate_signatures_validation():
    """Test input validation for aggregate_signatures."""
    from lean_multisig_py import prod, test

    # Use whichever module is available
    multisig = prod if prod is not None else test
    if multisig is None:
        pytest.skip("No module built")

    # Test message_hash length validation
    with pytest.raises(ValueError, match="message_hash must be exactly 32 bytes"):
        multisig.aggregate_signatures(
            [b"pubkey1"],
            [b"sig1"],
            b"short",  # Not 32 bytes
            1
        )

    # Test mismatched pub_keys and signatures
    with pytest.raises(ValueError, match="must match"):
        multisig.aggregate_signatures(
            [b"pubkey1", b"pubkey2"],
            [b"sig1"],  # Only one signature
            b"a" * 32,
            1
        )

    # Test flag-based selection
    import lean_multisig_py as lm

    if lm.prod is not None:
        with pytest.raises(ValueError, match="message_hash must be exactly 32 bytes"):
            lm.aggregate_signatures(
                [b"pk"],
                [b"sig"],
                b"short",
                1,
                mode="prod",
            )


def test_verify_signatures_validation():
    """Test input validation for verify_aggregated_signatures."""
    from lean_multisig_py import prod, test

    # Use whichever module is available
    multisig = prod if prod is not None else test
    if multisig is None:
        pytest.skip("No module built")

    # Test message_hash length validation
    with pytest.raises(ValueError, match="message_hash must be exactly 32 bytes"):
        multisig.verify_aggregated_signatures(
            [b"pubkey1"],
            b"short",  # Not 32 bytes
            b"agg_sig",
            1
        )

    import lean_multisig_py as lm

    if lm.prod is not None:
        with pytest.raises(ValueError, match="message_hash must be exactly 32 bytes"):
            lm.verify_aggregated_signatures(
                [b"pk"],
                b"short",
                b"agg_sig",
                1,
                mode="prod",
            )


def test_ssz_functions_available():
    """Test that SSZ encode/decode functions are available."""
    from lean_multisig_py import prod, test

    # Use whichever module is available
    multisig = prod if prod is not None else test
    if multisig is None:
        pytest.skip("No module built")

    # Functions should exist
    assert hasattr(multisig, 'ssz_encode_aggregate_signature')
    assert hasattr(multisig, 'ssz_decode_aggregate_signature')


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
