use backend::{precompute_dft_twiddles, KoalaBear};
use leansig_wrapper::{
    xmss_public_key_from_ssz, xmss_public_key_to_ssz, xmss_signature_from_ssz, XmssPublicKey,
    XmssSignature, MESSAGE_LENGTH,
};
use rec_aggregation::{
    aggregate_type_1, init_aggregation_bytecode, merge_many_type_1, split_type_2,
    split_type_2_by_msg, verify_type_1, verify_type_2, TypeOneMultiSignature, TypeTwoMultiSignature,
};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use ssz::{Decode, DecodeError, Encode};

#[cfg(feature = "test-config")]
pub const MODE: &str = "test";

#[cfg(not(feature = "test-config"))]
pub const MODE: &str = "prod";

/// Opaque SSZ container wrapping the compressed bytes of a Type-1 multi-signature
/// (postcard + lz4, produced by `TypeOneMultiSignature::compress_without_pubkeys`).
#[derive(Debug, Clone)]
pub struct Devnet5Type1Signature {
    pub proof_bytes: Vec<u8>,
}

impl Encode for Devnet5Type1Signature {
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

impl Decode for Devnet5Type1Signature {
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

/// Opaque SSZ container wrapping the compressed bytes of a Type-2 multi-signature
/// (postcard + lz4, produced by `TypeTwoMultiSignature::compress_without_pubkeys`).
#[derive(Debug, Clone)]
pub struct Devnet5Type2Signature {
    pub proof_bytes: Vec<u8>,
}

impl Encode for Devnet5Type2Signature {
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

impl Decode for Devnet5Type2Signature {
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

/// Aggregate XMSS signatures into a Type-1 multi-signature (single message, single slot).
///
/// Args:
///     pub_keys_bytes: SSZ-encoded public keys, one per raw signature.
///     signatures_bytes: SSZ-encoded raw XMSS signatures, paired with `pub_keys_bytes`.
///     message_hash: 32-byte message hash.
///     slot: Slot number.
///     log_inv_rate: Inverse rate exponent for the proof.
///     children_bytes: Optional list of `(child_pub_keys_ssz, child_type1_bytes)` tuples,
///         where `child_type1_bytes` is a Type-1 signature produced by a prior call
///         (compressed without pubkeys).
///
/// Returns:
///     `(sorted_pub_keys_ssz, type1_bytes)` — pubkeys are returned sorted+deduplicated,
///     `type1_bytes` is the Type-1 signature compressed without pubkeys.
#[pyfunction(name = "aggregate_type_1")]
#[pyo3(signature = (pub_keys_bytes, signatures_bytes, message_hash, slot, log_inv_rate, children_bytes=None))]
fn py_aggregate_type_1(
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

    let children: Vec<TypeOneMultiSignature> = match children_bytes {
        Some(cb) => cb
            .into_iter()
            .enumerate()
            .map(|(i, (child_pks_bytes, child_bytes))| {
                let child_pks = deserialize_pub_keys(&child_pks_bytes, &format!("child {i}"))?;
                TypeOneMultiSignature::decompress_without_pubkeys(&child_bytes, child_pks).ok_or_else(
                    || PyValueError::new_err(format!("Failed to decompress child {i} (Type 1)")),
                )
            })
            .collect::<PyResult<_>>()?,
        None => Vec::new(),
    };

    let agg = catch_panic(
        || aggregate_type_1(&children, raw_xmss, message_array, slot, log_inv_rate),
        "Type-1 aggregation",
    )?
    .map_err(|e| PyValueError::new_err(format!("Type-1 aggregation failed: {e:?}")))?;

    let sorted_pks_ssz = pks_to_ssz(&agg.info.pubkeys);
    Ok((sorted_pks_ssz, agg.compress_without_pubkeys()))
}

/// Verify a Type-1 multi-signature.
///
/// Args:
///     pub_keys_bytes: SSZ-encoded sorted+deduplicated public keys (as returned by aggregate).
///     message_hash: 32-byte message hash.
///     slot: Slot number.
///     sig_bytes: Type-1 signature bytes (compressed without pubkeys).
///
/// Raises:
///     ValueError on any failure.
#[pyfunction(name = "verify_type_1")]
fn py_verify_type_1(
    pub_keys_bytes: Vec<Vec<u8>>,
    message_hash: Vec<u8>,
    slot: u32,
    sig_bytes: Vec<u8>,
) -> PyResult<()> {
    let message_array = message_to_array(message_hash)?;
    let pub_keys = deserialize_pub_keys(&pub_keys_bytes, "type-1")?;
    let sig = TypeOneMultiSignature::decompress_without_pubkeys(&sig_bytes, pub_keys)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress Type-1 signature"))?;

    if sig.info.without_pubkeys.message != message_array {
        return Err(PyValueError::new_err("message_hash does not match signature"));
    }
    if sig.info.without_pubkeys.slot != slot {
        return Err(PyValueError::new_err("slot does not match signature"));
    }

    verify_type_1(&sig).map_err(|e| PyValueError::new_err(format!("Verification failed: {e:?}")))?;
    Ok(())
}

/// Merge multiple Type-1 multi-signatures (potentially over different messages/slots)
/// into a single Type-2 multi-signature.
///
/// Args:
///     type1_entries: List of `(pub_keys_ssz, type1_bytes)` tuples.
///     log_inv_rate: Inverse rate exponent for the proof.
///
/// Returns:
///     `(pks_per_component_ssz, type2_bytes)` — `pks_per_component_ssz[i]` is the
///     SSZ-encoded pubkey list for component `i`; `type2_bytes` is the Type-2
///     signature compressed without pubkeys.
#[pyfunction(name = "merge_many_type_1")]
fn py_merge_many_type_1(
    type1_entries: Vec<(Vec<Vec<u8>>, Vec<u8>)>,
    log_inv_rate: usize,
) -> PyResult<(Vec<Vec<Vec<u8>>>, Vec<u8>)> {
    if type1_entries.is_empty() {
        return Err(PyValueError::new_err("merge_many_type_1 requires at least one entry"));
    }

    let types_1: Vec<TypeOneMultiSignature> = type1_entries
        .into_iter()
        .enumerate()
        .map(|(i, (pks_bytes, sig_bytes))| {
            let pks = deserialize_pub_keys(&pks_bytes, &format!("component {i}"))?;
            TypeOneMultiSignature::decompress_without_pubkeys(&sig_bytes, pks).ok_or_else(|| {
                PyValueError::new_err(format!("Failed to decompress Type-1 component {i}"))
            })
        })
        .collect::<PyResult<_>>()?;

    let type_2 = catch_panic(
        || merge_many_type_1(types_1, log_inv_rate),
        "merge_many_type_1",
    )?
    .map_err(|e| PyValueError::new_err(format!("merge_many_type_1 failed: {e:?}")))?;

    let pks_per_component: Vec<Vec<Vec<u8>>> =
        type_2.info.iter().map(|info| pks_to_ssz(&info.pubkeys)).collect();
    Ok((pks_per_component, type_2.compress_without_pubkeys()))
}

/// Verify a Type-2 multi-signature.
///
/// Args:
///     pub_keys_per_component: List of SSZ-encoded pubkey lists, one per component.
///     sig_bytes: Type-2 signature bytes (compressed without pubkeys).
#[pyfunction(name = "verify_type_2")]
fn py_verify_type_2(
    pub_keys_per_component: Vec<Vec<Vec<u8>>>,
    sig_bytes: Vec<u8>,
) -> PyResult<()> {
    let pks_per_component: Vec<Vec<XmssPublicKey>> = pub_keys_per_component
        .iter()
        .enumerate()
        .map(|(i, pks_bytes)| deserialize_pub_keys(pks_bytes, &format!("component {i}")))
        .collect::<PyResult<_>>()?;

    let sig = TypeTwoMultiSignature::decompress_without_pubkeys(&sig_bytes, pks_per_component)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress Type-2 signature"))?;

    verify_type_2(&sig).map_err(|e| PyValueError::new_err(format!("Verification failed: {e:?}")))?;
    Ok(())
}

/// Verify a Type-2 multi-signature and bind each component to an expected
/// (message_hash, slot) pair.
///
/// Like `verify_type_2`, but additionally checks that component `i` of the
/// signature attests to `expected_messages[i]`. The order of
/// `expected_messages` must match the order of `pub_keys_per_component`.
///
/// Args:
///     pub_keys_per_component: List of SSZ-encoded pubkey lists, one per component.
///     expected_messages: List of `(message_hash, slot)` tuples, one per component,
///         where `message_hash` is exactly 32 bytes and `slot` is a u32.
///     sig_bytes: Type-2 signature bytes (compressed without pubkeys).
///
/// Raises:
///     ValueError if the SNARK fails, the component count mismatches, or any
///     component's (message, slot) does not match the expected pair.
#[pyfunction(name = "verify_type_2_with_messages")]
fn py_verify_type_2_with_messages(
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

    let sig = TypeTwoMultiSignature::decompress_without_pubkeys(&sig_bytes, pks_per_component)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress Type-2 signature"))?;

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

    verify_type_2(&sig).map_err(|e| PyValueError::new_err(format!("Verification failed: {e:?}")))?;
    Ok(())
}

/// Recover an independent Type-1 multi-signature for the component at `index` from a Type-2.
///
/// Args:
///     pub_keys_per_component: List of SSZ-encoded pubkey lists, one per Type-2 component.
///     sig_bytes: Type-2 signature bytes (compressed without pubkeys).
///     index: Index of the component to extract.
///     log_inv_rate: Inverse rate exponent for the new Type-1 proof.
///
/// Returns:
///     `(pks_ssz, type1_bytes)` for the extracted component.
#[pyfunction(name = "split_type_2")]
fn py_split_type_2(
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

    let type_2 = TypeTwoMultiSignature::decompress_without_pubkeys(&sig_bytes, pks_per_component)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress Type-2 signature"))?;

    let type_1 = catch_panic(
        || split_type_2(type_2, index, log_inv_rate),
        "split_type_2",
    )?
    .map_err(|e| PyValueError::new_err(format!("split_type_2 failed: {e:?}")))?;

    let pks_ssz = pks_to_ssz(&type_1.info.pubkeys);
    Ok((pks_ssz, type_1.compress_without_pubkeys()))
}

/// Recover an independent Type-1 multi-signature by message from a Type-2.
///
/// Args:
///     pub_keys_per_component: List of SSZ-encoded pubkey lists, one per Type-2 component.
///     sig_bytes: Type-2 signature bytes (compressed without pubkeys).
///     message_hash: 32-byte message hash identifying the target component.
///     log_inv_rate: Inverse rate exponent for the new Type-1 proof.
///
/// Returns:
///     `(pks_ssz, type1_bytes)` for the extracted component.
#[pyfunction(name = "split_type_2_by_msg")]
fn py_split_type_2_by_msg(
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

    let type_2 = TypeTwoMultiSignature::decompress_without_pubkeys(&sig_bytes, pks_per_component)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress Type-2 signature"))?;

    let type_1 = catch_panic(
        || split_type_2_by_msg(type_2, message_array, log_inv_rate),
        "split_type_2_by_msg",
    )?
    .map_err(|e| PyValueError::new_err(format!("split_type_2_by_msg failed: {e:?}")))?;

    let pks_ssz = pks_to_ssz(&type_1.info.pubkeys);
    Ok((pks_ssz, type_1.compress_without_pubkeys()))
}

/// Re-serialize a Type-1 multi-signature with pubkeys bundled into the blob.
///
/// Args:
///     pub_keys_bytes: SSZ-encoded pubkeys (as returned alongside the no-pubkeys blob).
///     sig_bytes: Type-1 blob produced by `compress_without_pubkeys` (the form returned
///         by `aggregate_type_1` and `split_type_2`).
///
/// Returns:
///     A self-contained Type-1 blob (`TypeOneMultiSignature::compress()` form).
#[pyfunction]
fn type1_compress_with_pubkeys(
    pub_keys_bytes: Vec<Vec<u8>>,
    sig_bytes: Vec<u8>,
) -> PyResult<Vec<u8>> {
    let pks = deserialize_pub_keys(&pub_keys_bytes, "type-1")?;
    let sig = TypeOneMultiSignature::decompress_without_pubkeys(&sig_bytes, pks)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress Type-1 signature"))?;
    Ok(sig.compress())
}

/// Split a self-contained Type-1 blob back into (pubkeys_ssz, no-pubkeys-blob).
///
/// Args:
///     sig_bytes: A Type-1 blob produced by `compress()` (pubkeys bundled in).
///
/// Returns:
///     `(pks_ssz, type1_no_pubkeys_bytes)` — the same shape `aggregate_type_1` returns.
#[pyfunction]
fn type1_decompress_with_pubkeys(sig_bytes: Vec<u8>) -> PyResult<(Vec<Vec<u8>>, Vec<u8>)> {
    let sig = TypeOneMultiSignature::decompress(&sig_bytes)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress Type-1 signature"))?;
    let pks_ssz = pks_to_ssz(&sig.info.pubkeys);
    Ok((pks_ssz, sig.compress_without_pubkeys()))
}

/// Strip the pubkeys from a self-contained Type-1 blob, returning only the
/// compact wire form.
///
/// Useful when storing the bundled form locally but propagating only the
/// no-pubkeys form over the network (recipient is assumed to know the pubkeys).
///
/// Args:
///     sig_bytes: A Type-1 blob produced by `compress()` (pubkeys bundled in).
///
/// Returns:
///     `no_pubkeys_bytes` — the form returned by `aggregate_type_1`'s second element.
#[pyfunction]
fn type1_compress_without_pubkeys(sig_bytes: Vec<u8>) -> PyResult<Vec<u8>> {
    let sig = TypeOneMultiSignature::decompress(&sig_bytes)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress Type-1 signature"))?;
    Ok(sig.compress_without_pubkeys())
}

/// Re-serialize a Type-2 multi-signature with pubkeys bundled into the blob.
#[pyfunction]
fn type2_compress_with_pubkeys(
    pub_keys_per_component: Vec<Vec<Vec<u8>>>,
    sig_bytes: Vec<u8>,
) -> PyResult<Vec<u8>> {
    let pks_per_component: Vec<Vec<XmssPublicKey>> = pub_keys_per_component
        .iter()
        .enumerate()
        .map(|(i, pks_bytes)| deserialize_pub_keys(pks_bytes, &format!("component {i}")))
        .collect::<PyResult<_>>()?;
    let sig = TypeTwoMultiSignature::decompress_without_pubkeys(&sig_bytes, pks_per_component)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress Type-2 signature"))?;
    Ok(sig.compress())
}

/// Split a self-contained Type-2 blob back into (pubkeys_per_component_ssz, no-pubkeys-blob).
#[pyfunction]
fn type2_decompress_with_pubkeys(sig_bytes: Vec<u8>) -> PyResult<(Vec<Vec<Vec<u8>>>, Vec<u8>)> {
    let sig = TypeTwoMultiSignature::decompress(&sig_bytes)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress Type-2 signature"))?;
    let pks_per_component: Vec<Vec<Vec<u8>>> =
        sig.info.iter().map(|info| pks_to_ssz(&info.pubkeys)).collect();
    Ok((pks_per_component, sig.compress_without_pubkeys()))
}

/// Strip the pubkeys from a self-contained Type-2 blob, returning only the
/// compact wire form.
#[pyfunction]
fn type2_compress_without_pubkeys(sig_bytes: Vec<u8>) -> PyResult<Vec<u8>> {
    let sig = TypeTwoMultiSignature::decompress(&sig_bytes)
        .ok_or_else(|| PyValueError::new_err("Failed to decompress Type-2 signature"))?;
    Ok(sig.compress_without_pubkeys())
}

#[pyfunction]
fn ssz_encode_type1_signature(sig_bytes: Vec<u8>) -> Vec<u8> {
    Devnet5Type1Signature { proof_bytes: sig_bytes }.as_ssz_bytes()
}

#[pyfunction]
fn ssz_decode_type1_signature(ssz_bytes: Vec<u8>) -> PyResult<Vec<u8>> {
    let container = Devnet5Type1Signature::from_ssz_bytes(&ssz_bytes)
        .map_err(|e| PyValueError::new_err(format!("SSZ decode failed: {e:?}")))?;
    Ok(container.proof_bytes)
}

#[pyfunction]
fn ssz_encode_type2_signature(sig_bytes: Vec<u8>) -> Vec<u8> {
    Devnet5Type2Signature { proof_bytes: sig_bytes }.as_ssz_bytes()
}

#[pyfunction]
fn ssz_decode_type2_signature(ssz_bytes: Vec<u8>) -> PyResult<Vec<u8>> {
    let container = Devnet5Type2Signature::from_ssz_bytes(&ssz_bytes)
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
    py_module.add_function(wrap_pyfunction!(py_aggregate_type_1, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(py_verify_type_1, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(py_merge_many_type_1, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(py_verify_type_2, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(py_verify_type_2_with_messages, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(py_split_type_2, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(py_split_type_2_by_msg, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(type1_compress_with_pubkeys, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(type1_decompress_with_pubkeys, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(type1_compress_without_pubkeys, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(type2_compress_with_pubkeys, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(type2_decompress_with_pubkeys, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(type2_compress_without_pubkeys, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(ssz_encode_type1_signature, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(ssz_decode_type1_signature, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(ssz_encode_type2_signature, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(ssz_decode_type2_signature, py_module)?)?;
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
