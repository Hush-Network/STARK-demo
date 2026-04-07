# Hush Network STARK Demo

[![CI](https://github.com/Hush-Network/stark-demo/actions/workflows/ci.yml/badge.svg)](https://github.com/Hush-Network/stark-demo/actions/workflows/ci.yml)

This repository contains the STARK circuit implementation, fee model runtime, benchmarks, and browser demo that power [demo.hushnetwork.io](https://demo.hushnetwork.io). The `web/` directory contains the demo frontend deployed to that domain.

Hush Network is building credential-gated private stablecoin settlement. This repository contains the STARK circuits that enforce it: real zero-knowledge proofs over Mersenne31, no trusted setup, running live in the browser. Every transaction proves ownership, balance conservation, and credential validity without revealing sender, receiver, or amount.

Built on [Stwo](https://github.com/starkware-libs/stwo) (FRI-based STARK prover, Mersenne31 field) with Poseidon2 as the in-circuit hash.

## What each circuit proves

| Circuit | What it proves |
|---------|----------------|
| Payment | The sender owns the input notes, the credential is valid, and the amounts balance. Sender, receiver, and amount stay hidden. |
| Credential Issuance | This wallet was authorized by a verified issuer to participate in the network. |
| Time-Window Audit | This wallet transacted a specific total volume between two timestamps, without revealing individual transactions. |

**[Try it live](https://demo.hushnetwork.io)** — browser-based STARK proving, no backend required.

## Circuits

Three STARK circuits on Stwo over Mersenne31, with full Poseidon2 AIR constraints (S-box decomposed as x^2 -> x^4 -> x^5 for degree-2 constraint compatibility).

**Payment circuit** (2-in-2-out credential-gated transfers)
- Note consumption and creation with nullifier/commitment pairs
- Balance conservation enforced in-circuit (supports both HUSH-only and dual-payment fee paths)
- Nullifier inequality check (prevents double-spend)
- Amount range checks (21-bit decomposition per amount, 5 amounts including fee field)
- Credential verification: issuer, expiry, and Merkle inclusion checked inside the proof
- Three depth-20 Merkle path verifications per transaction (2 note paths + 1 credential path)
- ~45,000 trace columns

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

## Fee models

Two fee models are implemented and under evaluation. Both are functional and the architecture supports toggling between them at the protocol level.

**HUSH-only gas model:** All transaction fees are denominated in HUSH, the native protocol token. This is the industry-default approach for L1 networks.

**Dual-payment fee model:** Transaction fees can be paid in the same stablecoin being transacted, or optionally in HUSH. The sender pays amount plus fee, and the receiver gets the exact intended amount. Fee accounting, epoch accrual, and validator payout are handled by `dual_fee_runtime.rs` and `accounting.rs`.

The decision on which model to use at network launch will be made after further evaluation and investor input. Both paths are actively maintained.

## What the payment circuit proves

Owner derivation, input/output note commitments, Merkle inclusion (2 note paths + 1 credential path), nullifier derivation and uniqueness, balance conservation, credential validity (commitment + expiry range check + Merkle inclusion), and credential nullifier binding.

Public outputs bound via Fiat-Shamir: nullifiers, output commitments, credential nullifier. Everything a validator needs to update ledger state.

## Performance

Measured on AMD Ryzen 9 / release build (April 2, 2026). 10 iterations per circuit. Single-threaded, no batching, no recursion.

| Circuit             | Prove (avg) | Prove (min) | Prove (max) | Verify (avg) |
|---------------------|-------------|-------------|-------------|--------------|
| Payment             |      847ms  |      831ms  |      872ms  |       113ms  |
| Credential Issuance |      277ms  |      268ms  |      289ms  |   (combined) |
| Time-Window Audit   |      286ms  |      274ms  |      301ms  |   (combined) |

WASM (browser): ~1.2s prove, ~20ms verify.

These improve significantly with recursive batching and multi-threading. See [benchmarks/](benchmarks/) for full details.

## Tests

50+ tests covering:
- Valid proof generation and verification for all three circuits
- Balance conservation rejection (mismatched inputs/outputs)
- Nullifier reuse rejection (double-spend prevention)
- Expired credential rejection
- Unauthorized issuer rejection
- Invalid time window rejection
- Mismatched audit total rejection
- Poseidon2 AIR correctness (output verification, column count validation)
- Owner derivation consistency
- Fee model accounting (both HUSH-only and dual-payment paths)
- Payment transaction validation and fixtures

## Cryptographic stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Proving system | STARK (FRI) | Transparent, no trusted setup, post-quantum |
| Prover | Stwo | Fastest production STARK prover, Rust, open source |
| Field | Mersenne31 (M31) | Native to Stwo, fast field arithmetic |
| In-circuit hash | Poseidon2 (width-16, Plonky3 constants) | STARK-optimized, efficient over M31 |
| Commitment backend | Poseidon252 (native) / Blake2s (WASM) | |
| Domain separation | 9 domains (owner, nullifier, note, credential, Merkle, tx binding, sender binding) | Prevents cross-context hash collisions |

## Repository structure

```
src/
  circuit.rs              Payment circuit (2-in-2-out, fee-aware)
  credential_issuance.rs  Credential issuance circuit
  time_window.rs          Time-window audit circuit
  poseidon2.rs            Poseidon2 hash (M31, width-16, 9 domains)
  poseidon2_air.rs        Poseidon2 AIR constraints
  types.rs                Witness types (payment, credential, fee)
  wasm.rs                 WASM bindings for browser demo
  prover_common.rs        Shared prover utilities
  dual_fee_runtime.rs     Dual-payment fee model runtime
  accounting.rs           Fee accounting, epoch accrual, validator payout
  fee_sidecar.rs          HUSH fee sidecar proof construction
  payment_tx.rs           Payment transaction builder and binding
  payment_validation.rs   Transaction validation logic
  payment_fixtures.rs     Test fixtures for payment flows
  measurement.rs          Timing utilities
  bin/
    bench.rs              Benchmark suite
    lifecycle.rs          Full protocol flow demo
web/
  index.html              Demo frontend (deployed to demo.hushnetwork.io)
  verify.html             Proof verification page
  src/main.js             Demo wallet UI
  src/verify.js           Verification UI
  vite.config.js          Vite build config
docs/
  architecture.md         Architecture decisions, system design, scaling path
benchmarks/
  BENCHMARK_REPORT_2026-04-02.md   Measured performance numbers
```

## Development

```bash
scripts/test.sh     # run tests
scripts/bench.sh    # benchmarks
scripts/fmt.sh      # format
cargo clippy -- -D warnings
```

```bash
cargo run --bin lifecycle --release   # full protocol flow demo
cargo run --bin bench --release       # performance benchmarks
```

### WASM build

```bash
wasm-pack build --target web --out-dir web/pkg
```

### Web frontend

```bash
cd web && npm install && npx vite build
```

## Status

This is the proving engine for Hush Network. Circuits are functional and tested. Both fee models are implemented at the circuit and runtime level. Not yet implemented:

- **Recursive proof aggregation.** One STARK proof per block is the next milestone. Once in place, a single proof covers all transactions in that block.
- **Consensus.** HotStuff-2 BFT is designed, not built.
- **Note discovery protocol.** FMD-based recipient detection is specified, not implemented.
- **Mempool encryption.** Threshold encryption at block time is specified, not implemented.
- **Production wallet SDK.** Rust + WASM wallet SDK is next after the node.

See [docs/architecture.md](docs/architecture.md) for the full system design, scaling path, and open problems.

## Prior art

- **Zcash** (Sapling/Orchard): note model, nullifier design, selective disclosure lineage
- **Penumbra**: shielded UTXO model, credential-gated participation concepts
- **Aztec**: private execution model, compliance integration patterns
- **Stwo** (StarkWare): prover architecture, FRI over Mersenne31
- **Poseidon2** (Grassi et al.): STARK-friendly hash construction

## License

MIT
