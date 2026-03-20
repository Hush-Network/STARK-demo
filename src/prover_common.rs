//! Shared prover/verifier types and config.

#[cfg(target_arch = "wasm32")]
pub use stwo::core::channel::Blake2sChannel as ProverChannel;
// Poseidon252 for native (algebraic, recursion-ready). Blake2s for WASM.
#[cfg(not(target_arch = "wasm32"))]
pub use stwo::core::channel::Poseidon252Channel as ProverChannel;
#[cfg(target_arch = "wasm32")]
pub use stwo::core::vcs::blake2_merkle::{
    Blake2sMerkleChannel as ProverMerkleChannel, Blake2sMerkleHasher as ProverMerkleHasher,
};
#[cfg(not(target_arch = "wasm32"))]
pub use stwo::core::vcs::poseidon252_merkle::{
    Poseidon252MerkleChannel as ProverMerkleChannel, Poseidon252MerkleHasher as ProverMerkleHasher,
};
use stwo::core::{fri::FriConfig, pcs::PcsConfig};

/// HACK: pow_bits=0 because stwo's non-parallel Poseidon252 grind passes raw digest
/// but verify expects prefixed_digest. Not relevant for proof correctness anyway.
pub fn pcs_config() -> PcsConfig {
    PcsConfig { pow_bits: 0, fri_config: FriConfig::new(0, 1, 3) }
}
