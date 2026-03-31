# Hush Network STARK Demo

[![CI](https://github.com/Hush-Network/stark-demo/actions/workflows/ci.yml/badge.svg)](https://github.com/Hush-Network/stark-demo/actions/workflows/ci.yml)

STARK circuit implementation for [Hush Network](https://hushnetwork.io), a private-by-default stablecoin settlement L1 with built-in compliance. Built on [Stwo](https://github.com/starkware-libs/stwo) (FRI-based STARK prover, Mersenne31 field) with Poseidon2 as the in-circuit hash.

**Live demo:** [demo.hushnetwork.io](https://demo.hushnetwork.io) (generates real STARK proofs in your browser)

## Circuits

Three STARK circuits on Stwo over Mersenne31, with full Poseidon2 AIR constraints (S-box decomposed as x^2 -> x^4 -> x^5 for degree-2 constraint compatibility).

**Payment circuit** (2-in-2-out credential-gated transfers)
- Note consumption and creation with nullifier/commitment pairs
- Balance conservation enforced in-circuit
- Nullifier inequality check (prevents double-spend)
- Amount range checks (21-bit decomposition per amount)
- Credential verification: issuer, expiry, and Merkle inclusion checked inside the proof
- Three depth-20 Merkle path verifications per transaction (2 note paths + 1 credential path)
- ~44,000 trace columns

**Credential issuance circuit**
- Derives issuer identity from private key via Poseidon2
- Computes credential commitment over (issuer, subject, expiry, secret)
- Verifies issuer Merkle inclusion (depth-20)
- ~13,000 trace columns

**Time-window audit circuit** (16 transaction slots)
- Proves aggregate volume over a time window without revealing individual transactions
- Per-transaction: binary window flag, conditional contribution, 24-bit timestamp range checks
- Sum constraint: contributions equal claimed total
- Credential verification with expiry range check
- Merkle inclusion for credential set

## What the payment circuit proves

Owner derivation, input/output note commitments, Merkle inclusion (2 note paths + 1 credential path), nullifier derivation and uniqueness, balance conservation, credential validity (commitment + expiry range check + Merkle inclusion), and credential nullifier binding.

Public outputs bound via Fiat-Shamir: nullifiers, output commitments, credential nullifier. Everything a validator needs to update ledger state.

## Performance

Measured on AMD Ryzen 9 / release build. 10 iterations per circuit. Single-threaded, no batching, no recursion.

| Circuit             | Prove (avg) | Prove (min) | Prove (max) | Verify (avg) |
|---------------------|-------------|-------------|-------------|--------------|
| Payment             |      906ms  |      850ms  |      974ms  |       115ms  |
| Credential Issuance |      282ms  |      273ms  |      294ms  |   (combined) |
| Time-Window Audit   |      297ms  |      282ms  |      316ms  |   (combined) |

These improve significantly with recursive batching and multi-threading. See [benchmarks/](benchmarks/) for full details.

## Tests

50 tests covering:
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
| Commitment backend | Poseidon252 (native) / Blake2s (WASM) | |

## Repository structure

```
src/
  circuit.rs              Payment circuit (2-in-2-out)
  credential_issuance.rs  Credential issuance circuit
  time_window.rs          Time-window audit circuit
  poseidon2.rs            Poseidon2 hash (M31, width-16, domain-separated)
  poseidon2_air.rs        Poseidon2 AIR constraints
  types.rs                Witness types
  wasm.rs                 Browser WASM bindings
  prover_common.rs        Shared prover utilities
  bin/
    bench.rs              Benchmark suite
    lifecycle.rs          Full protocol flow demo
docs/
  architecture.md         Architecture decisions, system design, scaling path
benchmarks/
  2026-03-31-stark-circuits.md   Measured performance numbers
```

## Development

```bash
scripts/test.sh     # run tests (50 tests)
scripts/bench.sh    # benchmarks
scripts/fmt.sh      # format
cargo clippy -- -D warnings
```

## Binaries

```bash
cargo run --bin lifecycle --release   # full protocol flow demo
cargo run --bin bench --release       # performance benchmarks
```

## Status

This is the cryptographic core for Hush Network. Circuits are functional and tested. Not yet implemented:

- Recursive proof aggregation (batching)
- Consensus (HotStuff-2 BFT is designed, not built)
- Note discovery protocol
- Mempool encryption
- Production wallet SDK

See [docs/architecture.md](docs/architecture.md) for the full system design, scaling path, and open problems.

## Prior art

- **Zcash** (Sapling/Orchard): note model, nullifier design, selective disclosure lineage
- **Penumbra**: shielded UTXO model, credential-gated participation concepts
- **Aztec**: private execution model, compliance integration patterns
- **Stwo** (StarkWare): prover architecture, FRI over Mersenne31
- **Poseidon2** (Grassi et al.): STARK-friendly hash construction

## License

MIT
