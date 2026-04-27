# Benchmark Report - April 17, 2026

Measurements refreshed April 17, 2026 after the public demo wording and compliance audit.

**Hardware:** AMD Ryzen 9, release build  
**Prover:** Stwo (FRI-based STARK, Mersenne31)  
**Iterations:** 10 per circuit  
**Field:** Mersenne31 (M31)  
**Merkle depth:** 20

---

## Scope of this refresh

- Re-ran the native single-threaded benchmark suite
- Re-ran the native parallel benchmark suite
- Rebuilt the browser demo assets so generated output matches the updated source wording
- Kept the latest separately measured browser WASM timing note for continuity

This refresh did not change circuit shape, proving logic, or benchmark methodology.

---

## Measured: Browser (WASM)

The live demo at demo.hushnetwork.io runs the full prover in Chrome via WebAssembly. The latest separately measured browser average remains:

| Circuit | Prove (avg) |
|---|---|
| Payment (2-in-2-out) | ~334ms |

WASM uses Blake2s for the Merkle commitment backend (no SIMD Poseidon252 in browser). Payment amount does not affect proving time due to fixed-width trace layout.

---

## Measured: Native (single-threaded)

Results from `cargo run --bin bench --release`:

| Circuit | Prove (avg) | Prove (min) | Prove (max) | Verify (avg) |
|---|---|---|---|---|
| Payment (2-in-2-out) | 989.56ms | 594.92ms | 1775.58ms | 209.24ms |
| Mode A Bundle (same-asset fee) | 906.53ms | 761.08ms | 1042.06ms | 203.77ms |
| Mode B Bundle (HUSH sidecar) | 1168.48ms | 1054.78ms | 1289.20ms | 247.88ms |
| Credential Issuance | 166.44ms | 154.22ms | 184.60ms | (combined) |
| Time-Window Audit (16 slots) | 159.21ms | 143.67ms | 179.73ms | (combined) |
| Accounting Accept | 0.61us | 0.20us | 3.40us | (state) |
| Epoch Accrual | 2.93us | 2.10us | 9.70us | (state) |
| Payout Generation | 0.18us | 0.00us | 1.00us | (state) |

Mode B / Mode A bundle prove ratio: 1.29x | verify ratio: 1.22x

---

## Measured: Native (parallel)

Results from `cargo run --bin bench --release --features parallel` (rayon multi-threading):

| Circuit | Prove (avg) | Prove (min) | Prove (max) | Verify (avg) |
|---|---|---|---|---|
| Payment (2-in-2-out) | 639.13ms | 561.35ms | 717.44ms | 127.75ms |
| Mode A Bundle (same-asset fee) | 709.91ms | 616.79ms | 779.50ms | 128.42ms |
| Mode B Bundle (HUSH sidecar) | 1066.88ms | 1001.91ms | 1109.74ms | 206.40ms |
| Credential Issuance | 153.36ms | 142.47ms | 172.41ms | (combined) |
| Time-Window Audit (16 slots) | 140.57ms | 130.47ms | 158.11ms | (combined) |
| Accounting Accept | 1.52us | 0.20us | 11.30us | (state) |
| Epoch Accrual | 3.31us | 1.30us | 20.60us | (state) |
| Payout Generation | 0.18us | 0.00us | 1.00us | (state) |

Mode B / Mode A bundle prove ratio: 1.50x | verify ratio: 1.61x

Parallel speedup on payment proving: ~1.55x (989.56ms to 639.13ms). WASM builds cannot use this feature.

---

## Actual circuit trace column counts

From code analysis (verified against constants in `circuit.rs` and `fee_sidecar.rs`):

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

## Final-Form Design Goal

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

Payment circuit prove time at 989.56ms native single-threaded (639.13ms parallel) remains a credible baseline for a full STARK proof over a ~44,400-column trace with three depth-20 Merkle paths and nine Poseidon2 hash traces. Browser WASM proving at ~334ms still benefits from Blake2s as the commitment backend, which avoids the Poseidon252 overhead used in native builds.

Mode A bundles (same-asset fee) add moderate overhead beyond the payment proof while accounting and epoch operations still run in microseconds. Mode B bundles (HUSH sidecar) require a second proof for the fee sidecar circuit (~30,000 columns), producing the ~1.3x single-threaded prove ratio and ~1.5x parallel prove ratio.

The path to production throughput runs through recursive proof aggregation: one STARK proof per block covering all transactions. That is part of the final-form architecture, not a measured result.
