use backend::{precompute_dft_twiddles, KoalaBear};
use leansig_wrapper::{
    xmss_public_key_from_ssz, xmss_public_key_to_ssz, xmss_signature_from_ssz, XmssPublicKey,
    XmssSignature, MESSAGE_LENGTH,
};
use rec_aggregation::{
    aggregate_single_message_signatures, init_aggregation_bytecode, merge_single_message_aggregates,
    split_multi_message_aggregate, split_multi_message_aggregate_by_message,
    verify_multi_message_aggregate, verify_single_message_aggregate, MultiMessageAggregateSignature,
    SingleMessageAggregateSignature,
};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use ssz::{Decode, DecodeError, Encode};

#[cfg(feature = "test-config")]
pub const MODE: &str = "test";

#[cfg(not(feature = "test-config"))]
pub const MODE: &str = "prod";

/// Opaque SSZ container wrapping the compressed bytes of a single-message proof
/// (postcard + lz4, produced by `SingleMessageAggregateSignature::compress_without_pubkeys`).
#[derive(Debug, Clone)]
pub struct Devnet5SingleMessageProof {
    pub proof_bytes: Vec<u8>,
}

impl Encode for Devnet5SingleMessageProof {
    fn is_ssz_fixed_len() -> bool {
        false
    }
    fn ssz_bytes_len(&self) -> usize {
        self.proof_bytes.len()
    }
    fn ssz_append(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.proof_bytes);
    }
}

impl Decode for Devnet5SingleMessageProof {
    fn is_ssz_fixed_len() -> bool {
        false
    }
    fn from_ssz_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.is_empty() {
            return Err(DecodeError::InvalidByteLength { len: 0, expected: 1 });
        }
        Ok(Self { proof_bytes: bytes.to_vec() })
    }
}

/// Opaque SSZ container wrapping the compressed bytes of a multi-message proof
/// (postcard + lz4, produced by `MultiMessageAggregateSignature::compress_without_pubkeys`).
#[derive(Debug, Clone)]
pub struct Devnet5MultiMessageProof {
    pub proof_bytes: Vec<u8>,
}

impl Encode for Devnet5MultiMessageProof {
    fn is_ssz_fixed_len() -> bool {
        false
    }
    fn ssz_bytes_len(&self) -> usize {
        self.proof_bytes.len()
    }
    fn ssz_append(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.proof_bytes);
    }
}

impl Decode for Devnet5MultiMessageProof {
    fn is_ssz_fixed_len() -> bool {
        false
    }
    fn from_ssz_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.is_empty() {
            return Err(DecodeError::InvalidByteLength { len: 0, expected: 1 });
        }
        Ok(Self { proof_bytes: bytes.to_vec() })
    }
}

fn deserialize_pub_keys(ssz_list: &[Vec<u8>], label: &str) -> PyResult<Vec<XmssPublicKey>> {
    ssz_list
        .iter()
        .enumerate()
        .map(|(i, bytes)| {
            xmss_public_key_from_ssz(bytes).map_err(|()| {
                PyValueError::new_err(format!("Failed to deserialize {label} public key {i} (SSZ)"))
            })
        })
        .collect()
}

fn deserialize_signatures(ssz_list: &[Vec<u8>]) -> PyResult<Vec<XmssSignature>> {
    ssz_list
        .iter()
        .enumerate()
        .map(|(i, bytes)| {
            xmss_signature_from_ssz(bytes)
                .map_err(|()| PyValueError::new_err(format!("Failed to deserialize signature {i} (SSZ)")))
        })
        .collect()
}

fn message_to_array(message_hash: Vec<u8>) -> PyResult<[u8; MESSAGE_LENGTH]> {
    if message_hash.len() != MESSAGE_LENGTH {
        return Err(PyValueError::new_err(format!(
            "message_hash must be exactly {} bytes, got {}",
            MESSAGE_LENGTH,
            message_hash.len()
        )));
    }
    message_hash
        .try_into()
        .map_err(|_| PyValueError::new_err("Failed to convert message_hash to fixed-size array"))
}

fn pks_to_ssz(pks: &[XmssPublicKey]) -> Vec<Vec<u8>> {
    pks.iter().map(xmss_public_key_to_ssz).collect()
}

fn catch_panic<R>(f: impl FnOnce() -> R, what: &str) -> PyResult<R> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|e| {
        let msg = if let Some(s) = e.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = e.downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown error".to_string()
        };
        PyValueError::new_err(format!("{what} failed: {msg}"))
    })
}

#[pyfunction]
fn setup_prover() {
    init_aggregation_bytecode();
    precompute_dft_twiddles::<KoalaBear>(1 << 24);
}

#[pyfunction]
fn setup_verifier() {
    init_aggregation_bytecode();
}

/// Aggregate XMSS signatures into a single-message proof (single message, single slot).
///
/// Args:
///     pub_keys_bytes: SSZ-encoded public keys, one per raw signature.
///     signatures_bytes: SSZ-encoded raw XMSS signatures, paired with `pub_keys_bytes`.
///     message_hash: 32-byte message hash.
///     slot: Slot number.
///     log_inv_rate: Inverse rate exponent for the proof.
///     children_bytes: Optional list of `(child_pub_keys_ssz, child_single_message_proof_bytes)` tuples,
///         where `child_single_message_proof_bytes` is a single-message proof produced by a prior call
///         (compressed without pubkeys).
///
/// Returns:
///     `(sorted_pub_keys_ssz, single_message_proof_bytes)` — pubkeys are returned sorted+deduplicated,
///     `single_message_proof_bytes` is the single-message proof compressed without pubkeys.
#[pyfunction(name = "aggregate_single_message")]
#[pyo3(signature = (pub_keys_bytes, signatures_bytes, message_hash, slot, log_inv_rate, children_bytes=None))]
fn py_aggregate_single_message(
    pub_keys_bytes: Vec<Vec<u8>>,
    signatures_bytes: Vec<Vec<u8>>,
    message_hash: Vec<u8>,
    slot: u32,
    log_inv_rate: usize,
    children_bytes: Option<Vec<(Vec<Vec<u8>>, Vec<u8>)>>,
) -> PyResult<(Vec<Vec<u8>>, Vec<u8>)> {
    if pub_keys_bytes.len() != signatures_bytes.len() {
        return Err(PyValueError::new_err(format!(
            "Number of public keys ({}) must match number of signatures ({})",
            pub_keys_bytes.len(),
            signatures_bytes.len()
        )));
    }
    let message_array = message_to_array(message_hash)?;

    let pub_keys = deserialize_pub_keys(&pub_keys_bytes, "raw")?;
    let signatures = deserialize_signatures(&signatures_bytes)?;
    let raw_xmss: Vec<(XmssPublicKey, XmssSignature)> =
        pub_keys.into_iter().zip(signatures).collect();

    let children: Vec<SingleMessageAggregateSignature> = match children_bytes {
        Some(cb) => cb
            .into_iter()
            .enumerate()
            .map(|(i, (child_pks_bytes, child_bytes))| {
                let child_pks = deserialize_pub_keys(&child_pks_bytes, &format!("child {i}"))?;
                SingleMessageAggregateSignature::decompress_without_pubkeys(&child_bytes, child_pks).ok_or_else(
                    || PyValueError::new_err(format!("Failed to decompress child {i} (single-message aggregate)")),
                )
            })
            .collect::<PyResult<_>>()?,
        None => Vec::new(),
    };

    let agg = catch_panic(
        || aggregate_single_message_signatures(&children, raw_xmss, message_array, slot, log_inv_rate),
        "single-message aggregation",
    )?
    .map_err(|e| PyValueError::new_err(format!("single-message aggregation failed: {e:?}")))?;

    let sorted_pks_ssz = pks_to_ssz(&agg.info.pubkeys);
    Ok((sorted_pks_ssz, agg.compress_without_pubkeys()))
}

/// Verify a single-message proof.
///
/// Args:
///     pub_keys_bytes: SSZ-encoded sorted+deduplicated public keys (as returned by aggregate).
///     message_hash: 32-byte message hash.
///     slot: Slot number.
///     sig_bytes: single-message proof bytes (compressed without pubkeys).
///
/// Raises:
///     ValueError on any failure.
#[pyfunction(name = "verify_single_message_proof")]
fn py_verify_single_message_proof(
    pub_keys_bytes: Vec<Vec<u8>>,
    message_hash: Vec<u8>,
    slot: u32,
    sig_bytes: Vec<u8>,
) -> PyResult<()> {
    let message_array = message_to_array(message_hash)?;
    let pub_keys = deserialize_pub_keys(&pub_keys_bytes, "single-message")?;
    let sig = SingleMessageAggregateSignature::decompress_without_pubkeys(&sig_bytes, pub_keys)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress single-message proof"))?;

    if sig.info.without_pubkeys.message != message_array {
        return Err(PyValueError::new_err("message_hash does not match signature"));
    }
    if sig.info.without_pubkeys.slot != slot {
        return Err(PyValueError::new_err("slot does not match signature"));
    }

    verify_single_message_aggregate(&sig)
        .map_err(|e| PyValueError::new_err(format!("Verification failed: {e:?}")))?;
    Ok(())
}

/// Merge multiple single-message proofs (potentially over different messages/slots)
/// into a single multi-message proof.
///
/// Args:
///     single_message_proof_entries: List of `(pub_keys_ssz, single_message_proof_bytes)` tuples.
///     log_inv_rate: Inverse rate exponent for the proof.
///
/// Returns:
///     `(pks_per_component_ssz, multi_message_proof_bytes)` — `pks_per_component_ssz[i]` is the
///     SSZ-encoded pubkey list for component `i`; `multi_message_proof_bytes` is the multi-message
///     proof compressed without pubkeys.
#[pyfunction(name = "merge_many_single_message_proof")]
fn py_merge_many_single_message_proof(
    single_message_proof_entries: Vec<(Vec<Vec<u8>>, Vec<u8>)>,
    log_inv_rate: usize,
) -> PyResult<(Vec<Vec<Vec<u8>>>, Vec<u8>)> {
    if single_message_proof_entries.is_empty() {
        return Err(PyValueError::new_err("merge_many_single_message_proof requires at least one entry"));
    }

    let single_message_proofs: Vec<SingleMessageAggregateSignature> = single_message_proof_entries
        .into_iter()
        .enumerate()
        .map(|(i, (pks_bytes, sig_bytes))| {
            let pks = deserialize_pub_keys(&pks_bytes, &format!("component {i}"))?;
            SingleMessageAggregateSignature::decompress_without_pubkeys(&sig_bytes, pks).ok_or_else(|| {
                PyValueError::new_err(format!("Failed to decompress single-message component {i}"))
            })
        })
        .collect::<PyResult<_>>()?;

    let multi_message_proof = catch_panic(
        || merge_single_message_aggregates(single_message_proofs, log_inv_rate),
        "merge_many_single_message_proof",
    )?
    .map_err(|e| PyValueError::new_err(format!("merge_many_single_message_proof failed: {e:?}")))?;

    let pks_per_component: Vec<Vec<Vec<u8>>> =
        multi_message_proof.info.iter().map(|info| pks_to_ssz(&info.pubkeys)).collect();
    Ok((pks_per_component, multi_message_proof.compress_without_pubkeys()))
}

/// Verify a multi-message proof.
///
/// Args:
///     pub_keys_per_component: List of SSZ-encoded pubkey lists, one per component.
///     sig_bytes: multi-message proof bytes (compressed without pubkeys).
#[pyfunction(name = "verify_multi_message_proof")]
fn py_verify_multi_message_proof(
    pub_keys_per_component: Vec<Vec<Vec<u8>>>,
    sig_bytes: Vec<u8>,
) -> PyResult<()> {
    let pks_per_component: Vec<Vec<XmssPublicKey>> = pub_keys_per_component
        .iter()
        .enumerate()
        .map(|(i, pks_bytes)| deserialize_pub_keys(pks_bytes, &format!("component {i}")))
        .collect::<PyResult<_>>()?;

    let sig = MultiMessageAggregateSignature::decompress_without_pubkeys(&sig_bytes, pks_per_component)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress multi-message proof"))?;

    verify_multi_message_aggregate(&sig)
        .map_err(|e| PyValueError::new_err(format!("Verification failed: {e:?}")))?;
    Ok(())
}

/// Verify a multi-message proof and bind each component to an expected
/// (message_hash, slot) pair.
///
/// Like `verify_multi_message_proof`, but additionally checks that component `i` of the
/// signature attests to `expected_messages[i]`. The order of
/// `expected_messages` must match the order of `pub_keys_per_component`.
///
/// Args:
///     pub_keys_per_component: List of SSZ-encoded pubkey lists, one per component.
///     expected_messages: List of `(message_hash, slot)` tuples, one per component,
///         where `message_hash` is exactly 32 bytes and `slot` is a u32.
///     sig_bytes: multi-message proof bytes (compressed without pubkeys).
///
/// Raises:
///     ValueError if the SNARK fails, the component count mismatches, or any
///     component's (message, slot) does not match the expected pair.
#[pyfunction(name = "verify_multi_message_proof_with_messages")]
fn py_verify_multi_message_proof_with_messages(
    pub_keys_per_component: Vec<Vec<Vec<u8>>>,
    expected_messages: Vec<(Vec<u8>, u32)>,
    sig_bytes: Vec<u8>,
) -> PyResult<()> {
    if pub_keys_per_component.len() != expected_messages.len() {
        return Err(PyValueError::new_err(format!(
            "Number of pubkey-lists ({}) must match number of expected messages ({})",
            pub_keys_per_component.len(),
            expected_messages.len()
        )));
    }

    let expected: Vec<([u8; MESSAGE_LENGTH], u32)> = expected_messages
        .into_iter()
        .map(|(msg, slot)| Ok((message_to_array(msg)?, slot)))
        .collect::<PyResult<_>>()?;

    let pks_per_component: Vec<Vec<XmssPublicKey>> = pub_keys_per_component
        .iter()
        .enumerate()
        .map(|(i, pks_bytes)| deserialize_pub_keys(pks_bytes, &format!("component {i}")))
        .collect::<PyResult<_>>()?;

    let sig = MultiMessageAggregateSignature::decompress_without_pubkeys(&sig_bytes, pks_per_component)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress multi-message proof"))?;

    if sig.info.len() != expected.len() {
        return Err(PyValueError::new_err(format!(
            "Signature has {} components but {} expected messages were given",
            sig.info.len(),
            expected.len()
        )));
    }

    for (i, (info, (exp_msg, exp_slot))) in sig.info.iter().zip(&expected).enumerate() {
        if info.without_pubkeys.message != *exp_msg {
            return Err(PyValueError::new_err(format!(
                "Component {i}: message_hash does not match signature"
            )));
        }
        if info.without_pubkeys.slot != *exp_slot {
            return Err(PyValueError::new_err(format!(
                "Component {i}: slot does not match signature (expected {}, got {})",
                exp_slot, info.without_pubkeys.slot
            )));
        }
    }

    verify_multi_message_aggregate(&sig)
        .map_err(|e| PyValueError::new_err(format!("Verification failed: {e:?}")))?;
    Ok(())
}

/// Recover an independent single-message proof for the component at `index` from a multi-message proof.
///
/// Args:
///     pub_keys_per_component: List of SSZ-encoded pubkey lists, one per multi-message component.
///     sig_bytes: multi-message proof bytes (compressed without pubkeys).
///     index: Index of the component to extract.
///     log_inv_rate: Inverse rate exponent for the new single-message proof.
///
/// Returns:
///     `(pks_ssz, single_message_proof_bytes)` for the extracted component.
#[pyfunction(name = "split_multi_message_proof")]
fn py_split_multi_message_proof(
    pub_keys_per_component: Vec<Vec<Vec<u8>>>,
    sig_bytes: Vec<u8>,
    index: usize,
    log_inv_rate: usize,
) -> PyResult<(Vec<Vec<u8>>, Vec<u8>)> {
    let pks_per_component: Vec<Vec<XmssPublicKey>> = pub_keys_per_component
        .iter()
        .enumerate()
        .map(|(i, pks_bytes)| deserialize_pub_keys(pks_bytes, &format!("component {i}")))
        .collect::<PyResult<_>>()?;

    let multi_message_proof = MultiMessageAggregateSignature::decompress_without_pubkeys(&sig_bytes, pks_per_component)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress multi-message proof"))?;

    let single_message_proof = catch_panic(
        || split_multi_message_aggregate(multi_message_proof, index, log_inv_rate),
        "split_multi_message_proof",
    )?
    .map_err(|e| PyValueError::new_err(format!("split_multi_message_proof failed: {e:?}")))?;

    let pks_ssz = pks_to_ssz(&single_message_proof.info.pubkeys);
    Ok((pks_ssz, single_message_proof.compress_without_pubkeys()))
}

/// Recover an independent single-message proof by message from a multi-message proof.
///
/// Args:
///     pub_keys_per_component: List of SSZ-encoded pubkey lists, one per multi-message component.
///     sig_bytes: multi-message proof bytes (compressed without pubkeys).
///     message_hash: 32-byte message hash identifying the target component.
///     log_inv_rate: Inverse rate exponent for the new single-message proof.
///
/// Returns:
///     `(pks_ssz, single_message_proof_bytes)` for the extracted component.
#[pyfunction(name = "split_multi_message_proof_by_message")]
fn py_split_multi_message_proof_by_message(
    pub_keys_per_component: Vec<Vec<Vec<u8>>>,
    sig_bytes: Vec<u8>,
    message_hash: Vec<u8>,
    log_inv_rate: usize,
) -> PyResult<(Vec<Vec<u8>>, Vec<u8>)> {
    let message_array = message_to_array(message_hash)?;
    let pks_per_component: Vec<Vec<XmssPublicKey>> = pub_keys_per_component
        .iter()
        .enumerate()
        .map(|(i, pks_bytes)| deserialize_pub_keys(pks_bytes, &format!("component {i}")))
        .collect::<PyResult<_>>()?;

    let multi_message_proof = MultiMessageAggregateSignature::decompress_without_pubkeys(&sig_bytes, pks_per_component)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress multi-message proof"))?;

    let single_message_proof = catch_panic(
        || split_multi_message_aggregate_by_message(multi_message_proof, message_array, log_inv_rate),
        "split_multi_message_proof_by_message",
    )?
    .map_err(|e| PyValueError::new_err(format!("split_multi_message_proof_by_message failed: {e:?}")))?;

    let pks_ssz = pks_to_ssz(&single_message_proof.info.pubkeys);
    Ok((pks_ssz, single_message_proof.compress_without_pubkeys()))
}

/// Re-serialize a single-message proof with pubkeys bundled into the blob.
///
/// Args:
///     pub_keys_bytes: SSZ-encoded pubkeys (as returned alongside the no-pubkeys blob).
///     sig_bytes: single-message proof blob produced by `compress_without_pubkeys` (the form returned
///         by `aggregate_single_message` and `split_multi_message_proof`).
///
/// Returns:
///     A self-contained single-message proof blob (`SingleMessageAggregateSignature::compress()` form).
#[pyfunction]
fn single_message_proof_compress_with_pubkeys(
    pub_keys_bytes: Vec<Vec<u8>>,
    sig_bytes: Vec<u8>,
) -> PyResult<Vec<u8>> {
    let pks = deserialize_pub_keys(&pub_keys_bytes, "single-message")?;
    let sig = SingleMessageAggregateSignature::decompress_without_pubkeys(&sig_bytes, pks)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress single-message proof"))?;
    Ok(sig.compress())
}

/// Split a self-contained single-message proof blob back into (pubkeys_ssz, no-pubkeys-blob).
///
/// Args:
///     sig_bytes: A single-message proof blob produced by `compress()` (pubkeys bundled in).
///
/// Returns:
///     `(pks_ssz, single_message_proof_no_pubkeys_bytes)` — the same shape `aggregate_single_message` returns.
#[pyfunction]
fn single_message_proof_decompress_with_pubkeys(sig_bytes: Vec<u8>) -> PyResult<(Vec<Vec<u8>>, Vec<u8>)> {
    let sig = SingleMessageAggregateSignature::decompress(&sig_bytes)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress single-message proof"))?;
    let pks_ssz = pks_to_ssz(&sig.info.pubkeys);
    Ok((pks_ssz, sig.compress_without_pubkeys()))
}

/// Strip the pubkeys from a self-contained single-message proof blob, returning only the
/// compact wire form.
///
/// Useful when storing the bundled form locally but propagating only the
/// no-pubkeys form over the network (recipient is assumed to know the pubkeys).
///
/// Args:
///     sig_bytes: A single-message proof blob produced by `compress()` (pubkeys bundled in).
///
/// Returns:
///     `no_pubkeys_bytes` — the form returned by `aggregate_single_message`'s second element.
#[pyfunction]
fn single_message_proof_compress_without_pubkeys(sig_bytes: Vec<u8>) -> PyResult<Vec<u8>> {
    let sig = SingleMessageAggregateSignature::decompress(&sig_bytes)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress single-message proof"))?;
    Ok(sig.compress_without_pubkeys())
}

/// Re-serialize a multi-message proof with pubkeys bundled into the blob.
#[pyfunction]
fn multi_message_proof_compress_with_pubkeys(
    pub_keys_per_component: Vec<Vec<Vec<u8>>>,
    sig_bytes: Vec<u8>,
) -> PyResult<Vec<u8>> {
    let pks_per_component: Vec<Vec<XmssPublicKey>> = pub_keys_per_component
        .iter()
        .enumerate()
        .map(|(i, pks_bytes)| deserialize_pub_keys(pks_bytes, &format!("component {i}")))
        .collect::<PyResult<_>>()?;
    let sig = MultiMessageAggregateSignature::decompress_without_pubkeys(&sig_bytes, pks_per_component)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress multi-message proof"))?;
    Ok(sig.compress())
}

/// Split a self-contained multi-message proof blob back into (pubkeys_per_component_ssz, no-pubkeys-blob).
#[pyfunction]
fn multi_message_proof_decompress_with_pubkeys(sig_bytes: Vec<u8>) -> PyResult<(Vec<Vec<Vec<u8>>>, Vec<u8>)> {
    let sig = MultiMessageAggregateSignature::decompress(&sig_bytes)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress multi-message proof"))?;
    let pks_per_component: Vec<Vec<Vec<u8>>> =
        sig.info.iter().map(|info| pks_to_ssz(&info.pubkeys)).collect();
    Ok((pks_per_component, sig.compress_without_pubkeys()))
}

/// Strip the pubkeys from a self-contained multi-message proof blob, returning only the
/// compact wire form.
#[pyfunction]
fn multi_message_proof_compress_without_pubkeys(sig_bytes: Vec<u8>) -> PyResult<Vec<u8>> {
    let sig = MultiMessageAggregateSignature::decompress(&sig_bytes)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress multi-message proof"))?;
    Ok(sig.compress_without_pubkeys())
}

#[pyfunction]
fn ssz_encode_single_message_proof(sig_bytes: Vec<u8>) -> Vec<u8> {
    Devnet5SingleMessageProof { proof_bytes: sig_bytes }.as_ssz_bytes()
}

#[pyfunction]
fn ssz_decode_single_message_proof(ssz_bytes: Vec<u8>) -> PyResult<Vec<u8>> {
    let container = Devnet5SingleMessageProof::from_ssz_bytes(&ssz_bytes)
        .map_err(|e| PyValueError::new_err(format!("SSZ decode failed: {e:?}")))?;
    Ok(container.proof_bytes)
}

#[pyfunction]
fn ssz_encode_multi_message_proof(sig_bytes: Vec<u8>) -> Vec<u8> {
    Devnet5MultiMessageProof { proof_bytes: sig_bytes }.as_ssz_bytes()
}

#[pyfunction]
fn ssz_decode_multi_message_proof(ssz_bytes: Vec<u8>) -> PyResult<Vec<u8>> {
    let container = Devnet5MultiMessageProof::from_ssz_bytes(&ssz_bytes)
        .map_err(|e| PyValueError::new_err(format!("SSZ decode failed: {e:?}")))?;
    Ok(container.proof_bytes)
}

#[pyfunction]
fn get_mode() -> &'static str {
    MODE
}

fn register_functions(py_module: &Bound<'_, PyModule>) -> PyResult<()> {
    py_module.add("MODE", MODE)?;
    py_module.add_function(wrap_pyfunction!(setup_prover, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(setup_verifier, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(py_aggregate_single_message, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(py_verify_single_message_proof, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(py_merge_many_single_message_proof, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(py_verify_multi_message_proof, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(py_verify_multi_message_proof_with_messages, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(py_split_multi_message_proof, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(py_split_multi_message_proof_by_message, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(single_message_proof_compress_with_pubkeys, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(single_message_proof_decompress_with_pubkeys, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(single_message_proof_compress_without_pubkeys, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(multi_message_proof_compress_with_pubkeys, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(multi_message_proof_decompress_with_pubkeys, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(multi_message_proof_compress_without_pubkeys, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(ssz_encode_single_message_proof, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(ssz_decode_single_message_proof, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(ssz_encode_multi_message_proof, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(ssz_decode_multi_message_proof, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(get_mode, py_module)?)?;
    Ok(())
}

#[cfg(feature = "test-config")]
#[pymodule]
fn lean_multisig_test(py_module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_functions(py_module)
}

#[cfg(not(feature = "test-config"))]
#[pymodule]
fn lean_multisig(py_module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_functions(py_module)
}
