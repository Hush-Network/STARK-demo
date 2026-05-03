# Circuit Architecture Notes

This note covers the proving engine used by the Hush browser demo. It does not describe a complete live network or the full fee-routing design shown in the browser wallet experience.

## Modules

- `poseidon2` - M31 permutation, commitments, Merkle trees
- `poseidon2_air` - AIR constraints for hash verification (degree-2 decomposition)
- `circuit` - Payment circuit (2-in-2-out private transfer with provenance + non-revocation check)
- `provenance_attestation` - Provenance attestation proof
- `time_window` - Aggregate audit circuit over a time range, surfaced as audit keys in the demo
- `payment_tx` / `payment_validation` / `payment_fixtures` - canonical tx encoding, bundle validation, and shared fixtures
- `fee_sidecar` / `dual_fee_runtime` - HUSH fee sidecar circuit and dual-fee runtime
- `accounting` - Block accounting and validator payout primitives
- `prover_common` - Shared prover config, channel, and hasher type aliases
- `types` - Witness structs and shared constants
- `wasm` - Browser bindings via wasm-bindgen

## Why hand-rolled Poseidon2

Stwo does not ship Poseidon2-over-M31 with AIR constraints. Hush needs hash inputs and outputs inside the STARK trace itself, not just native evaluation, so the permutation and its AIR are implemented directly in this crate.

Constants come from Plonky3's Grain LFSR and are verified against the upstream test vectors. See `test_plonky3_vector` in `poseidon2.rs`.

## Trace layout

Column counts per circuit at Merkle depth 20:

| Circuit | Base | Range | Hash | Merkle | Total |
|---------|------|-------|------|--------|-------|
| Payment | 66 | 300 | 5,724 | 38,340 | 44,430 |
| Provenance Attestation | 6 | - | 1,272 | 12,780 | 14,058 |
| Time-Window Audit | 58 | 832 | 1,272 | 12,780 | 14,942 |

Because the payment circuit uses fixed-width amount encoding, the shape of the trace does not change when the payment value gets large. That matters for Hush because the browser demo can already provide a real early signal on large-value payment latency before batching or recursion is added.

## Commitment backend

Native builds use Poseidon252 because it stays algebraic and recursion-friendly.

WASM uses a Blake2s fallback where the `starknet-crypto` dependencies are not available. This keeps the browser demo runnable without changing the proof flow shown to the user.

`pow_bits=0` is set because of a known bug in Stwo's non-parallel Poseidon252 grinding path. That affects DoS hardening, not proof soundness.

## Batch proving

Planned direction: pack multiple transactions into one STARK proof by assigning each witness to different trace rows and padding to the next power of two. The goal is to amortize FRI overhead across the batch.

This is not implemented in this crate today.
