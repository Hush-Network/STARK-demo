# Hush Network STARK Demo

[![CI](https://github.com/Hush-Network/stark-demo/actions/workflows/ci.yml/badge.svg)](https://github.com/Hush-Network/stark-demo/actions/workflows/ci.yml)

**[Try it live at demo.hushnetwork.io](https://demo.hushnetwork.io)**

This repository contains the STARK proving engine behind the Hush browser demo. It proves the core payment, credential issuance, and time-window audit circuits over Mersenne31 with no trusted setup. It does not prove a live network.

Built on [Stwo](https://github.com/starkware-libs/stwo) (FRI-based STARK prover, Mersenne31 field) with Poseidon2 as the in-circuit hash.

## Relationship to the product demo

The live demo shows the intended Hush wallet experience under the fee model being designed: the sender sees amount, fee, and total in the payment asset while the receiver gets the full amount.

This repository does not implement that fee-routing model. It provides the proof engine underneath the demo: payment validity, credential checks, audit proofs, and receipt verification.

## Status boundary

**Implemented**
- Payment, credential issuance, and time-window audit circuits
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
| Payment | The sender owns the input notes, the credential is valid, and the amounts balance. Sender, receiver, and amount stay hidden. |
| Credential Issuance | This wallet was authorized by a verified issuer to participate in the network. |
| Time-Window Audit | This wallet transacted a specific total volume between two timestamps, without revealing individual transactions. |

## Circuits

Three STARK circuits on Stwo over Mersenne31, with full Poseidon2 AIR constraints (S-box decomposed as x^2 -> x^4 -> x^5 for degree-2 constraint compatibility).

**Payment circuit** (2-in-2-out credential-gated transfers)
- Note consumption and creation with nullifier/commitment pairs
- Balance conservation enforced in-circuit
- Nullifier inequality check (prevents double-spend)
- Amount range checks (four 15-bit limbs per amount, radix 2^15, with carry-propagation conservation)
- Credential verification: issuer, expiry, and Merkle inclusion checked inside the proof
- Three depth-20 Merkle path verifications per transaction (2 note paths + 1 credential path)
- ~44,400 trace columns

**Credential issuance circuit**
- Derives issuer identity from private key via Poseidon2
- Computes credential commitment over (issuer, subject, expiry, secret)
- Verifies issuer Merkle inclusion (depth-20)
- ~14,000 trace columns

**Time-window audit circuit** (16 transaction slots)
- Proves aggregate volume over a time window without revealing individual transactions
- Per-transaction: binary window flag, conditional contribution, 24-bit timestamp range checks
- Sum constraint: contributions equal claimed total
- Credential verification with expiry range check
- Merkle inclusion for credential set

## What the payment circuit proves

Owner derivation, input/output note commitments, Merkle inclusion (2 note paths + 1 credential path), nullifier derivation and uniqueness, balance conservation, credential validity (commitment + expiry range check + Merkle inclusion), and credential nullifier binding.

Public outputs bound via Fiat-Shamir: nullifiers, output commitments, credential nullifier. Everything a validator would need to update ledger state.

## Performance

Measured on AMD Ryzen 9 / release build. 10 iterations per circuit. Single-threaded, no batching, no recursion.

| Circuit             | Prove (avg) | Prove (min) | Prove (max) | Verify (avg) |
|---------------------|-------------|-------------|-------------|--------------|
| Payment             |      970ms  |      907ms  |     1034ms  |       119ms  |
| Mode A Bundle       |     1058ms  |     1003ms  |     1122ms  |       119ms  |
| Mode B Bundle       |     1661ms  |     1627ms  |     1702ms  |       191ms  |
| Credential Issuance |      285ms  |      269ms  |      322ms  |   (combined) |
| Time-Window Audit   |      291ms  |      281ms  |      313ms  |   (combined) |

Native: AMD Ryzen 9, release build, April 7 2026. Mode A = same-asset fee. Mode B = HUSH sidecar fee (payment + fee sidecar proofs). Accounting, epoch accrual, and payout generation run in sub-microsecond time and are not shown.

Recursive batching and multi-threading are target-state optimizations, not measured here. See [benchmarks/](benchmarks/) for the full breakdown.

## Tests

110 tests covering:
- Valid proof generation and verification for all three circuits
- Balance conservation rejection (mismatched inputs/outputs)
- Nullifier reuse rejection (double-spend prevention)
- Expired credential rejection
- Unauthorized issuer rejection
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
  circuit.rs              Payment circuit (2-in-2-out)
  credential_issuance.rs  Credential issuance circuit
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
  BENCHMARK_REPORT_2026-04-02.md  Latest benchmark run with measured, inferred, and target sections
```

## Development

```bash
scripts/test.sh     # run tests (110 tests)
scripts/bench.sh    # benchmarks
scripts/fmt.sh      # format
cargo clippy -- -D warnings
```

```bash
cargo run --bin lifecycle --release   # full protocol flow demo
cargo run --bin bench --release       # performance benchmarks
```

## Scope notes

This crate is the proving engine for Hush Network. It does not implement:

- Consensus
- Fee extraction
- Validator incentives
- Note discovery
- Consumer wallet flows beyond the browser demo

See [docs/architecture.md](docs/architecture.md) for circuit architecture notes and [benchmarks/BENCHMARK_REPORT_2026-04-02.md](benchmarks/BENCHMARK_REPORT_2026-04-02.md) for the measured versus target breakdown.

## Prior art

- **Zcash** (Sapling/Orchard): note model, nullifier design, selective disclosure lineage
- **Penumbra**: shielded UTXO model, private eligibility-gating concepts
- **Aztec**: private execution model, compliance integration patterns
- **Stwo** (StarkWare): prover architecture, FRI over Mersenne31
- **Poseidon2** (Grassi et al.): STARK-friendly hash construction

## License

MIT
