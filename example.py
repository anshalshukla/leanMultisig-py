"""Example usage of lean-multisig Python bindings (devnet5).

This file documents the public API surface. Real signatures come from
the leanSig / leanSpec Python code; the snippets below show what to do
with them once you have SSZ-encoded `XmssPublicKey` and `XmssSignature`
bytes.
"""

import lean_multisig_py as lm


def example_type_1():
    """Type 1: aggregate raw signatures over a single (message, slot)."""
    lm.setup_prover()
    lm.setup_verifier()

    pub_keys_bytes = [...]      # list[bytes], SSZ-encoded XmssPublicKey
    signatures_bytes = [...]    # list[bytes], SSZ-encoded XmssSignature, paired 1:1 with pub_keys
    message_hash = b"\x00" * 32
    slot = 42
    log_inv_rate = 1

    sorted_pks_ssz, type1_bytes = lm.aggregate_type_1(
        pub_keys_bytes, signatures_bytes, message_hash, slot, log_inv_rate
    )

    lm.verify_type_1(sorted_pks_ssz, message_hash, slot, type1_bytes)


def example_type_1_hierarchical():
    """Type 1 with prior Type-1 children (still single message + slot)."""
    sorted_pks_a, type1_a = lm.aggregate_type_1(
        pks_a, sigs_a, message_hash, slot, log_inv_rate
    )

    sorted_pks_b, type1_b = lm.aggregate_type_1(
        pks_b, sigs_b, message_hash, slot, log_inv_rate,
        children_bytes=[(sorted_pks_a, type1_a)],
    )

    lm.verify_type_1(sorted_pks_b, message_hash, slot, type1_b)


def example_type_2():
    """Type 2: merge Type-1 multi-signatures with distinct messages."""
    sig_a = lm.aggregate_type_1(pks_a, sigs_a, msg_a, slot, log_inv_rate)
    sig_b = lm.aggregate_type_1(pks_b, sigs_b, msg_b, slot, log_inv_rate)

    pks_per_component, type2_bytes = lm.merge_many_type_1(
        [sig_a, sig_b], log_inv_rate
    )

    lm.verify_type_2(pks_per_component, type2_bytes)

    # Extract the first component back as an independent Type-1:
    pks_only_a, type1_a = lm.split_type_2(
        pks_per_component, type2_bytes, 0, log_inv_rate
    )
    lm.verify_type_1(pks_only_a, msg_a, slot, type1_a)
