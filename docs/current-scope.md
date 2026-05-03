# Current Scope

What this repository implements today, what is demo scaffolding, and what is intentionally not implemented here. The intent is to make the boundary between cryptographic/protocol code and browser demo scaffolding obvious to a reviewer.

## Implemented today

- **Payment circuit** (`src/circuit.rs`): 2-in-2-out private transfer with provenance attestation root verification, non-revocation check against an accumulator, balance conservation across four 15-bit limbs per amount, nullifier derivation and inequality, and depth-20 Merkle path verification for both note inputs and the attestation root.
- **Provenance attestation circuit** (`src/provenance_attestation.rs`): proves a private note carries an attestation signed by an approved boundary actor with Merkle inclusion in the boundary actor set.
- **Time-window audit circuit** (`src/time_window.rs`): proves aggregate volume across up to 16 transactions in a defined window, surfaced to the user as an audit key.
- **Poseidon2 hash + AIR** (`src/poseidon2.rs`, `src/poseidon2_air.rs`): width-16 over Mersenne31, with degree-2 constraint decomposition for in-circuit verification. Constants verified against Plonky3 vectors.
- **Payment tx encoding + binding hash** (`src/payment_tx.rs`): canonical encoding of payment + fee descriptors and the binding-hash domain separation used by every transaction proof.
- **Payment + fee bundle validation** (`src/payment_validation.rs`): validates a (payment, fee sidecar) bundle end to end before submission.
- **HUSH fee sidecar** (`src/fee_sidecar.rs`): independent proof that pays the protocol fee in HUSH against the same binding hash as the payment.
- **Dual-fee runtime** (`src/dual_fee_runtime.rs`): quote and submit paths exposed to the browser demo. Mode A is same-asset fee; Mode B is the HUSH sidecar.
- **Block accounting** (`src/accounting.rs`): protocol-action accounting and validator payout primitives.
- **Browser WASM bindings** (`src/wasm.rs`): narrow surface used by the live demo. Exports cover proof construction, proof verification, audit-proof construction and verification, the dual-fee quote/submit flow, and a binding-hash recompute helper used by the receipt verifier.
- **Browser demo** (`web/`): wallet shell, payment composer, audit overlay, receipt verifier. Uses Vite for build.
- **Native benchmarks** (`src/bin/bench.rs`): single-threaded and `--features parallel` paths for all three circuits.
- **Lifecycle binary** (`src/bin/lifecycle.rs`): end-to-end attestation -> payment -> audit flow over native code.
- **Test suite** (`cargo test`): circuit correctness, balance / nullifier / provenance / revocation rejection cases, Poseidon2 AIR correctness against Plonky3, and helper coverage.

## Demo-only assumptions

These live in the browser demo and the WASM helpers it calls. They are not part of the protocol design and they are not part of the proving stack truth.

- **Hardcoded demo identities** in `web/src/main.js`: `SK`, `CRED_ISSUER`, `CRED_EXPIRY`, `CRED_SECRET`, `USER_HANDLE`, `DEFAULT_RECIPIENT`, `DEFAULT_AMOUNT`. These exist so a visitor can produce a real proof without going through wallet onboarding.
- **Hardcoded starting balances** in `web/src/main.js`: `INITIAL_BALANCES_UNITS` (USDC, USDT) and `INITIAL_HUSH_BALANCE_UNITS`.
- **Demo HUSH spot price** (`HUSH_USD_PRICE` in `web/src/main.js`): a fixed display rate used to render the balance card. The proving stack does not consume this number.
- **Single boundary actor in `prove_demo_provenance_attestation`** (`src/wasm.rs`): builds a one-leaf Merkle tree to keep the demo path small. The circuit constraints are unchanged.
- **Demo wallet state** in `web/src/main.js`: balances, transactions, activity, proof log are kept entirely in memory and reset on reload.

A reviewer can identify demo state by the constants block at the top of `web/src/main.js` and by anything prefixed `prove_demo_*` in `src/wasm.rs`.

## Not implemented in this repo

- Live validator network and consensus
- Live ledger, mempool, or block production
- Issuer integration and live boundary-actor signing infrastructure
- Production wallet SDK or note discovery
- Fee extraction pipeline at the network layer
- Validator incentive flow at the network layer
- Recursive proof aggregation
- Threshold-encrypted mempool
- Full revocation update pipeline (the circuit checks non-revocation against a publishable accumulator root; the network surface that mutates that accumulator lives elsewhere)

## Known limitations

- WASM build uses Blake2s for the Merkle commitment backend instead of Poseidon252. Native build uses Poseidon252. Proof shape and validity are unaffected; this is a backend-only divergence to keep WASM dependencies minimal.
- `pow_bits=0` in the Stwo config because of an upstream bug in non-parallel Poseidon252 grinding. This affects DoS hardening rather than soundness.
- Fixed-width amount encoding caps payment amounts at the four-15-bit-limb range described in `docs/architecture.md`. This shape is intentional: the circuit cost does not vary with payment value within the supported range.
- Single boundary actor in the demo (see above). The constraint system supports a depth-20 Merkle tree of boundary actors; the demo just pre-populates one leaf.

## Next implementation priorities

These are tracked but not in this repo:

1. Validator-network proof submission path (lives in the alphanet repo)
2. Recursive proof aggregation across batched transactions
3. Boundary actor set management beyond the demo single-leaf tree
4. Threshold-encrypted mempool

For broader Hush Network architecture, scope, and current network state see the public Hush Network site rather than this repo.
