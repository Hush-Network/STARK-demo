# Benchmark Report - April 7, 2026

**Hardware:** AMD Ryzen 9, release build
**Prover:** Stwo (FRI-based STARK, Mersenne31)
**Iterations:** 10 per circuit
**Mode:** Single-threaded, no batching, no recursion
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

## Measured

Results from `cargo run --bin bench --release`:

| Circuit | Prove (avg) | Prove (min) | Prove (max) | Verify (avg) |
|---|---|---|---|---|
| Payment (2-in-2-out) | 970ms | 907ms | 1034ms | 119ms |
| Mode A Bundle (same-asset fee) | 1058ms | 1003ms | 1122ms | 119ms |
| Mode B Bundle (HUSH sidecar) | 1661ms | 1627ms | 1702ms | 191ms |
| Credential Issuance | 285ms | 269ms | 322ms | (combined) |
| Time-Window Audit (16 slots) | 291ms | 281ms | 313ms | (combined) |
| Accounting Accept | 0.49us | 0.10us | 2.40us | (state) |
| Epoch Accrual | 2.18us | 1.40us | 7.30us | (state) |
| Payout Generation | 0.16us | 0.00us | 0.90us | (state) |

Mode B / Mode A bundle prove ratio: 1.57x | verify ratio: 1.61x

Payment prove increased ~14% from the April 2 baseline (847ms to 970ms). This is consistent with the additional 238 trace columns (range check bits for multi-limb amounts and carry decomposition). The column count increase was 0.54%, but the 14% prove time increase suggests the range check columns have a disproportionate cost relative to their count, likely due to the FRI commitment overhead per column.

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

- Browser WASM prove/verify times (WASM build refreshed, not re-benchmarked in browser)
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

Payment circuit prove time at 970ms native single-threaded is a credible baseline for a full STARK proof over a ~44,400-column trace with three depth-20 Merkle paths and nine Poseidon2 hash traces. The 14% increase from the previous 847ms baseline reflects the cost of multi-limb amount encoding (300 range check columns, 6 carry bit columns).

Mode A bundles (same-asset fee) add minimal overhead beyond the payment proof since accounting and epoch operations run in sub-microsecond time. Mode B bundles (HUSH sidecar) require a second proof for the fee sidecar circuit (~30,000 columns), producing the 1.57x prove ratio.

The path to production throughput runs through recursive proof aggregation: one STARK proof per block covering all transactions. That is a design target, not a measured result.
