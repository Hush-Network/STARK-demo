# Architecture

## Modules

- `poseidon2` — M31 permutation, commitments, Merkle trees
- `poseidon2_air` — AIR constraints for hash verification (degree-2 decomposition)
- `circuit` — Payment circuit (2-in-2-out, credential-gated)
- `credential_issuance` — Issuer authorization proof
- `time_window` — Aggregate audit over a time range
- `prover_common` — Shared prover config, channel/hasher type aliases
- `types` — Witness structs, constants
- `wasm` — Browser bindings via wasm-bindgen

## Why hand-rolled Poseidon2?

Stwo doesn't ship Poseidon2-over-M31 with AIR constraints. We need the
hash inputs and outputs to be part of the STARK trace (algebraic
constraints, not just native evaluation), so we implement the full
permutation and constrain it in-circuit.

Constants from Plonky3's Grain LFSR, verified against their test vectors.
See `test_plonky3_vector` in `poseidon2.rs`.

## Trace layout

Column counts per circuit (at depth 20):

| Circuit              | Base | Range | Hash  | Merkle | Total  |
|----------------------|------|-------|-------|--------|--------|
| Payment              | 44   | 84    | 5,724 | 38,340 | 44,192 |
| Credential Issuance  | 6    | —     | 1,272 | 12,780 | 14,058 |
| Time-Window Audit    | 58   | 832   | 1,272 | 12,780 | 14,942 |

## Commitment backend

Poseidon252 for native builds (algebraic, recursion-ready).
Blake2s fallback for WASM where the starknet-crypto deps aren't available.

Config: `pow_bits=0` due to a known bug in Stwo's non-parallel
Poseidon252 grinding path (grind passes raw digest, verify expects
prefixed digest). PoW is DoS protection, doesn't affect proof soundness.

## Batch proving

Multiple transactions packed into one STARK proof. Each witness fills
different trace rows, padded to next power of 2. Amortizes FRI overhead.

TODO: diagram showing trace layout with multiple tx rows
