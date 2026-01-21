use lean_multisig::{
    Devnet2XmssAggregateSignature, config::LeanSigPubKey, config::LeanSigSignature,
    xmss_aggregate_signatures, xmss_aggregation_setup_prover, xmss_aggregation_setup_verifier,
    xmss_verify_aggregated_signatures,
};
use ssz::{Decode, Encode};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// Mode constant - "test" when compiled with test_config feature, "prod" otherwise.
#[cfg(feature = "test_config")]
pub const MODE: &str = "test";

#[cfg(not(feature = "test_config"))]
pub const MODE: &str = "prod";

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
///     pub_keys_bytes: List of serialized public keys (each as bytes, SSZ format)
///     signatures_bytes: List of serialized signatures (each as bytes, SSZ format)
///     message_hash: 32-byte message hash
///     epoch: Epoch number (u32)
///
/// Returns:
///     SSZ-encoded aggregated signature as bytes
///
/// Raises:
///     ValueError: If inputs are invalid or aggregation fails
#[pyfunction]
fn aggregate_signatures(
    pub_keys_bytes: Vec<Vec<u8>>,
    signatures_bytes: Vec<Vec<u8>>,
    message_hash: Vec<u8>,
    epoch: u32,
) -> PyResult<Vec<u8>> {
    // Validate message hash length
    if message_hash.len() != 32 {
        return Err(PyValueError::new_err(format!(
            "message_hash must be exactly 32 bytes, got {}",
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

    // Deserialize public keys using SSZ
    let pub_keys: Result<Vec<LeanSigPubKey>, _> = pub_keys_bytes
        .iter()
        .map(|bytes| LeanSigPubKey::from_ssz_bytes(bytes))
        .collect();
    let pub_keys = pub_keys
        .map_err(|e| PyValueError::new_err(format!("Failed to deserialize public key (SSZ): {:?}", e)))?;

    // Deserialize signatures using SSZ
    let signatures: Result<Vec<LeanSigSignature>, _> = signatures_bytes
        .iter()
        .map(|bytes| LeanSigSignature::from_ssz_bytes(bytes))
        .collect();
    let signatures = signatures
        .map_err(|e| PyValueError::new_err(format!("Failed to deserialize signature (SSZ): {:?}", e)))?;

    // Convert message_hash to array
    let message_array: [u8; 32] = message_hash
        .try_into()
        .map_err(|_| PyValueError::new_err("Failed to convert message_hash to [u8; 32]"))?;

    // Call the aggregation function
    let agg_sig = xmss_aggregate_signatures(&pub_keys, &signatures, &message_array, epoch)
        .map_err(|e| PyValueError::new_err(format!("Aggregation failed: {:?}", e)))?;

    // SSZ encode the result
    Ok(agg_sig.as_ssz_bytes())
}

/// Verify aggregated XMSS signatures.
///
/// Args:
///     pub_keys_bytes: List of serialized public keys (each as bytes, SSZ format)
///     message_hash: 32-byte message hash
///     agg_signature_ssz: SSZ-encoded aggregated signature as bytes
///     epoch: Epoch number (u32)
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
    agg_signature_ssz: Vec<u8>,
    epoch: u32,
) -> PyResult<()> {
    // Validate message hash length
    if message_hash.len() != 32 {
        return Err(PyValueError::new_err(format!(
            "message_hash must be exactly 32 bytes, got {}",
            message_hash.len()
        )));
    }

    // Deserialize public keys using SSZ
    let pub_keys: Result<Vec<LeanSigPubKey>, _> = pub_keys_bytes
        .iter()
        .map(|bytes| LeanSigPubKey::from_ssz_bytes(bytes))
        .collect();
    let pub_keys = pub_keys
        .map_err(|e| PyValueError::new_err(format!("Failed to deserialize public key (SSZ): {:?}", e)))?;

    // Convert message_hash to array
    let message_array: [u8; 32] = message_hash
        .try_into()
        .map_err(|_| PyValueError::new_err("Failed to convert message_hash to [u8; 32]"))?;

    // SSZ decode the aggregated signature
    let agg_sig = Devnet2XmssAggregateSignature::from_ssz_bytes(&agg_signature_ssz)
        .map_err(|e| PyValueError::new_err(format!("Failed to decode SSZ signature: {:?}", e)))?;

    // Call the verification function
    xmss_verify_aggregated_signatures(&pub_keys, &message_array, &agg_sig, epoch)
        .map_err(|e| PyValueError::new_err(format!("Verification failed: {:?}", e)))?;

    Ok(())
}

/// Convert bincode-encoded Devnet2XmssAggregateSignature to SSZ encoding.
///
/// Args:
///     bincode_bytes: Bincode-encoded aggregated signature
///
/// Returns:
///     SSZ-encoded aggregated signature as bytes
#[pyfunction]
fn ssz_encode_aggregate_signature(bincode_bytes: Vec<u8>) -> PyResult<Vec<u8>> {
    let agg_sig: Devnet2XmssAggregateSignature = bincode::deserialize(&bincode_bytes)
        .map_err(|e| PyValueError::new_err(format!("Failed to deserialize bincode: {}", e)))?;
    Ok(agg_sig.as_ssz_bytes())
}

/// Convert SSZ-encoded Devnet2XmssAggregateSignature to bincode encoding.
///
/// Args:
///     ssz_bytes: SSZ-encoded aggregated signature
///
/// Returns:
///     Bincode-encoded aggregated signature as bytes
#[pyfunction]
fn ssz_decode_aggregate_signature(ssz_bytes: Vec<u8>) -> PyResult<Vec<u8>> {
    let agg_sig = Devnet2XmssAggregateSignature::from_ssz_bytes(&ssz_bytes)
        .map_err(|e| PyValueError::new_err(format!("Failed to decode SSZ: {:?}", e)))?;
    bincode::serialize(&agg_sig)
        .map_err(|e| PyValueError::new_err(format!("Failed to serialize to bincode: {}", e)))
}

/// Get the mode this module was compiled with.
///
/// Returns:
///     "test" if compiled with test_config feature, "prod" otherwise
#[pyfunction]
fn get_mode() -> &'static str {
    MODE
}

/// Python module for lean-multisig XMSS aggregation (test mode)
#[cfg(feature = "test_config")]
#[pymodule]
fn lean_multisig_test(py_module: &Bound<'_, PyModule>) -> PyResult<()> {
    py_module.add("MODE", MODE)?;
    py_module.add_function(wrap_pyfunction!(setup_prover, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(setup_verifier, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(aggregate_signatures, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(verify_aggregated_signatures, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(ssz_encode_aggregate_signature, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(ssz_decode_aggregate_signature, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(get_mode, py_module)?)?;
    Ok(())
}

/// Python module for lean-multisig XMSS aggregation (prod mode)
#[cfg(not(feature = "test_config"))]
#[pymodule]
fn lean_multisig_prod(py_module: &Bound<'_, PyModule>) -> PyResult<()> {
    py_module.add("MODE", MODE)?;
    py_module.add_function(wrap_pyfunction!(setup_prover, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(setup_verifier, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(aggregate_signatures, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(verify_aggregated_signatures, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(ssz_encode_aggregate_signature, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(ssz_decode_aggregate_signature, py_module)?)?;
    py_module.add_function(wrap_pyfunction!(get_mode, py_module)?)?;
    Ok(())
}
