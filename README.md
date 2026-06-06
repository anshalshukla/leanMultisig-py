# lean-multisig Python Bindings (devnet5)

Python bindings for XMSS multi-signature aggregation from the
[leanMultisig](https://github.com/leanEthereum/leanMultisig) project (`devnet5`).

devnet5 splits aggregation into two layers:

- **Single-message proof** — one message, one slot, many signers (raw signatures and/or prior single-message proof children).
- **Multi-message proof** — many components (potentially with distinct messages/slots), merged by a single SNARK.

## Installation

Wheels for macOS arm64 and Linux x86_64 (Python 3.12 / 3.13 / 3.14) are
attached to each [GitHub Release](https://github.com/anshalshukla/leanMultisig-py/releases):

```bash
# pick the wheel matching your interpreter + platform
pip install https://github.com/anshalshukla/leanMultisig-py/releases/download/v0.0.5/lean_multisig_py-0.0.5-cp312-cp312-macosx_11_0_arm64.whl
```

Each wheel ships both the `lean_multisig` (prod) and `lean_multisig_test` (test-config)
extension modules in one package — pick at call time with `mode="prod"` / `mode="test"`.

### From source (for development)

```bash
pip install maturin
maturin develop --release                  # installs the prod module into your venv
maturin develop --release --features test-config  # test-config build
```

Or build a release wheel locally with maturin directly:

```bash
maturin build --release --out target/wheels             # prod module
maturin build --release --features test-config --out target/wheels-test
```

Release wheels (both modules merged into one, for all supported
platforms/interpreters) are produced by the `release.yml` GitHub Actions
workflow — there is no longer a local `build_all.sh`.

## Usage

### Single-message proof (single message + slot)

```python
import lean_multisig_py as lm

lm.setup_prover(mode="prod")        # call once at startup
# lm.setup_verifier(mode="prod")    # call once before the first verify

pub_keys_bytes   = [...]            # list of SSZ-encoded XmssPublicKey
signatures_bytes = [...]            # list of SSZ-encoded XmssSignature
message_hash     = b"\x00" * 32
slot             = 42
log_inv_rate     = 1

sorted_pks_ssz, single_message_proof_bytes = lm.aggregate_single_message(
    pub_keys_bytes, signatures_bytes, message_hash, slot, log_inv_rate
)

lm.verify_single_message_proof(sorted_pks_ssz, message_hash, slot, single_message_proof_bytes)   # raises on failure
```

### Multi-message proof (merge many single-message proofs)

```python
# Build several single-message proofs (one per (message, slot) group), then merge:
sig_a = lm.aggregate_single_message(pks_a, sigs_a, msg_a, slot, log_inv_rate)
sig_b = lm.aggregate_single_message(pks_b, sigs_b, msg_b, slot, log_inv_rate)

pks_per_component, multi_message_proof_bytes = lm.merge_many_single_message_proof(
    [sig_a, sig_b], log_inv_rate
)

lm.verify_multi_message_proof(pks_per_component, multi_message_proof_bytes)

# Pull one component back out as an independent single-message proof:
pks_only_a, single_message_proof_a = lm.split_multi_message_proof(pks_per_component, multi_message_proof_bytes, 0, log_inv_rate)
# …or select by message:
pks_only_a, single_message_proof_a = lm.split_multi_message_proof_by_message(
    pks_per_component, multi_message_proof_bytes, msg_a, log_inv_rate
)
```

### Choosing the serialization form

`aggregate_single_message`, `merge_many_single_message_proof`, `split_multi_message_proof*` all return the **no-pubkeys**
form `(pks_ssz, sig_bytes)` — small blob, pubkeys carried separately. To bundle the
pubkeys into a single self-contained blob (the upstream `compress()` form), use the
converters:

```python
# Single-message proof
combined = lm.single_message_proof_compress_with_pubkeys(pks_ssz, sig_bytes)         # bundle
pks_ssz, sig_bytes = lm.single_message_proof_decompress_with_pubkeys(combined)       # split
wire_only = lm.single_message_proof_compress_without_pubkeys(combined)               # strip → no-pubkeys form

# Multi-message proof
combined = lm.multi_message_proof_compress_with_pubkeys(pks_per_component, sig_bytes)
pks_per_component, sig_bytes = lm.multi_message_proof_decompress_with_pubkeys(combined)
wire_only = lm.multi_message_proof_compress_without_pubkeys(combined)
```

Typical flow if you **store bundled locally but propagate stripped on the wire**:

```python
# on disk: keep `bundled` (with pubkeys, single blob)
wire_bytes = lm.single_message_proof_compress_without_pubkeys(bundled)               # outbound
# … receive `(pks_ssz, wire_bytes)` from a peer …
bundled    = lm.single_message_proof_compress_with_pubkeys(pks_ssz, wire_bytes)      # rehydrate to bundled form
```

### SSZ container codecs

`ssz_encode_single_message_proof` / `ssz_decode_single_message_proof` wrap a single-message proof blob in
the `Devnet5SingleMessageProof` SSZ container. Analogous `*_multi_message_proof` helpers exist
for multi-message proofs.

## API reference

| Function | Description |
|---|---|
| `setup_prover(mode=)` | Compile aggregation bytecode and precompute DFT twiddles. |
| `setup_verifier(mode=)` | Compile aggregation bytecode. |
| `aggregate_single_message(pks, sigs, msg, slot, log_inv_rate, children=None, mode=)` | Returns `(sorted_pks_ssz, single_message_proof_bytes)`. |
| `verify_single_message_proof(pks, msg, slot, sig_bytes, mode=)` | Raises `ValueError` on failure. |
| `merge_many_single_message_proof(entries, log_inv_rate, mode=)` | `entries = [(pks_ssz, single_message_proof_bytes), …]`. Returns `(pks_per_component, multi_message_proof_bytes)`. |
| `verify_multi_message_proof(pks_per_component, sig_bytes, mode=)` | Raises on failure. |
| `verify_multi_message_proof_with_messages(pks_per_component, expected_messages, sig_bytes, mode=)` | Like `verify_multi_message_proof`, but also binds each component to an expected `(message_hash, slot)`. |
| `split_multi_message_proof(pks_per_component, sig_bytes, index, log_inv_rate, mode=)` | Returns `(pks_ssz, single_message_proof_bytes)`. |
| `split_multi_message_proof_by_message(pks_per_component, sig_bytes, message, log_inv_rate, mode=)` | Same, selected by message. |
| `single_message_proof_compress_with_pubkeys(pks_ssz, sig_bytes, mode=)` | Bundle pubkeys into a single single-message proof blob. |
| `single_message_proof_decompress_with_pubkeys(sig_bytes, mode=)` | Split a self-contained single-message proof blob into `(pks_ssz, sig_bytes)`. |
| `single_message_proof_compress_without_pubkeys(sig_bytes, mode=)` | Strip pubkeys from a self-contained single-message proof blob; returns the no-pubkeys wire form. |
| `multi_message_proof_compress_with_pubkeys(pks_per_component, sig_bytes, mode=)` | Bundle per-component pubkeys into a single multi-message proof blob. |
| `multi_message_proof_decompress_with_pubkeys(sig_bytes, mode=)` | Split a self-contained multi-message proof blob into `(pks_per_component, sig_bytes)`. |
| `multi_message_proof_compress_without_pubkeys(sig_bytes, mode=)` | Strip pubkeys from a self-contained multi-message proof blob; returns the no-pubkeys wire form. |
| `ssz_encode_single_message_proof` / `ssz_decode_single_message_proof` | Opaque SSZ wrapper for single-message proof blobs. |
| `ssz_encode_multi_message_proof` / `ssz_decode_multi_message_proof` | Opaque SSZ wrapper for multi-message proof blobs. |

## Notes

- `setup_prover` is expensive (~seconds). Call it once at process start.
- All `pub_keys_bytes` / `signatures_bytes` must be SSZ-encoded (see `leansig_wrapper`'s `xmss_public_key_to_ssz` / `xmss_signature_to_ssz`).
- The number of public keys must match the number of signatures in `aggregate_single_message`.
- Mode selection: with no `mode=` argument the wrappers prefer the prod module if both are present.
