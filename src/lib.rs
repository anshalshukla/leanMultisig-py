use ::lean_multisig::{
    xmss_aggregate_signatures, xmss_aggregation_setup_prover, xmss_aggregation_setup_verifier,
    xmss_verify_aggregated_signatures, XmssPublicKey, XmssSignature, F,
    PrimeCharacteristicRing,
};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// Setup the prover for XMSS aggregation (precomputes DFT twiddles).
/// Call this once before first aggregation to avoid slowdown.
#[pyfunction]
fn setup_prover() {
    xmss_aggregation_setup_prover();
}

/// Setup the verifier for XMSS aggregation.
/// Call this once before first verification to avoid slowdown.
#[pyfunction]
fn setup_verifier() {
    xmss_aggregation_setup_verifier();
}

/// Aggregate XMSS signatures.
///
/// Args:
///     pub_keys_bytes: List of serialized public keys (each as bytes)
///     signatures_bytes: List of serialized signatures (each as bytes)
///     message_hash: List of 8 field elements (u64 values)
///     slot: Slot number
///     test_mode: If True, returns a dummy signature without actual aggregation
///
/// Returns:
///     Serialized aggregated signature as bytes
///
/// Raises:
///     ValueError: If inputs are invalid or aggregation fails
#[pyfunction]
fn aggregate_signatures(
    pub_keys_bytes: Vec<Vec<u8>>,
    signatures_bytes: Vec<Vec<u8>>,
    message_hash: Vec<u64>,
    slot: u64,
    test_mode: bool,
) -> PyResult<Vec<u8>> {
    if test_mode {
        return Ok(vec![0u8; 1]);
    }

    // Validate message hash length
    if message_hash.len() != 8 {
        return Err(PyValueError::new_err(
            "message_hash must be exactly 8 field elements",
        ));
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
        .map(|bytes| bincode::deserialize(bytes))
        .collect();
    let pub_keys = pub_keys
        .map_err(|e| PyValueError::new_err(format!("Failed to deserialize public key: {}", e)))?;

    // Deserialize signatures
    let signatures: Result<Vec<XmssSignature>, _> = signatures_bytes
        .iter()
        .map(|bytes| bincode::deserialize(bytes))
        .collect();
    let signatures = signatures
        .map_err(|e| PyValueError::new_err(format!("Failed to deserialize signature: {}", e)))?;

    // Convert message_hash to field element array
    let message_array: [F; 8] = message_hash
        .iter()
        .map(|&v| F::from_u64(v))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    // Call the aggregation function
    let agg_sig = xmss_aggregate_signatures(&pub_keys, &signatures, message_array, slot)
        .map_err(|e| PyValueError::new_err(format!("Aggregation failed: {:?}", e)))?;

    Ok(agg_sig)
}

/// Verify aggregated XMSS signatures.
///
/// Args:
///     pub_keys_bytes: List of serialized public keys (each as bytes)
///     message_hash: List of 8 field elements (u64 values)
///     agg_signature_bytes: Serialized aggregated signature as bytes
///     slot: Slot number
///     test_mode: If True, skips actual verification
///
/// Returns:
///     None if verification succeeds
///
/// Raises:
///     ValueError: If inputs are invalid or verification fails
#[pyfunction]
fn verify_aggregated_signatures(
    pub_keys_bytes: Vec<Vec<u8>>,
    message_hash: Vec<u64>,
    agg_signature_bytes: Vec<u8>,
    slot: u64,
    test_mode: bool,
) -> PyResult<()> {
    if test_mode {
        return Ok(());
    }

    // Validate message hash length
    if message_hash.len() != 8 {
        return Err(PyValueError::new_err(
            "message_hash must be exactly 8 field elements",
        ));
    }

    // Deserialize public keys
    let pub_keys: Result<Vec<XmssPublicKey>, _> = pub_keys_bytes
        .iter()
        .map(|bytes| bincode::deserialize(bytes))
        .collect();
    let pub_keys = pub_keys
        .map_err(|e| PyValueError::new_err(format!("Failed to deserialize public key: {}", e)))?;

    // Convert message_hash to field element array
    let message_array: [F; 8] = message_hash
        .iter()
        .map(|&v| F::from_u64(v))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    // Call the verification function
    xmss_verify_aggregated_signatures(&pub_keys, message_array, &agg_signature_bytes, slot)
        .map_err(|e| PyValueError::new_err(format!("Verification failed: {:?}", e)))?;

    Ok(())
}

/// Python module for lean-multisig XMSS aggregation
#[pymodule]
fn lean_multisig(py_module: &Bound<'_, PyModule>) -> PyResult<()> {
    py_module.add_function(wrap_pyfunction!(setup_prover, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(setup_verifier, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(aggregate_signatures, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(verify_aggregated_signatures, py_module)?)?;
    Ok(())
}
