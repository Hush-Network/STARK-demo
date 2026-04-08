# Changelog

## 0.2.0 (2026-04-07)

- Multi-limb amount encoding: four 15-bit limbs, radix 2^15, supporting full u64 amounts
- 7-input Poseidon2 note commitments
- Fee set to 50 protocol units ($0.0050) from genesis gas model; 1 protocol unit = $0.0001
- Limb-by-limb conservation with carry propagation in payment and fee sidecar circuits
- Binding hash updated for u64 amounts (lo/hi M31 pairs)
- WASM bindings accept f64 for u64 transport
- JS AMT_SCALE = 10,000; fee shown to 4 decimal places
- Fee included in receipt payload
- 110 tests passing (1 ignored)

## 0.1.0 (2026-03-30)

- Payment circuit: 2-in-2-out credential-gated private transfer
- Credential issuance circuit with Merkle-based issuer authorization
- Time-window audit circuit (16-slot aggregate proofs)
- Batch proving for multi-transaction STARK proofs
- Lifecycle binary: full issuance → payment → audit flow
- WASM bindings for in-browser proving
- Poseidon252 commitment backend (algebraic, recursion-ready)
- Depth-20 Merkle trees (1M+ leaves)
- 44 tests including Plonky3 cross-validation (now 110 as of 0.2.0)
