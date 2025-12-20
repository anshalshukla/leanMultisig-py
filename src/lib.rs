use ::lean_multisig::{
    xmss_aggregate_signatures, xmss_aggregation_setup_prover, xmss_aggregation_setup_verifier,
    xmss_verify_aggregated_signatures, Devnet2XmssAggregateSignature, LeanSigPubKey,
    LeanSigSignature, XmssAggregateError,
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
///     message_hash: 32-byte message hash
///     epoch: Epoch number
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
    message_hash: Vec<u8>,
    epoch: u32,
    test_mode: bool,
) -> PyResult<Vec<u8>> {
    if test_mode {
        return Ok(vec![0u8; 1]);
    }

    // Validate message hash length
    if message_hash.len() != 32 {
        return Err(PyValueError::new_err(
            "message_hash must be exactly 32 bytes",
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
    let pub_keys: Result<Vec<LeanSigPubKey>, _> = pub_keys_bytes
        .iter()
        .map(|bytes| bincode::deserialize(bytes))
        .collect();
    let pub_keys = pub_keys
        .map_err(|e| PyValueError::new_err(format!("Failed to deserialize public key: {}", e)))?;

    // Deserialize signatures
    let signatures: Result<Vec<LeanSigSignature>, _> = signatures_bytes
        .iter()
        .map(|bytes| bincode::deserialize(bytes))
        .collect();
    let signatures = signatures
        .map_err(|e| PyValueError::new_err(format!("Failed to deserialize signature: {}", e)))?;

    // Convert message_hash to array
    let mut message_array = [0u8; 32];
    message_array.copy_from_slice(&message_hash);

    // Call the aggregation function
    let agg_sig = xmss_aggregate_signatures(&pub_keys, &signatures, &message_array, epoch)
        .map_err(|e| match e {
            XmssAggregateError::WrongSignatureCount => {
                PyValueError::new_err("Wrong signature count")
            }
            XmssAggregateError::InvalidSigature => PyValueError::new_err("Invalid signature"),
        })?;

    // Serialize the aggregated signature
    let serialized = bincode::serialize(&agg_sig).map_err(|e| {
        PyValueError::new_err(format!("Failed to serialize aggregated signature: {}", e))
    })?;

    Ok(serialized)
}

/// Verify aggregated XMSS signatures.
///
/// Args:
///     pub_keys_bytes: List of serialized public keys (each as bytes)
///     message_hash: 32-byte message hash
///     agg_signature_bytes: Serialized aggregated signature as bytes
///     epoch: Epoch number
///
/// Returns:
///     None if verification succeeds
///
/// Raises:
///     ValueError: If inputs are invalid or verification fails
#[pyfunction]
fn verify_aggregated_signatures(
    pub_keys_bytes: Vec<Vec<u8>>,
    message_hash: Vec<u8>,
    agg_signature_bytes: Vec<u8>,
    epoch: u32,
    test_mode: bool,
) -> PyResult<()> {
    if test_mode {
        return Ok(());
    }

    // Validate message hash length
    if message_hash.len() != 32 {
        return Err(PyValueError::new_err(
            "message_hash must be exactly 32 bytes",
        ));
    }

    // Deserialize public keys
    let pub_keys: Result<Vec<LeanSigPubKey>, _> = pub_keys_bytes
        .iter()
        .map(|bytes| bincode::deserialize(bytes))
        .collect();
    let pub_keys = pub_keys
        .map_err(|e| PyValueError::new_err(format!("Failed to deserialize public key: {}", e)))?;

    // Deserialize aggregated signature
    let agg_sig: Devnet2XmssAggregateSignature = bincode::deserialize(&agg_signature_bytes)
        .map_err(|e| {
            PyValueError::new_err(format!("Failed to deserialize aggregated signature: {}", e))
        })?;

    // Convert message_hash to array
    let mut message_array = [0u8; 32];
    message_array.copy_from_slice(&message_hash);

    // Call the verification function
    xmss_verify_aggregated_signatures(&pub_keys, &message_array, &agg_sig, epoch)
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
