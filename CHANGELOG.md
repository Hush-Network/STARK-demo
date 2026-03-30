# Changelog

## 0.1.0 (2026-03-30)

- Payment circuit: 2-in-2-out credential-gated private transfer
- Credential issuance circuit with Merkle-based issuer authorization
- Time-window audit circuit (16-slot aggregate proofs)
- Batch proving for multi-transaction STARK proofs
- Lifecycle binary: full issuance → payment → audit flow
- WASM bindings for in-browser proving
- Poseidon252 commitment backend (algebraic, recursion-ready)
- Depth-20 Merkle trees (1M+ leaves)
- 44 tests including Plonky3 cross-validation
