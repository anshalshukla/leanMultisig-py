"""
Example usage of lean-multisig Python bindings.

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

    # TODO: Replace this with your actual signature generation from leanSpec
    # You need to:
    # 1. Generate XMSS signatures using your Python spec from ../leanSpec
    # 2. Serialize each public key to bytes using bincode or compatible format
    # 3. Serialize each signature to bytes using bincode or compatible format

    # Example placeholder data (you'll replace this with your actual data):
    # pub_keys_bytes = [pk1_bytes, pk2_bytes, ...]  # List of serialized public keys
    # signatures_bytes = [sig1_bytes, sig2_bytes, ...]  # List of serialized signatures
    # message_hash = b'...'  # 32-byte message hash
    # epoch = 50  # Your epoch number

    print("""
To use this with your actual signatures:

1. Generate XMSS signatures from your Python spec in ../leanSpec
2. Serialize the public keys and signatures to bytes using bincode format:
   - In Python, you can use a bincode library or match the Rust bincode format
   - Each public key should be serialized as LeanSigPubKey
   - Each signature should be serialized as LeanSigSignature

3. Call aggregate_signatures:
   agg_sig_bytes = lean_multisig_py.aggregate_signatures(
       pub_keys_bytes,  # List[bytes]
       signatures_bytes,  # List[bytes]
       message_hash,  # bytes (32 bytes)
       epoch  # int
   )

4. Call verify_aggregated_signatures:
   lean_multisig_py.verify_aggregated_signatures(
       pub_keys_bytes,  # List[bytes]
       message_hash,  # bytes (32 bytes)
       agg_sig_bytes,  # bytes
       epoch  # int
   )
   # Raises ValueError if verification fails
""")

if __name__ == "__main__":
    example_usage()
