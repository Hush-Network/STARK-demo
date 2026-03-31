# Changelog

## 0.1.0 (2026-03-30)

- Payment circuit: 2-in-2-out credential-gated private transfer (~44,000 trace columns)
- Credential issuance circuit with Merkle-based issuer authorization (~13,000 trace columns)
- Time-window audit circuit: 16-slot aggregate proofs with configurable disclosure
- Poseidon2 AIR constraints (width-16, domain-separated, S-box as x^2 -> x^4 -> x^5)
- Depth-20 Merkle path verification (1M+ leaves)
- WASM bindings for in-browser proving
- Lifecycle binary: full issuance -> payment -> audit flow
- Benchmark binary with per-circuit timing
- 50 tests covering valid proofs, rejection cases, and Poseidon2 AIR correctness
