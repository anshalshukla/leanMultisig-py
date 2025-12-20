# lean-multisig Python Bindings

Python bindings for XMSS signature aggregation from the lean-multisig project.

## Features

- `aggregate_signatures`: Aggregate multiple XMSS signatures into a single proof
- `verify_aggregated_signatures`: Verify an aggregated signature
- `setup_prover`: Pre-compute DFT twiddles for faster proving (optional but recommended)
- `setup_verifier`: Pre-compute data for faster verification (optional but recommended)

## Installation

### Prerequisites

- Python 3.8 or later
- Rust toolchain
- maturin (`pip install maturin`)

### Build and Install

```bash
# From the leanMultisig-py directory
maturin develop --release
```

For production builds:
```bash
maturin build --release
pip install target/wheels/lean_multisig_py-*.whl
```

## Usage

### Basic Example

```python
import lean_multisig_py

# Optional: Setup for better performance (call once at startup)
lean_multisig_py.setup_prover()
lean_multisig_py.setup_verifier()

# Your serialized data (from leanSpec or other source)
pub_keys_bytes = [...]  # List of serialized public keys (bytes)
signatures_bytes = [...]  # List of serialized signatures (bytes)
message_hash = b'...'  # 32-byte message hash
epoch = 50

# Aggregate signatures
agg_sig_bytes = lean_multisig_py.aggregate_signatures(
    pub_keys_bytes,
    signatures_bytes,
    message_hash,
    epoch
)

# Verify aggregated signature
try:
    lean_multisig_py.verify_aggregated_signatures(
        pub_keys_bytes,
        message_hash,
        agg_sig_bytes,
        epoch
    )
    print("Verification successful!")
except ValueError as e:
    print(f"Verification failed: {e}")
```

## Data Format

The bindings expect data to be serialized using the bincode format, which is the same format used by the Rust code.

### Required Types

1. **Public Keys**: Each public key must be serialized as `LeanSigPubKey` type
2. **Signatures**: Each signature must be serialized as `LeanSigSignature` type
3. **Message Hash**: Must be exactly 32 bytes
4. **Epoch**: A 32-bit unsigned integer

### Serialization

Since you mentioned you already have ways to generate XMSS signatures from `../leanSpec`, you'll need to ensure they're serialized in bincode format before passing to these functions.

If you need help with serialization, you can:
1. Use a Python bincode library
2. Create a simple Rust helper that serializes your Python objects
3. Match the exact binary format that Rust's bincode produces

## API Reference

### `setup_prover()`
Pre-computes DFT twiddles for faster proving. Call this once at startup before the first aggregation.

### `setup_verifier()`
Pre-computes data for faster verification. Call this once at startup before the first verification.

### `aggregate_signatures(pub_keys_bytes, signatures_bytes, message_hash, epoch)`
Aggregate multiple XMSS signatures into a single proof.

**Parameters:**
- `pub_keys_bytes` (List[bytes]): List of serialized public keys
- `signatures_bytes` (List[bytes]): List of serialized signatures (must match length of pub_keys_bytes)
- `message_hash` (bytes): 32-byte message hash
- `epoch` (int): Epoch number

**Returns:** bytes - Serialized aggregated signature

**Raises:** ValueError if inputs are invalid or aggregation fails

### `verify_aggregated_signatures(pub_keys_bytes, message_hash, agg_signature_bytes, epoch)`
Verify an aggregated signature.

**Parameters:**
- `pub_keys_bytes` (List[bytes]): List of serialized public keys
- `message_hash` (bytes): 32-byte message hash
- `agg_signature_bytes` (bytes): Serialized aggregated signature
- `epoch` (int): Epoch number

**Returns:** None

**Raises:** ValueError if verification fails

## Development

### Building in Debug Mode
```bash
maturin develop
```

### Running Tests
```bash
# Build and install
maturin develop --release

# Run your Python tests
python -m pytest tests/
```

## Notes

- The aggregation process uses SNARK proofs and can be computationally intensive
- Using `setup_prover()` and `setup_verifier()` is optional but recommended to avoid slowdowns on the first call
- Make sure the number of public keys matches the number of signatures
- All serialization must use the bincode format compatible with the Rust types
