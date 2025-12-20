"""Python bindings for lean-multisig XMSS aggregation.

This package is a thin wrapper around the compiled PyO3 extension module
`lean_multisig_py.lean_multisig` (built by maturin).
"""

from lean_multisig_py.lean_multisig import (
    setup_prover,
    setup_verifier,
    aggregate_signatures,
    verify_aggregated_signatures,
)

__all__ = [
    "setup_prover",
    "setup_verifier",
    "aggregate_signatures",
    "verify_aggregated_signatures",
]


