from lean_multisig_py import (  # noqa: E402
    aggregate_signatures,
    setup_prover,
    setup_verifier,
    verify_aggregated_signatures,
)

def test_debug():
    setup_prover()
    setup_verifier()

    pub_keys_bytes = [b'1234567890']
    signatures_bytes = [b'1234567890']
    # message_hash is now a list of 8 field elements (u64 values)
    message_hash = [0, 1, 2, 3, 4, 5, 6, 7]
    slot = 1  # renamed from epoch
    test_mode = True
    agg_sig_bytes = aggregate_signatures(pub_keys_bytes, signatures_bytes, message_hash, slot, test_mode)
    print(agg_sig_bytes)
    # `verify_aggregated_signatures` returns None on success and raises on failure.
    assert (
        verify_aggregated_signatures(
            pub_keys_bytes, message_hash, agg_sig_bytes, slot, test_mode
        )
        is None
    )

if __name__ == "__main__":
    test_debug()