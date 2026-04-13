# Benchmark Report - April 13, 2026

This report keeps the original filename for continuity. Measurements refreshed April 13, 2026.

**Hardware:** AMD Ryzen 9, release build
**Prover:** Stwo (FRI-based STARK, Mersenne31)
**Iterations:** 10 per circuit
**Field:** Mersenne31 (M31)
**Merkle depth:** 20

---

## Changes since April 2 report

- Amount encoding changed from single M31 elements to four 15-bit limbs (radix 2^15)
- Note commitments changed from 4-input to 7-input Poseidon2 (asset, a0, a1, a2, a3, owner, randomness)
- Payment circuit trace: 44,192 columns to 44,430 columns (+238, +0.54%)
- Fee set to 50 protocol units ($0.0050) from genesis gas model
- All amount types changed from u32 to u64

---

## Measured: Browser (WASM)

The live demo at demo.hushnetwork.io runs the full prover in Chrome via WebAssembly. WASM uses Blake2s for the Merkle commitment backend (no SIMD Poseidon252 in browser).

| Circuit | Prove (avg) |
|---|---|
| Payment (2-in-2-out) | ~334ms |

Payment amount does not affect proving time due to fixed-width trace layout.

---

## Measured: Native (single-threaded)

Results from `cargo run --bin bench --release`:

| Circuit | Prove (avg) | Prove (min) | Prove (max) | Verify (avg) |
|---|---|---|---|---|
| Payment (2-in-2-out) | 1010ms | 960ms | 1057ms | 124ms |
| Mode A Bundle (same-asset fee) | 1089ms | 1035ms | 1123ms | 123ms |
| Mode B Bundle (HUSH sidecar) | 1763ms | 1666ms | 1852ms | 201ms |
| Credential Issuance | 269ms | 265ms | 273ms | (combined) |
| Time-Window Audit (16 slots) | 283ms | 275ms | 297ms | (combined) |
| Accounting Accept | 2.63us | 0.20us | 23.70us | (state) |
| Epoch Accrual | 3.26us | 1.30us | 19.60us | (state) |
| Payout Generation | 0.25us | 0.00us | 1.50us | (state) |

Mode B / Mode A bundle prove ratio: 1.62x | verify ratio: 1.63x

---

## Measured: Native (parallel)

Results from `cargo run --bin bench --release --features parallel` (rayon multi-threading):

| Circuit | Prove (avg) | Prove (min) | Prove (max) | Verify (avg) |
|---|---|---|---|---|
| Payment (2-in-2-out) | 593ms | 513ms | 651ms | 124ms |
| Mode A Bundle (same-asset fee) | 682ms | 657ms | 715ms | 123ms |
| Mode B Bundle (HUSH sidecar) | 1087ms | 1014ms | 1167ms | 202ms |
| Credential Issuance | 145ms | 142ms | 148ms | (combined) |
| Time-Window Audit (16 slots) | 138ms | 133ms | 154ms | (combined) |
| Accounting Accept | 0.79us | 0.20us | 4.50us | (state) |
| Epoch Accrual | 3.68us | 2.40us | 13.40us | (state) |
| Payout Generation | 0.22us | 0.00us | 1.00us | (state) |

Mode B / Mode A bundle prove ratio: 1.59x | verify ratio: 1.65x

Parallel speedup: ~1.7x on payment proving (1010ms to 593ms). The `parallel` feature enables rayon via stwo's internal parallelization. WASM builds cannot use this feature (no thread support).

---

Payment prove increased ~25% from the April 2 baseline (847ms to 1010ms single-threaded). This is consistent with the additional 238 trace columns (range check bits for multi-limb amounts and carry decomposition) plus the current bundle and accounting path.

---

## Actual circuit trace column counts

From code analysis (verified against constants in circuit.rs, fee_sidecar.rs):

| Circuit | Base | Range | Hash | Merkle | Total |
|---|---|---|---|---|---|
| Payment (2-in-2-out) | 66 | 300 | 5,724 | 38,340 | 44,430 |
| Fee Sidecar (HUSH) | 34 | 240 | 4,452 | 25,560 | 30,286 |
| Credential Issuance | 6 | - | 1,272 | 12,780 | 14,058 |
| Time-Window Audit (16 slots) | 58 | 832 | 1,272 | 12,780 | 14,942 |

Payment circuit Base breakdown: 42 witness + 18 aux (null_diff_inv, expiry_diff, 16 expiry bits) + 6 carry bits (3 carries x 2 bits each).
Payment circuit Range breakdown: 5 amounts x 4 limbs x 15 bits = 300.

---

## Inferred

| Metric | Value | Derivation |
|---|---|---|
| Per-note gas (trace columns) | ~7,600 | (44,430 - 14,058) / 4 notes = 7,593 |
| Base overhead per transaction | ~14,000 | Credential issuance circuit trace count |
| Amortized verify cost (recursive, projected) | ~1ms per tx | Estimated at a 100-tx batch; requires recursion, not implemented |

---

## Target / Design Goal

Not yet measurable. Requires components that are not built yet.

| Metric | Target | Requires |
|---|---|---|
| TPS (baseline, single-threaded verify) | 100+ | Consensus + basic node |
| TPS (with recursive aggregation) | 1,000+ | Recursive STARK (one proof per block) |
| TPS (post-mainnet, sharded) | 10,000+ | Sharded state, parallel proving, L1 optimizations |
| Block finality | ~2s | HotStuff-2 BFT (designed, not built) |
| Est. tx fee (standard payment) | $0.0050 | 50 protocol units at 1 unit = $0.0001 |
| Recursive verify latency | ~1ms amortized | Recursive aggregation |

---

## Not measured / not implemented

- Consensus throughput
- Block finality
- Mixed-asset fee routing economics
- Validator compensation distribution
- Full revocation path
- Recursive aggregation
- Note discovery
- Actual stablecoin integration (no tokens, testnet, or bridges exist)

---

## Context

Payment circuit prove time at 1010ms native single-threaded (593ms parallel) is a credible baseline for a full STARK proof over a ~44,400-column trace with three depth-20 Merkle paths and nine Poseidon2 hash traces. Browser WASM proving at ~334ms benefits from Blake2s as the commitment backend, which avoids the Poseidon252 overhead used in native builds.

Mode A bundles (same-asset fee) add moderate overhead beyond the payment proof while accounting and epoch operations still run in microseconds. Mode B bundles (HUSH sidecar) require a second proof for the fee sidecar circuit (~30,000 columns), producing the ~1.6x prove ratio.

The path to production throughput runs through recursive proof aggregation: one STARK proof per block covering all transactions. That is a design target, not a measured result.
