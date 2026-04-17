# Hush Network STARK Demo

[![CI](https://github.com/Hush-Network/stark-demo/actions/workflows/ci.yml/badge.svg)](https://github.com/Hush-Network/stark-demo/actions/workflows/ci.yml)

**[Try it live at demo.hushnetwork.io](https://demo.hushnetwork.io)**

This repository contains the STARK proving engine and browser demo behind the public HushPay proof demo. It proves the core payment, provenance attestation, and time-window audit circuits over Mersenne31 with no trusted setup. It does not prove a live network.

Built on [Stwo](https://github.com/starkware-libs/stwo) (FRI-based STARK prover, Mersenne31 field) with Poseidon2 as the in-circuit hash.

## Relationship to the browser demo

The live demo opens directly into the intended HushPay wallet experience: the sender sees amount, fee route, and total debit up front while the receiver gets the full payment amount.

This repository provides the proof engine underneath that experience: payment validity, provenance and non-revocation checks, audit proofs, and receipt verification. Wallet funding, boundary-issued provenance attestations, and live network submission remain represented in the demo.

## Status boundary

**Implemented**
- Payment, provenance attestation, and time-window audit circuits
- Browser WASM bindings used by the live demo
- Proof generation and proof verification for the payment circuit
- Test suite for circuit correctness

**Benchmarked**
- Native prove and verify timings for all three circuits
- Browser WASM timings for the payment circuit
- Trace layout and circuit size estimates

**Target-state**
- Recursive proof aggregation
- HotStuff-2 BFT consensus
- Threshold-encrypted mempool
- Structured note discovery
- Full revocation path

**Not implemented**
- Live validator network
- Protocol fee extraction
- Validator incentives
- Production wallet SDK

## What each circuit proves

| Circuit | What it proves |
|---------|----------------|
| Payment | The sender owns the input notes, the note's provenance attestation is valid, the lineage is not in the revocation accumulator, and the amounts balance. Sender, receiver, and amount stay hidden. |
| Provenance Attestation | The note carries a valid attestation signed by a screened boundary actor (exchange, bridge, issuer, PSP, merchant) at entry. |
| Time-Window Audit | This wallet transacted a specific total volume between two timestamps, without revealing individual transactions. |

## Circuits

Three STARK circuits on Stwo over Mersenne31, with full Poseidon2 AIR constraints (S-box decomposed as x^2 -> x^4 -> x^5 for degree-2 constraint compatibility).

**Payment circuit** (2-in-2-out private transfer with provenance + non-revocation check)
- Note consumption and creation with nullifier/commitment pairs
- Balance conservation enforced in-circuit
- Nullifier inequality check (prevents double-spend)
- Amount range checks (four 15-bit limbs per amount, radix 2^15, with carry-propagation conservation)
- Provenance attestation verification: boundary actor identity, attestation root, and Merkle inclusion checked inside the proof
- Non-revocation check: lineage marker proven absent from the revocation accumulator
- Three depth-20 Merkle path verifications per transaction (2 note paths + 1 attestation path)
- ~44,400 trace columns

**Provenance attestation circuit**
- Derives boundary actor identity from private key via Poseidon2
- Computes attestation commitment over (boundary actor, recipient note, attestation parameters, secret)
- Verifies boundary actor Merkle inclusion (depth-20)
- ~14,000 trace columns

**Time-window audit circuit** (16 transaction slots)
- Proves aggregate volume over a time window without revealing individual transactions
- Per-transaction: binary window flag, conditional contribution, 24-bit timestamp range checks
- Sum constraint: contributions equal claimed total
- Provenance attestation verification across the audited transactions
- Merkle inclusion for the boundary actor set

## What the payment circuit proves

Owner derivation, input/output note commitments, Merkle inclusion (2 note paths + 1 attestation path), nullifier derivation and uniqueness, balance conservation, provenance attestation validity (commitment + Merkle inclusion in the boundary actor set), and non-revocation (lineage marker absent from the revocation accumulator).

Public outputs bound via Fiat-Shamir: nullifiers, output commitments, lineage marker. Everything a validator would need to update ledger state and check the revocation accumulator.

## Performance

### Browser (WASM)

The live demo at demo.hushnetwork.io runs the full prover in the browser via WebAssembly. Measured in Chrome on AMD Ryzen 9:

| Circuit             | Prove (avg) |
|---------------------|-------------|
| Payment             |      ~334ms |

WASM uses Blake2s for the Merkle commitment backend (no SIMD Poseidon252 in browser). Payment amount does not affect proving time due to fixed-width trace layout.

### Native (single-threaded)

AMD Ryzen 9, release build. 10 iterations per circuit.

| Circuit             | Prove (avg) | Prove (min) | Prove (max) | Verify (avg) |
|---------------------|-------------|-------------|-------------|--------------|
| Payment             |     1010ms  |      960ms  |     1057ms  |       124ms  |
| Mode A Bundle       |     1089ms  |     1035ms  |     1123ms  |       123ms  |
| Mode B Bundle       |     1763ms  |     1666ms  |     1852ms  |       201ms  |
| Provenance Attest.  |      269ms  |      265ms  |      273ms  |   (combined) |
| Time-Window Audit   |      283ms  |      275ms  |      297ms  |   (combined) |

### Native (parallel, --features parallel)

Same hardware, multi-threaded via rayon. ~1.7x speedup on payment proving.

| Circuit             | Prove (avg) | Prove (min) | Prove (max) | Verify (avg) |
|---------------------|-------------|-------------|-------------|--------------|
| Payment             |      593ms  |      513ms  |      651ms  |       124ms  |
| Mode A Bundle       |      682ms  |      657ms  |      715ms  |       123ms  |
| Mode B Bundle       |     1087ms  |     1014ms  |     1167ms  |       202ms  |
| Provenance Attest.  |      145ms  |      142ms  |      148ms  |   (combined) |
| Time-Window Audit   |      138ms  |      133ms  |      154ms  |   (combined) |

Mode A = same-asset fee. Mode B = HUSH sidecar fee (payment + fee sidecar proofs). Accounting, epoch accrual, and payout generation run in sub-microsecond time and are not shown. April 13, 2026.

Recursive batching is a target-state optimization, not measured here. See [benchmarks/](benchmarks/) for the full breakdown.

Fixed-width amount encoding means payment size does not change the circuit shape within the supported amount range. That is why the browser demo can say something meaningful about large-value payment latency even before batching or recursion is implemented.

## Tests

113 tests covering:
- Valid proof generation and verification for all three circuits
- Balance conservation rejection (mismatched inputs/outputs)
- Nullifier reuse rejection (double-spend prevention)
- Revoked-lineage rejection (lineage marker present in revocation accumulator)
- Unauthorized boundary-actor rejection
- Invalid time window rejection
- Mismatched audit total rejection
- Poseidon2 AIR correctness (output verification, column count validation)
- Owner derivation consistency

## Cryptographic stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Proving system | STARK (FRI) | Transparent, no trusted setup, post-quantum |
| Prover | Stwo | Fastest production STARK prover, Rust, open source |
| Field | Mersenne31 (M31) | Native to Stwo, fast field arithmetic |
| In-circuit hash | Poseidon2 (width-16, Plonky3 constants) | STARK-optimized, efficient over M31 |
| Commitment backend | Poseidon252 (native) / Blake2s (WASM) | Native build stays algebraic; WASM uses a compatible fallback |

## Repository structure

```
src/
  circuit.rs              Payment circuit (2-in-2-out, with provenance + non-revocation check)
  credential_issuance.rs  Provenance attestation circuit (file name kept for compatibility)
  time_window.rs          Time-window audit circuit
  poseidon2.rs            Poseidon2 hash (M31, width-16, domain-separated)
  poseidon2_air.rs        Poseidon2 AIR constraints
  types.rs                Witness types
  wasm.rs                 WASM bindings (compiled to power the browser demo)
  prover_common.rs        Shared prover utilities
  bin/
    bench.rs              Benchmark suite
    lifecycle.rs          Full protocol flow demo
docs/
  architecture.md         Circuit architecture and proving notes
benchmarks/
  BENCHMARK_REPORT_2026-04-07.md  Latest benchmark run: WASM, native, and parallel results
  BENCHMARK_REPORT_2026-04-02.md  Previous baseline (pre-multi-limb)
```

## Development

```bash
cargo run --bin lifecycle --release                   # full protocol flow
cargo run --bin bench --release                       # benchmarks (single-threaded)
cargo run --bin bench --release --features parallel   # benchmarks (multi-threaded)
cargo test
cargo fmt -- --check
cargo clippy -- -D warnings
```

### WASM build

```bash
wasm-pack build --target web --out-dir web/pkg
cd web && npm ci && npx vite build
```

### CI

Push to `main` triggers fmt, clippy, test, audit, wasm-build, and web-build jobs. Cloudflare Pages deploys the web build automatically on push.

## Scope notes

This crate is the proving engine for Hush Network. It does not implement:

- Consensus
- Fee extraction
- Validator incentives
- Note discovery
- Consumer wallet flows beyond the browser demo

See [docs/architecture.md](docs/architecture.md) for circuit architecture notes and [benchmarks/BENCHMARK_REPORT_2026-04-07.md](benchmarks/BENCHMARK_REPORT_2026-04-07.md) for the full breakdown including WASM, single-threaded, and parallel native results.

## Prior art

- **Zcash** (Sapling/Orchard): note model, nullifier design, selective disclosure lineage
- **Penumbra**: shielded UTXO model, fund-level enforcement concepts
- **Aztec**: private execution model, compliance integration patterns
- **Stwo** (StarkWare): prover architecture, FRI over Mersenne31
- **Poseidon2** (Grassi et al.): STARK-friendly hash construction

## License

MIT
