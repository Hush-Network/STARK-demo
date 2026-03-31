# Hush Network Architecture

Hush is a private-by-default stablecoin settlement L1. This document covers the architecture decisions, how the system scales, and what the hard unsolved problems are.

## Why L1

Private payment settlement with issuer-aware compliance cannot be safely delegated to a layer that doesn't control its own mempool, execution, and state commitment.

### Where rollups and appchains leak

**Mempool exposure.** Rollups submit transactions to an L1 sequencer or shared mempool. Even if the transaction payload is encrypted, the metadata (sender address, gas payment, submission timing) is visible to the host chain. For a payment network where the relationship graph is the most sensitive data, this is a fundamental leak. Threshold-encrypted mempools require coordination at the consensus layer, which rollups don't control.

**Data availability.** Rollups post transaction data (or state diffs) to the host chain's DA layer. Even with encrypted payloads, the volume, timing, and size of DA blobs leak information about network activity. An L1 controls its own DA and can design the blob format to resist traffic analysis.

**Execution environment.** On a shared execution layer, other applications share the same proof aggregation pipeline. A privacy payment system can't tolerate its proofs being batched with arbitrary other programs that may have different trust assumptions, upgrade cycles, or compliance requirements.

**Issuer controls.** Stablecoin issuers (Circle, Tether) require asset-level enforcement: freeze, force-transfer, compliance holds. These controls must be rule-constrained and transparently auditable, applied only to the issuer's own asset, with no chain-wide surveillance capability. This requires protocol-level support that a rollup or appchain can't guarantee across upgrades to its host chain.

**Bridge risk.** Moving assets between layers introduces bridge contracts, which are surveillance points (all deposits/withdrawals are visible on the host chain) and security risks. Native issuance on the L1 means stablecoins are created directly into the private ledger. There is no public-to-private boundary for observers to watch.

### What the L1 controls

- **Mempool:** threshold encrypted, decrypted at block time, fair ordering post-decryption
- **Consensus:** HotStuff-2 BFT with BLS-aggregated signatures, deterministic finality
- **State commitment:** private UTXO set, nullifier set, credential set all managed at the base layer
- **Upgrade governance:** protocol changes are governed by the network, not inherited from a host chain
- **Issuer integration:** constrained issuer controls enforced at the protocol layer, not by smart contract

## System design

### Private state model

Hush uses a note-based (UTXO-style) ledger. Each note is a tuple of (asset, amount, owner, randomness) committed via Poseidon2. Notes are consumed by revealing their nullifier (derived from the owner's secret key and the note commitment) and creating new notes with fresh commitments.

This model has two properties that matter for private payments:

1. **No shared mutable state.** Unlike account-based systems, notes are independent. Two transactions that don't consume the same note can't conflict. This eliminates the need for a global execution lock and makes parallel proving possible.

2. **Natural privacy boundary.** Each note commitment reveals nothing about its contents. The nullifier reveals nothing about which note was consumed (to anyone who doesn't know the secret key). The only public data is: a nullifier was used, and new commitments were added.

### Credential model

All participants hold a zero-knowledge credential proving eligibility (not sanctioned, verified status, etc.). The credential is committed via Poseidon2 and included in a Merkle-committed credential set. Every transaction proof includes a sub-proof that the sender holds a valid, non-expired credential from an authorized issuer, without revealing which credential or which issuer.

Credential issuance is itself a STARK proof: the issuer proves they are in the authorized issuer set and that the credential commitment is correctly formed.

Revocation is handled by epoch-based expiry and a separate revocation nullifier. Credentials must be refreshed periodically, giving issuers a natural enforcement point.

### Selective disclosure (three scopes)

**Receipt scope.** Per-transaction. The sender generates a STARK-backed receipt containing only the fields they choose to disclose (amount, date, recipient, asset, sender identity, balance). The proof cryptographically binds the disclosed fields to the real transaction. Used for proof of payment, merchant settlements, payroll confirmation.

**Time-window scope.** Aggregate. The sender proves facts about a set of transactions within a date range (total volume, transaction count, individual recipients) without revealing undisclosed fields. The STARK proof covers the aggregate claim. Used for regulatory audits, tax reporting, institutional compliance.

**Eligibility scope.** Protocol-level. Every transaction proves the sender holds a valid credential. The network verifies compliance at transaction time without learning the sender's identity. Used for sanctions compliance, permissioned access.

### Scaling path

**Recursive proof aggregation.** STARK proofs can verify other STARK proofs. A block producer batches N transaction proofs into a single recursive proof. The marginal cost per transaction decreases as batch size increases. Not implemented yet, but the circuit architecture is designed for it (Poseidon2 over M31 is recursion-friendly).

**Batching economics.** At current single-proof performance (906ms prove, 115ms verify for a payment), a batch of 100 transactions with recursive aggregation would amortize the verification cost to roughly 1ms per transaction. Proving can be parallelized across cores since notes are independent.

**State growth.** The nullifier set grows by 2 entries per transaction (one per consumed note). At 1M transactions per day, that's 730M nullifiers per year. Each nullifier is a single M31 field element (4 bytes), so roughly 2.9 GB/year of nullifier storage. The note commitment tree is append-only and can be pruned once notes are spent (their nullifiers are in the set). The credential tree is bounded by the number of active participants.

## What isn't solved yet

These are the hardest problems. We're not going to pretend they're done.

**Note discovery.** How does a recipient know they received funds? The current design uses deterministic encrypted tags (the sender encrypts a detection tag using the recipient's public detection key). For mobile/intermittent clients, a relay-assisted protocol using fuzzy message detection (FMD) is planned, where the relay can filter likely-matching notes without learning the recipient's identity. This is an active research area (Penumbra's FMD paper is the current state of the art).

**Mempool encryption.** The threshold encryption scheme for the mempool (where validators hold key shares and decrypt transactions at block time) is specified but not implemented. The cryptographic primitive is standard (threshold ElGamal or similar), but the engineering challenge is key rotation, liveness under validator churn, and latency impact.

**Proof size and recursion.** Current STARK proofs are large compared to SNARKs (~100KB vs ~200B). Recursive aggregation reduces the per-block proof to a single STARK, but the individual proof size affects mempool bandwidth. FRI-based proof compression and ongoing Stwo development will improve this.

**Wallet proving constraints.** The payment circuit takes ~900ms to prove on a desktop CPU. This needs to reach sub-200ms for production wallet UX. The path is circuit optimization (reduce unnecessary columns), Stwo performance improvements (actively being optimized upstream), and optional delegated proving with privacy-preserving protocols.

**Validator economics.** The consensus mechanism (HotStuff-2 BFT) is specified with BLS-aggregated signatures, but the staking model, reward distribution, and slashing conditions aren't finalized. These depend on network parameters we don't have yet.

## Related work

Hush builds on research from across the privacy and STARK ecosystem:

- **Zcash** (Sapling/Orchard) pioneered the note-based privacy model, nullifier design, and selective disclosure that Hush draws from. Hush adds native multi-asset support, constrained issuer controls, and STARK proving (vs Groth16/Halo2).
- **Penumbra** applies shielded UTXO design to DeFi on Cosmos. Hush's credential model is heavier (issuer-aware compliance), targeting regulated stablecoin settlement rather than general shielded DeFi.
- **Aztec** is building general-purpose private execution. Hush is narrower: payments only, with compliance built into the protocol rather than left to application developers.
- **STRK20 / Starknet privacy layers** bring private transfers to an existing DeFi ecosystem. Hush's position is that privacy at the application layer inherits the metadata leaks of the host chain (mempool, DA, bridge visibility), and that base-layer privacy avoids these.
- **Stwo** (StarkWare) provides the prover architecture and FRI over Mersenne31.
- **Poseidon2** (Grassi et al.) provides the STARK-friendly hash construction.
