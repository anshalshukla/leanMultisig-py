"""
Example usage of lean-multisig Python bindings (devnet4).

This example shows how to:
1. Load XMSS public keys and signatures from your Python spec
2. Aggregate them using the Rust backend
3. Verify the aggregated signature
"""

import lean_multisig_py


def example_usage():
    """
    Example demonstrating XMSS signature aggregation.

    You should replace this with your actual code that generates
    signatures from ../leanSpec.
    """

    # Optional: Setup prover and verifier for better performance
    # Call these once at startup to precompute DFT twiddles
    print("Setting up prover...")
    lean_multisig_py.setup_prover()

    print("Setting up verifier...")
    lean_multisig_py.setup_verifier()

    print("""
To use this with your actual signatures:

1. Generate XMSS signatures from your Python spec in ../leanSpec
2. Serialize the public keys and signatures to bytes using postcard format:
   - Each public key should be serialized as XmssPublicKey
   - Each signature should be serialized as XmssSignature

3. Call aggregate_signatures:
   agg_sig_bytes = lean_multisig_py.aggregate_signatures(
       pub_keys_bytes,   # List[bytes]
       signatures_bytes, # List[bytes]
       message_hash,     # bytes (32 bytes)
       slot,             # int
       log_inv_rate,     # int (1-4, lower = faster but bigger proofs)
   )

4. Call verify_aggregated_signatures:
   lean_multisig_py.verify_aggregated_signatures(
       message_hash,     # bytes (32 bytes)
       agg_sig_bytes,    # bytes
       slot,             # int
   )
   # Raises ValueError if verification fails

5. For hierarchical aggregation, pass previously aggregated results:
   final_agg = lean_multisig_py.aggregate_signatures(
       more_pub_keys_bytes,
       more_signatures_bytes,
       message_hash,
       slot,
       log_inv_rate,
       children_bytes=[agg_sig_bytes],  # previously aggregated
   )
""")

if __name__ == "__main__":
    example_usage()
