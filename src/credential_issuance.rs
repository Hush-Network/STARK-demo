//! Credential issuance circuit.
//!
//! All hash outputs are `HashOut = [M31; 4]` (4 x M31, ~124-bit collision resistance).
//! Credential commitment uses a 2-block sponge (10 inputs: issuer[4], owner[4], expiry, secret).
//! Merkle siblings are HashOut; `constrain_hash_pair` hashes 8 rate elements per level.

use num_traits::{One, Zero};
use stwo::{
    core::{
        air::Component,
        channel::Channel,
        fields::{m31::M31, qm31::QM31},
        pcs::CommitmentSchemeVerifier,
        poly::circle::CanonicCoset,
        verifier::verify,
        ColumnVec,
    },
    prover::{
        backend::{
            simd::{column::BaseColumn, m31::LOG_N_LANES, SimdBackend},
            Column,
        },
        poly::{
            circle::{CircleEvaluation, PolyOps},
            BitReversedOrder,
        },
        prove, CommitmentSchemeProver,
    },
};
use stwo_constraint_framework::{
    EvalAtRow, FrameworkComponent, FrameworkEval, TraceLocationAllocator,
};

use crate::{
    poseidon2,
    poseidon2::HashOut,
    poseidon2_air,
    prover_common::{pcs_config, ProverChannel, ProverMerkleChannel},
    types::MERKLE_DEPTH,
};

const LOG_CONSTRAINT_EVAL_BLOWUP_FACTOR: u32 = 1;

/// Per Merkle level: sibling(4) + direction(1) + left(4) + hash_pair intermediates.
const MERKLE_LEVEL_COLS: usize = 9 + poseidon2_air::HASH_INTERMEDIATE_COLS;

/// Total trace columns:
///   witness:  issuer_key(1) + subject(4) + expiry(1) + secret(1) = 7
///   public:   pub_issuer_root(4) + pub_cred_cm(4) = 8
///   issuer_id hash:      HASH_INTERMEDIATE_COLS  (single-block hash2)
///   credential_commitment: SPONGE_2BLOCK_INTERMEDIATE_COLS  (10-input, 2-block sponge)
///   Merkle path:          MERKLE_DEPTH * MERKLE_LEVEL_COLS
const NUM_COLS: usize = 15
    + poseidon2_air::HASH_INTERMEDIATE_COLS
    + poseidon2_air::SPONGE_2BLOCK_INTERMEDIATE_COLS
    + MERKLE_DEPTH * MERKLE_LEVEL_COLS;

#[derive(Clone, Debug)]
pub struct IssuanceWitness {
    pub issuer_root: [u32; 4],
    pub credential_commitment: [u32; 4],
    pub issuer_key: u32,
    /// `derive_owner(sk)` output, a HashOut.
    pub subject: [u32; 4],
    pub expiry: u32,
    pub secret: u32,
    pub issuer_path: [([u32; 4], u32); MERKLE_DEPTH],
}

#[derive(Clone)]
pub struct IssuanceEval {
    pub log_size: u32,
}

impl FrameworkEval for IssuanceEval {
    fn log_size(&self) -> u32 {
        self.log_size
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.log_size + LOG_CONSTRAINT_EVAL_BLOWUP_FACTOR
    }

    fn evaluate<E: EvalAtRow>(&self, mut eval: E) -> E {
        // --- witness scalars ---
        let issuer_key = eval.next_trace_mask(); // col 0
        let subject: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask()); // cols 1-4
        let expiry = eval.next_trace_mask(); // col 5
        let secret = eval.next_trace_mask(); // col 6

        // --- public values (4 limbs each) ---
        let pub_issuer_root: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask()); // cols 7-10
        let pub_cred_cm: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask()); // cols 11-14

        // --- derive issuer_id = hash2(issuer_key, 0, DOMAIN_ISSUER_ID) → [E::F; 4] ---
        let issuer_id = poseidon2_air::constrain_hash2(
            &mut eval,
            issuer_key,
            E::F::zero(),
            poseidon2::DOMAIN_ISSUER_ID,
        );

        // --- credential commitment = sponge_2block(issuer[0..4], owner[0..4], expiry, secret) ---
        let sponge_inputs: Vec<E::F> = vec![
            issuer_id[0].clone(),
            issuer_id[1].clone(),
            issuer_id[2].clone(),
            issuer_id[3].clone(),
            subject[0].clone(),
            subject[1].clone(),
            subject[2].clone(),
            subject[3].clone(),
            expiry,
            secret,
        ];
        let cm_out = poseidon2_air::constrain_sponge_2block(
            &mut eval,
            &sponge_inputs,
            poseidon2::DOMAIN_CRED_CM,
        );
        for k in 0..4 {
            eval.add_constraint(cm_out[k].clone() - pub_cred_cm[k].clone());
        }

        // --- Merkle path verification (HashOut siblings, constrain_hash_pair) ---
        let mut current = issuer_id;
        for _ in 0..MERKLE_DEPTH {
            let sibling: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());
            let direction = eval.next_trace_mask();
            let left: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());

            // direction in {0, 1}
            eval.add_constraint(direction.clone() * (direction.clone() - E::F::one()));

            // For each limb: left[k] = current[k] + direction * (sibling[k] - current[k])
            for k in 0..4 {
                eval.add_constraint(
                    left[k].clone()
                        - current[k].clone()
                        - direction.clone() * (sibling[k].clone() - current[k].clone()),
                );
            }

            // right[k] = current[k] + sibling[k] - left[k]
            let right: [E::F; 4] =
                core::array::from_fn(|k| current[k].clone() + sibling[k].clone() - left[k].clone());

            current = poseidon2_air::constrain_hash_pair(
                &mut eval,
                left,
                right,
                poseidon2::DOMAIN_MERKLE,
            );
        }

        // Constrain Merkle root (4 limbs)
        for k in 0..4 {
            eval.add_constraint(current[k].clone() - pub_issuer_root[k].clone());
        }

        eval
    }
}

pub type IssuanceComponent = FrameworkComponent<IssuanceEval>;

pub struct IssuancePublicData {
    pub issuer_root: [u32; 4],
    pub credential_commitment: [u32; 4],
}

impl IssuancePublicData {
    pub fn mix_into(&self, channel: &mut impl Channel) {
        for &limb in &self.issuer_root {
            channel.mix_u64(limb as u64);
        }
        for &limb in &self.credential_commitment {
            channel.mix_u64(limb as u64);
        }
    }
}

pub fn prove_issuance(witness: &IssuanceWitness) -> Result<(), String> {
    let log_num_rows = LOG_N_LANES;
    let num_rows = 1 << log_num_rows;

    #[cfg(debug_assertions)]
    eprintln!("[issuance] {NUM_COLS} cols, log_rows={log_num_rows}");

    let issuer_key = M31::from(witness.issuer_key);
    let subject: HashOut = poseidon2::u32_array_to_hashout(witness.subject);
    let expiry = M31::from(witness.expiry);
    let secret = M31::from(witness.secret);

    let issuer_id = poseidon2::derive_issuer_id(issuer_key);
    let computed_cm = poseidon2::credential_commitment(issuer_id, subject, expiry, secret);

    let expected_cm = poseidon2::u32_array_to_hashout(witness.credential_commitment);
    if computed_cm != expected_cm {
        return Err("Credential commitment does not match expected value".to_string());
    }

    // Verify issuer Merkle path
    let issuer_root = poseidon2::u32_array_to_hashout(witness.issuer_root);
    let issuer_path: Vec<(HashOut, u32)> =
        witness.issuer_path.iter().map(|&(s, d)| (poseidon2::u32_array_to_hashout(s), d)).collect();
    if !poseidon2::verify_merkle_path(issuer_id, &issuer_path, issuer_root) {
        return Err("Issuer is not in the authorized issuer set".to_string());
    }

    // Hash intermediates
    let issuer_id_cols = poseidon2_air::gen_hash2_intermediates(
        issuer_key,
        M31::from(0u32),
        poseidon2::DOMAIN_ISSUER_ID,
    );
    let cm_cols = poseidon2_air::gen_sponge_2block_intermediates(
        &[
            issuer_id[0],
            issuer_id[1],
            issuer_id[2],
            issuer_id[3],
            subject[0],
            subject[1],
            subject[2],
            subject[3],
            expiry,
            secret,
        ],
        poseidon2::DOMAIN_CRED_CM,
    );

    // Merkle path intermediates
    let merkle_data = gen_issuer_merkle_trace(issuer_id, &witness.issuer_path);

    let mut cols: Vec<BaseColumn> = (0..NUM_COLS).map(|_| BaseColumn::zeros(num_rows)).collect();

    let issuer_root_ho = poseidon2::u32_array_to_hashout(witness.issuer_root);
    let cred_cm_ho = poseidon2::u32_array_to_hashout(witness.credential_commitment);

    for r in 0..num_rows {
        let mut c = 0usize;
        // issuer_key (1)
        cols[c].set(r, issuer_key);
        c += 1;
        // subject (4)
        for k in 0..4 {
            cols[c].set(r, subject[k]);
            c += 1;
        }
        // expiry, secret (2)
        cols[c].set(r, expiry);
        c += 1;
        cols[c].set(r, secret);
        c += 1;
        // pub_issuer_root (4)
        for k in 0..4 {
            cols[c].set(r, issuer_root_ho[k]);
            c += 1;
        }
        // pub_cred_cm (4)
        for k in 0..4 {
            cols[c].set(r, cred_cm_ho[k]);
            c += 1;
        }
        // issuer_id hash intermediates
        for i in 0..issuer_id_cols.len() {
            cols[c + i].set(r, issuer_id_cols[i]);
        }
        c += issuer_id_cols.len();
        // credential commitment sponge intermediates
        for i in 0..cm_cols.len() {
            cols[c + i].set(r, cm_cols[i]);
        }
        c += cm_cols.len();
        // Merkle path intermediates
        for i in 0..merkle_data.len() {
            cols[c + i].set(r, merkle_data[i]);
        }
    }

    let domain = CanonicCoset::new(log_num_rows).circle_domain();
    let trace: ColumnVec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>> =
        cols.into_iter().map(|col| CircleEvaluation::new(domain, col)).collect();

    let public_data = IssuancePublicData {
        issuer_root: witness.issuer_root,
        credential_commitment: witness.credential_commitment,
    };

    let config = pcs_config();
    let twiddles = SimdBackend::precompute_twiddles(
        CanonicCoset::new(
            log_num_rows + LOG_CONSTRAINT_EVAL_BLOWUP_FACTOR + config.fri_config.log_blowup_factor,
        )
        .circle_domain()
        .half_coset,
    );

    let channel = &mut ProverChannel::default();
    let mut commitment_scheme =
        CommitmentSchemeProver::<SimdBackend, ProverMerkleChannel>::new(config, &twiddles);

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(vec![]);
    tree_builder.commit(channel);

    channel.mix_u64(log_num_rows as u64);
    public_data.mix_into(channel);

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(trace);
    tree_builder.commit(channel);

    let component = IssuanceComponent::new(
        &mut TraceLocationAllocator::default(),
        IssuanceEval { log_size: log_num_rows },
        QM31::zero(),
    );

    let proof = prove(&[&component], channel, commitment_scheme)
        .map_err(|e| format!("Issuance proof generation failed: {e:?}"))?;

    // Verify inline
    let channel = &mut ProverChannel::default();
    let commitment_scheme = &mut CommitmentSchemeVerifier::<ProverMerkleChannel>::new(config);
    let sizes = component.trace_log_degree_bounds();

    commitment_scheme.commit(proof.commitments[0], &sizes[0], channel);
    channel.mix_u64(log_num_rows as u64);
    public_data.mix_into(channel);
    commitment_scheme.commit(proof.commitments[1], &sizes[1], channel);

    verify(&[&component], channel, commitment_scheme, proof)
        .map_err(|e| format!("Issuance verification failed: {e:?}"))
}

fn gen_issuer_merkle_trace(leaf: HashOut, path: &[([u32; 4], u32); MERKLE_DEPTH]) -> Vec<M31> {
    let mut result = Vec::with_capacity(MERKLE_DEPTH * MERKLE_LEVEL_COLS);
    let mut current = leaf;

    for &(sibling_arr, direction_val) in path.iter() {
        let sibling = poseidon2::u32_array_to_hashout(sibling_arr);

        let (left, right) =
            if direction_val == 0 { (current, sibling) } else { (sibling, current) };

        // sibling (4 limbs)
        for k in 0..4 {
            result.push(sibling[k]);
        }
        // direction (1)
        result.push(M31::from(direction_val));
        // left (4 limbs)
        for k in 0..4 {
            result.push(left[k]);
        }

        let hash_cols =
            poseidon2_air::gen_hash_pair_intermediates(left, right, poseidon2::DOMAIN_MERKLE);
        result.extend_from_slice(&hash_cols);

        current = poseidon2::merkle_hash(left, right);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convert a HashOut Merkle path to the witness format `[([u32; 4], u32); MERKLE_DEPTH]`.
    fn path_to_witness(path_vec: &[(HashOut, u32)]) -> [([u32; 4], u32); MERKLE_DEPTH] {
        let mut out = [([0u32; 4], 0u32); MERKLE_DEPTH];
        for i in 0..MERKLE_DEPTH {
            out[i] = (poseidon2::hashout_to_u32_array(path_vec[i].0), path_vec[i].1);
        }
        out
    }

    #[test]
    fn test_issuance_roundtrip() {
        let issuer_key = M31::from(42u32);
        let issuer_id = poseidon2::derive_issuer_id(issuer_key);
        let subject = poseidon2::derive_owner(M31::from(12345u32));
        let cm = poseidon2::credential_commitment(
            issuer_id,
            subject,
            M31::from(2000u32),
            M31::from(777u32),
        );

        let mut issuer_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
        issuer_tree.set_leaf(0, issuer_id);
        let issuer_root = issuer_tree.root();
        let path_vec = issuer_tree.path(0);

        let witness = IssuanceWitness {
            issuer_root: poseidon2::hashout_to_u32_array(issuer_root),
            credential_commitment: poseidon2::hashout_to_u32_array(cm),
            issuer_key: 42,
            subject: poseidon2::hashout_to_u32_array(subject),
            expiry: 2000,
            secret: 777,
            issuer_path: path_to_witness(&path_vec),
        };

        prove_issuance(&witness).expect("Issuance should succeed");
    }

    #[test]
    fn test_wrong_issuer_key() {
        let issuer_key = M31::from(42u32);
        let issuer_id = poseidon2::derive_issuer_id(issuer_key);
        let subject = poseidon2::derive_owner(M31::from(12345u32));
        let cm = poseidon2::credential_commitment(
            issuer_id,
            subject,
            M31::from(2000u32),
            M31::from(777u32),
        );

        // Empty tree: issuer not present
        let empty_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
        let bad_root = empty_tree.root();
        let path_vec = empty_tree.path(0);

        let witness = IssuanceWitness {
            issuer_root: poseidon2::hashout_to_u32_array(bad_root),
            credential_commitment: poseidon2::hashout_to_u32_array(cm),
            issuer_key: 42,
            subject: poseidon2::hashout_to_u32_array(subject),
            expiry: 2000,
            secret: 777,
            issuer_path: path_to_witness(&path_vec),
        };

        match prove_issuance(&witness) {
            Err(e) => assert!(e.contains("authorized issuer set"), "Got: {e}"),
            Ok(_) => panic!("Should have rejected unauthorized issuer"),
        }
    }
}
