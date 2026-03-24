use ::lean_multisig::{
    setup_prover as lm_setup_prover, setup_verifier as lm_setup_verifier, xmss_aggregate,
    xmss_verify_aggregation, AggregatedXMSS,
};
use leansig_wrapper::{XmssPublicKey, XmssSignature, MESSAGE_LENGTH};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use ssz::{Decode, Encode};

/// Setup the prover for XMSS aggregation.
/// Call this once before first aggregation to avoid slowdown.
#[pyfunction]
fn setup_prover() {
    lm_setup_prover();
}

/// Setup the verifier for XMSS aggregation.
/// Call this once before first verification to avoid slowdown.
#[pyfunction]
fn setup_verifier() {
    lm_setup_verifier();
}

/// Aggregate XMSS signatures.
///
/// Args:
///     pub_keys_bytes: List of serialized public keys (each as bytes, postcard format)
///     signatures_bytes: List of serialized signatures (each as bytes, postcard format)
///     message_hash: 32-byte message hash
///     slot: Slot number (u32)
///     log_inv_rate: Inverse rate exponent for proof (1-4, lower = faster but bigger proofs)
///     children_bytes: Optional list of serialized AggregatedXMSS for hierarchical aggregation
///
/// Returns:
///     Serialized aggregated signature as bytes
///
/// Raises:
///     ValueError: If inputs are invalid or aggregation fails
#[pyfunction]
#[pyo3(signature = (pub_keys_bytes, signatures_bytes, message_hash, slot, log_inv_rate, children_bytes=None))]
fn aggregate_signatures(
    pub_keys_bytes: Vec<Vec<u8>>,
    signatures_bytes: Vec<Vec<u8>>,
    message_hash: Vec<u8>,
    slot: u32,
    log_inv_rate: usize,
    children_bytes: Option<Vec<Vec<u8>>>,
) -> PyResult<Vec<u8>> {
    // Validate message hash length
    if message_hash.len() != MESSAGE_LENGTH {
        return Err(PyValueError::new_err(format!(
            "message_hash must be exactly {} bytes, got {}",
            MESSAGE_LENGTH,
            message_hash.len()
        )));
    }

    // Validate that we have the same number of public keys and signatures
    if pub_keys_bytes.len() != signatures_bytes.len() {
        return Err(PyValueError::new_err(format!(
            "Number of public keys ({}) must match number of signatures ({})",
            pub_keys_bytes.len(),
            signatures_bytes.len()
        )));
    }

    // Deserialize public keys
    let pub_keys: Result<Vec<XmssPublicKey>, _> = pub_keys_bytes
        .iter()
        .map(|bytes| postcard::from_bytes(bytes))
        .collect();
    let pub_keys = pub_keys.map_err(|e| {
        PyValueError::new_err(format!("Failed to deserialize public key: {:?}", e))
    })?;

    // Deserialize signatures
    let signatures: Result<Vec<XmssSignature>, _> = signatures_bytes
        .iter()
        .map(|bytes| postcard::from_bytes(bytes))
        .collect();
    let signatures = signatures.map_err(|e| {
        PyValueError::new_err(format!("Failed to deserialize signature: {:?}", e))
    })?;

    // Build raw_xmss pairs
    let raw_xmss: Vec<(XmssPublicKey, XmssSignature)> =
        pub_keys.into_iter().zip(signatures).collect();

    // Deserialize children if provided
    let children: Vec<AggregatedXMSS> = match children_bytes {
        Some(cb) => cb
            .iter()
            .map(|b| {
                AggregatedXMSS::deserialize(b)
                    .ok_or_else(|| PyValueError::new_err("Failed to deserialize child aggregation"))
            })
            .collect::<PyResult<Vec<_>>>()?,
        None => vec![],
    };

    // Convert message_hash to array
    let message_array: [u8; MESSAGE_LENGTH] = message_hash
        .try_into()
        .map_err(|_| PyValueError::new_err("Failed to convert message_hash to fixed-size array"))?;

    // Call the aggregation function
    let agg = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        xmss_aggregate(&children, raw_xmss, &message_array, slot, log_inv_rate)
    }))
    .map_err(|e| {
        let msg = if let Some(s) = e.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = e.downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown error".to_string()
        };
        PyValueError::new_err(format!("Aggregation failed: {}", msg))
    })?;

    // Serialize using native method
    Ok(agg.serialize())
}

/// Verify aggregated XMSS signatures.
///
/// Args:
///     message_hash: 32-byte message hash
///     agg_signature_bytes: Serialized aggregated signature as bytes
///     slot: Slot number (u32)
///
/// Returns:
///     None if verification succeeds
///
/// Raises:
///     ValueError: If inputs are invalid or verification fails
#[pyfunction]
fn verify_aggregated_signatures(
    message_hash: Vec<u8>,
    agg_signature_bytes: Vec<u8>,
    slot: u32,
) -> PyResult<()> {
    // Validate message hash length
    if message_hash.len() != MESSAGE_LENGTH {
        return Err(PyValueError::new_err(format!(
            "message_hash must be exactly {} bytes, got {}",
            MESSAGE_LENGTH,
            message_hash.len()
        )));
    }

    // Convert message_hash to array
    let message_array: [u8; MESSAGE_LENGTH] = message_hash
        .try_into()
        .map_err(|_| PyValueError::new_err("Failed to convert message_hash to fixed-size array"))?;

    // Deserialize the aggregated signature
    let agg_sig = AggregatedXMSS::deserialize(&agg_signature_bytes)
        .ok_or_else(|| PyValueError::new_err("Failed to deserialize aggregated signature"))?;

    // Call the verification function
    xmss_verify_aggregation(&agg_sig, &message_array, slot)
        .map_err(|e| PyValueError::new_err(format!("Verification failed: {:?}", e)))?;

    Ok(())
}

/// SSZ-encode an aggregated signature.
///
/// Wraps the native serialized bytes as an SSZ byte list.
///
/// Args:
///     agg_signature_bytes: Serialized aggregated signature (from aggregate_signatures)
///
/// Returns:
///     SSZ-encoded bytes
#[pyfunction]
fn ssz_encode_aggregate_signature(agg_signature_bytes: Vec<u8>) -> Vec<u8> {
    agg_signature_bytes.as_ssz_bytes()
}

/// SSZ-decode an aggregated signature.
///
/// Args:
///     ssz_bytes: SSZ-encoded aggregated signature bytes
///
/// Returns:
///     Native serialized aggregated signature bytes
///
/// Raises:
///     ValueError: If SSZ decoding fails
#[pyfunction]
fn ssz_decode_aggregate_signature(ssz_bytes: Vec<u8>) -> PyResult<Vec<u8>> {
    Vec::<u8>::from_ssz_bytes(&ssz_bytes)
        .map_err(|e| PyValueError::new_err(format!("SSZ decode failed: {:?}", e)))
}

/// Python module for lean-multisig XMSS aggregation
#[pymodule]
fn lean_multisig(py_module: &Bound<'_, PyModule>) -> PyResult<()> {
    py_module.add_function(wrap_pyfunction!(setup_prover, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(setup_verifier, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(aggregate_signatures, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(verify_aggregated_signatures, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(ssz_encode_aggregate_signature, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(ssz_decode_aggregate_signature, py_module)?)?;
    Ok(())
}
