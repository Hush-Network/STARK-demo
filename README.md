# Hush Network STARK Demo

STARK circuit demo for [Hush Network](https://hushnetwork.io): credential-gated private payments on [Stwo](https://github.com/starkware-libs/stwo) (FRI-based STARK prover, Mersenne31 field). All hashing constrained via Poseidon2 AIR with domain separation.

## Circuits

| Circuit | Columns | Prove | Verify | Description |
|---------|---------|-------|--------|-------------|
| Payment | 44,192 | ~1.2s | ~20ms | 2-in-2-out private transfer, depth-20 Merkle, 21-bit range checks |
| Credential Issuance | 14,058 | ~300ms | (combined) | Authorized issuer creates a credential, depth-20 issuer Merkle |
| Time-Window Audit | 14,942 | ~350ms | (combined) | Aggregate totals over a period, 16 slots, depth-20 credential Merkle |

Benchmarks: native release mode, commodity hardware.

## What the payment circuit proves

Owner derivation, input/output note commitments, Merkle inclusion (2 note paths + 1 credential path), nullifier derivation and uniqueness, balance conservation, credential validity (commitment + expiry range check + Merkle inclusion), and credential nullifier binding.

Public outputs bound via Fiat-Shamir: nullifiers, output commitments, credential nullifier. Everything a validator needs to update ledger state.

## Cryptographic stack

| Component | Choice |
|-----------|--------|
| Proving system | STARK (FRI), transparent, no trusted setup |
| Prover | Stwo |
| Field | Mersenne31 (M31) |
| In-circuit hash | Poseidon2 (width-16, Plonky3 constants, domain-separated) |
| Commitment backend | Poseidon252 (native) / Blake2s (WASM) |

## Development

```bash
scripts/test.sh     # run tests (50 tests + proptests)
scripts/bench.sh    # benchmarks
scripts/fmt.sh      # format
cargo clippy -- -D warnings
```

See [docs/architecture.md](docs/architecture.md) for module layout and design decisions.

## Binaries

```bash
cargo run --bin lifecycle --release   # full protocol flow demo
cargo run --bin bench --release       # performance benchmarks
```

## Limitations

This is a constraint architecture demo. Known gaps:

- **Field size.** Single M31 elements (~31 bits). Production needs multi-element outputs for collision resistance.
- **Key space.** Single M31 spending keys. Production needs multi-field-element keys.
- **Fixed shape.** 2-in-2-out only. Extends naturally but not implemented.
- **No recursion.** Recursive verification planned but not implemented.
- **No serialization.** Proofs generated and verified in-process.

## License

MIT
