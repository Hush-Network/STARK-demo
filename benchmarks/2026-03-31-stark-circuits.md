# STARK Circuit Benchmarks

**Date:** 2026-03-31
**Hardware:** AMD Ryzen 9 / Windows 10 Pro
**Build:** release (opt-level 3, LTO enabled)
**Prover:** Stwo (FRI over Mersenne31)
**Iterations:** 10 per circuit

## Results

| Circuit             | Prove (avg) | Prove (min) | Prove (max) | Verify (avg) |
|---------------------|-------------|-------------|-------------|--------------|
| Payment             |      906ms  |      850ms  |      974ms  |       115ms  |
| Credential Issuance |      282ms  |      273ms  |      294ms  |   (combined) |
| Time-Window Audit   |      297ms  |      282ms  |      316ms  |   (combined) |

## Circuit details

**Payment (2-in-2-out):**
- ~44,000 trace columns
- 3x depth-20 Merkle verifications
- 9 Poseidon2 hashes (owner, 2 nullifiers, 2 input CMs, 2 output CMs, cred CM, cred nullifier)
- 4x 21-bit amount range checks
- 1x 16-bit credential expiry range check
- Nullifier inequality via multiplicative inverse

**Credential Issuance:**
- ~13,000 trace columns
- 1x depth-20 Merkle verification (issuer set)
- 2 Poseidon2 hashes (issuer ID derivation, credential commitment)

**Time-Window Audit (16 slots):**
- 16 per-transaction window checks with binary flag
- 2x 24-bit range checks per transaction (timestamp bounds)
- 1x depth-20 Merkle verification (credential set)
- Sum constraint over conditional contributions

## Notes

- Single-threaded, no batching, no recursion
- WASM (browser) performance is approximately 3-5x slower than native
- Stwo is under active development; these numbers will improve
- Recursive aggregation (not yet implemented) amortizes verification cost across batches

## Reproduce

```bash
cargo run --release --bin bench
```
