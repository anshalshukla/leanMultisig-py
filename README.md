# lean-multisig Python Bindings (devnet5)

Python bindings for XMSS multi-signature aggregation from the
[leanMultisig](https://github.com/leanEthereum/leanMultisig) project (`devnet5`).

devnet5 splits aggregation into two layers:

- **Type 1** — one message, one slot, many signers (raw signatures and/or prior Type-1 children).
- **Type 2** — many components (potentially with distinct messages/slots), merged by a single SNARK.

## Installation

Wheels for macOS arm64 and Linux x86_64 (Python 3.12 / 3.13 / 3.14) are
attached to each [GitHub Release](https://github.com/anshalshukla/leanMultisig-py/releases):

```bash
# pick the wheel matching your interpreter + platform
pip install https://github.com/anshalshukla/leanMultisig-py/releases/download/v0.2.0/lean_multisig_py-0.2.0-cp312-cp312-macosx_11_0_arm64.whl
```

Each wheel ships both the `lean_multisig` (prod) and `lean_multisig_test` (test-config)
extension modules in one package — pick at call time with `mode="prod"` / `mode="test"`.

### From source (for development)

```bash
pip install maturin
maturin develop --release                  # installs the prod module into your venv
maturin develop --release --features test-config  # test-config build
```

Or build all wheels locally (mirrors what CI does):

```bash
./build_all.sh --macos-only   # produces wheels in target/wheels/
```

## Usage

### Type 1 (single message + slot)

```python
import lean_multisig_py as lm

lm.setup_prover(mode="prod")        # call once at startup
# lm.setup_verifier(mode="prod")    # call once before the first verify

pub_keys_bytes   = [...]            # list of SSZ-encoded XmssPublicKey
signatures_bytes = [...]            # list of SSZ-encoded XmssSignature
message_hash     = b"\x00" * 32
slot             = 42
log_inv_rate     = 1

sorted_pks_ssz, type1_bytes = lm.aggregate_type_1(
    pub_keys_bytes, signatures_bytes, message_hash, slot, log_inv_rate
)

lm.verify_type_1(sorted_pks_ssz, message_hash, slot, type1_bytes)   # raises on failure
```

### Type 2 (merge many Type-1s)

```python
# Build several Type-1 multi-sigs (one per (message, slot) group), then merge:
sig_a = lm.aggregate_type_1(pks_a, sigs_a, msg_a, slot, log_inv_rate)
sig_b = lm.aggregate_type_1(pks_b, sigs_b, msg_b, slot, log_inv_rate)

pks_per_component, type2_bytes = lm.merge_many_type_1(
    [sig_a, sig_b], log_inv_rate
)

lm.verify_type_2(pks_per_component, type2_bytes)

# Pull one component back out as an independent Type-1:
pks_only_a, type1_a = lm.split_type_2(pks_per_component, type2_bytes, 0, log_inv_rate)
# …or select by message:
pks_only_a, type1_a = lm.split_type_2_by_msg(
    pks_per_component, type2_bytes, msg_a, log_inv_rate
)
```

### Choosing the serialization form

`aggregate_type_1`, `merge_many_type_1`, `split_type_2*` all return the **no-pubkeys**
form `(pks_ssz, sig_bytes)` — small blob, pubkeys carried separately. To bundle the
pubkeys into a single self-contained blob (the upstream `compress()` form), use the
converters:

```python
# Type 1
combined = lm.type1_compress_with_pubkeys(pks_ssz, sig_bytes)
pks_ssz, sig_bytes = lm.type1_decompress_with_pubkeys(combined)

# Type 2
combined = lm.type2_compress_with_pubkeys(pks_per_component, sig_bytes)
pks_per_component, sig_bytes = lm.type2_decompress_with_pubkeys(combined)
```

### SSZ container codecs

`ssz_encode_type1_signature` / `ssz_decode_type1_signature` wrap a Type-1 blob in
the `Devnet5Type1Signature` SSZ container. Analogous `*_type2_*` helpers exist
for Type-2.

## API reference

| Function | Description |
|---|---|
| `setup_prover(mode=)` | Compile aggregation bytecode and precompute DFT twiddles. |
| `setup_verifier(mode=)` | Compile aggregation bytecode. |
| `aggregate_type_1(pks, sigs, msg, slot, log_inv_rate, children=None, mode=)` | Returns `(sorted_pks_ssz, type1_bytes)`. |
| `verify_type_1(pks, msg, slot, sig_bytes, mode=)` | Raises `ValueError` on failure. |
| `merge_many_type_1(entries, log_inv_rate, mode=)` | `entries = [(pks_ssz, type1_bytes), …]`. Returns `(pks_per_component, type2_bytes)`. |
| `verify_type_2(pks_per_component, sig_bytes, mode=)` | Raises on failure. |
| `split_type_2(pks_per_component, sig_bytes, index, log_inv_rate, mode=)` | Returns `(pks_ssz, type1_bytes)`. |
| `split_type_2_by_msg(pks_per_component, sig_bytes, message, log_inv_rate, mode=)` | Same, selected by message. |
| `type1_compress_with_pubkeys(pks_ssz, sig_bytes, mode=)` | Bundle pubkeys into a single Type-1 blob. |
| `type1_decompress_with_pubkeys(sig_bytes, mode=)` | Split a self-contained Type-1 blob into `(pks_ssz, sig_bytes)`. |
| `type2_compress_with_pubkeys(pks_per_component, sig_bytes, mode=)` | Bundle per-component pubkeys into a single Type-2 blob. |
| `type2_decompress_with_pubkeys(sig_bytes, mode=)` | Split a self-contained Type-2 blob into `(pks_per_component, sig_bytes)`. |
| `ssz_encode_type1_signature` / `ssz_decode_type1_signature` | Opaque SSZ wrapper for Type-1 blobs. |
| `ssz_encode_type2_signature` / `ssz_decode_type2_signature` | Opaque SSZ wrapper for Type-2 blobs. |

## Notes

- `setup_prover` is expensive (~seconds). Call it once at process start.
- All `pub_keys_bytes` / `signatures_bytes` must be SSZ-encoded (see `leansig_wrapper`'s `xmss_public_key_to_ssz` / `xmss_signature_to_ssz`).
- The number of public keys must match the number of signatures in `aggregate_type_1`.
- Mode selection: with no `mode=` argument the wrappers prefer the prod module if both are present.
