//! Time-window audit circuit. 16 slots, u64 amounts via 4x15-bit limbs, 24-bit timestamp range.

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
    poseidon2, poseidon2_air,
    prover_common::{pcs_config, ProverChannel, ProverMerkleChannel, ProverMerkleHasher},
    types::{amount_to_limbs, MERKLE_DEPTH, NUM_LIMBS, RADIX_U32},
};

const LOG_CONSTRAINT_EVAL_BLOWUP_FACTOR: u32 = 1;
const MAX_TX: usize = 16;
const RANGE_BITS: usize = 24;
const SUM_CARRY_BITS: usize = 4;
const NUM_CARRIES: usize = NUM_LIMBS - 1;

const MERKLE_LEVEL_COLS: usize = 9 + poseidon2_air::HASH_INTERMEDIATE_COLS;

const COLS_PER_TX: usize = 1 + NUM_LIMBS + 2 * (1 + RANGE_BITS);

const TX_COLS_START: usize =
    2 + NUM_LIMBS + 4 + 4 + 5 + MAX_TX * NUM_LIMBS + MAX_TX + 1 + 16 + NUM_CARRIES * SUM_CARRY_BITS;

const HASH_COLS_START: usize = TX_COLS_START + MAX_TX * COLS_PER_TX;
const HASH_TOTAL: usize =
    3 * poseidon2_air::HASH_INTERMEDIATE_COLS + poseidon2_air::SPONGE_2BLOCK_INTERMEDIATE_COLS;
const MERKLE_START: usize = HASH_COLS_START + HASH_TOTAL;
const NUM_COLS: usize = MERKLE_START + MERKLE_DEPTH * MERKLE_LEVEL_COLS;

#[derive(Clone, Debug)]
pub struct TimeWindowWitness {
    pub window_start: u32,
    pub window_end: u32,
    pub claimed_total: u64,
    pub cred_root: [u32; 4],
    pub epoch: u32,
    pub tx_amounts: [u64; MAX_TX],
    pub tx_timestamps: [u32; MAX_TX],
    pub tx_count: usize,
    pub sk: u32,
    pub cred_issuer: u32,
    pub cred_expiry: u32,
    pub cred_secret: u32,
    pub cred_path: [([u32; 4], u32); MERKLE_DEPTH],
}

#[derive(Clone)]
pub struct TimeWindowEval {
    pub log_size: u32,
}

impl FrameworkEval for TimeWindowEval {
    fn log_size(&self) -> u32 {
        self.log_size
    }
    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.log_size + LOG_CONSTRAINT_EVAL_BLOWUP_FACTOR
    }
    fn evaluate<E: EvalAtRow>(&self, mut eval: E) -> E {
        let window_start = eval.next_trace_mask();
        let window_end = eval.next_trace_mask();
        let claimed_total_limbs: [E::F; NUM_LIMBS] =
            core::array::from_fn(|_| eval.next_trace_mask());
        let pub_cred_root: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());
        let pub_cred_null: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());
        let epoch = eval.next_trace_mask();
        let sk = eval.next_trace_mask();
        let cred_issuer = eval.next_trace_mask();
        let cred_expiry = eval.next_trace_mask();
        let cred_secret = eval.next_trace_mask();

        let mut amount_limbs: [[E::F; NUM_LIMBS]; MAX_TX] =
            core::array::from_fn(|_| core::array::from_fn(|_| E::F::zero()));
        for i in 0..MAX_TX {
            for k in 0..NUM_LIMBS {
                amount_limbs[i][k] = eval.next_trace_mask();
            }
        }
        let mut timestamps: Vec<E::F> = Vec::with_capacity(MAX_TX);
        for _ in 0..MAX_TX {
            timestamps.push(eval.next_trace_mask());
        }

        let two = E::F::one() + E::F::one();

        // Credential expiry range check
        let expiry_diff = eval.next_trace_mask();
        let mut reconstructed = E::F::zero();
        let mut pow2 = E::F::one();
        for _ in 0..16 {
            let bit = eval.next_trace_mask();
            eval.add_constraint(bit.clone() * (bit.clone() - E::F::one()));
            reconstructed += bit * pow2.clone();
            pow2 *= two.clone();
        }
        eval.add_constraint(reconstructed - expiry_diff.clone());
        eval.add_constraint(expiry_diff - (cred_expiry.clone() - epoch.clone() - E::F::one()));

        // Summation carry bits
        let radix = E::F::from(M31::from(RADIX_U32));
        let mut carries: [E::F; NUM_CARRIES] = core::array::from_fn(|_| E::F::zero());
        for k in 0..NUM_CARRIES {
            let mut carry_recon = E::F::zero();
            let mut p2 = E::F::one();
            for _ in 0..SUM_CARRY_BITS {
                let bit = eval.next_trace_mask();
                eval.add_constraint(bit.clone() * (bit.clone() - E::F::one()));
                carry_recon += bit * p2.clone();
                p2 *= two.clone();
            }
            carries[k] = carry_recon;
        }

        // Per-transaction constraints
        let mut total_sum_limbs: [E::F; NUM_LIMBS] = core::array::from_fn(|_| E::F::zero());
        for i in 0..MAX_TX {
            let in_window = eval.next_trace_mask();
            let contribution_limbs: [E::F; NUM_LIMBS] =
                core::array::from_fn(|_| eval.next_trace_mask());
            eval.add_constraint(in_window.clone() * (in_window.clone() - E::F::one()));
            for k in 0..NUM_LIMBS {
                eval.add_constraint(
                    contribution_limbs[k].clone() - in_window.clone() * amount_limbs[i][k].clone(),
                );
            }

            let ts_lower = eval.next_trace_mask();
            let mut lower_recon = E::F::zero();
            let mut pow2 = E::F::one();
            for _ in 0..RANGE_BITS {
                let bit = eval.next_trace_mask();
                eval.add_constraint(bit.clone() * (bit.clone() - E::F::one()));
                lower_recon += bit * pow2.clone();
                pow2 *= two.clone();
            }
            eval.add_constraint(in_window.clone() * (lower_recon - ts_lower.clone()));
            eval.add_constraint(
                in_window.clone() * (ts_lower - (timestamps[i].clone() - window_start.clone())),
            );

            let ts_upper = eval.next_trace_mask();
            let mut upper_recon = E::F::zero();
            let mut pow2 = E::F::one();
            for _ in 0..RANGE_BITS {
                let bit = eval.next_trace_mask();
                eval.add_constraint(bit.clone() * (bit.clone() - E::F::one()));
                upper_recon += bit * pow2.clone();
                pow2 *= two.clone();
            }
            eval.add_constraint(in_window.clone() * (upper_recon - ts_upper.clone()));
            eval.add_constraint(
                in_window * (ts_upper - (window_end.clone() - timestamps[i].clone())),
            );

            for k in 0..NUM_LIMBS {
                total_sum_limbs[k] = total_sum_limbs[k].clone() + contribution_limbs[k].clone();
            }
        }

        for k in 0..NUM_LIMBS {
            let c_prev = if k == 0 { E::F::zero() } else { carries[k - 1].clone() };
            let lhs = total_sum_limbs[k].clone() - claimed_total_limbs[k].clone() + c_prev;
            if k < NUM_CARRIES {
                eval.add_constraint(lhs - carries[k].clone() * radix.clone());
            } else {
                eval.add_constraint(lhs);
            }
        }

        // Hash constraints
        let owner_out =
            poseidon2_air::constrain_hash2(&mut eval, sk, E::F::zero(), poseidon2::DOMAIN_OWNER);
        let issuer_id_out = poseidon2_air::constrain_hash2(
            &mut eval,
            cred_issuer,
            E::F::zero(),
            poseidon2::DOMAIN_ISSUER_ID,
        );
        let cm_inputs: Vec<E::F> = vec![
            issuer_id_out[0].clone(),
            issuer_id_out[1].clone(),
            issuer_id_out[2].clone(),
            issuer_id_out[3].clone(),
            owner_out[0].clone(),
            owner_out[1].clone(),
            owner_out[2].clone(),
            owner_out[3].clone(),
            cred_expiry.clone(),
            cred_secret.clone(),
        ];
        let cm_out = poseidon2_air::constrain_sponge_2block(
            &mut eval,
            &cm_inputs,
            poseidon2::DOMAIN_CRED_CM,
        );
        let crednull_out = poseidon2_air::constrain_hash_many_7(
            &mut eval,
            cred_secret,
            cm_out[0].clone(),
            cm_out[1].clone(),
            cm_out[2].clone(),
            cm_out[3].clone(),
            epoch,
            E::F::zero(),
            poseidon2::DOMAIN_CRED_NULL,
        );
        for j in 0..4 {
            eval.add_constraint(pub_cred_null[j].clone() - crednull_out[j].clone());
        }

        let mut current = cm_out;
        for _ in 0..MERKLE_DEPTH {
            let sibling: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());
            let direction = eval.next_trace_mask();
            let left: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());
            eval.add_constraint(direction.clone() * (direction.clone() - E::F::one()));
            for j in 0..4 {
                eval.add_constraint(
                    left[j].clone()
                        - current[j].clone()
                        - direction.clone() * (sibling[j].clone() - current[j].clone()),
                );
            }
            let right: [E::F; 4] =
                core::array::from_fn(|j| current[j].clone() + sibling[j].clone() - left[j].clone());
            current = poseidon2_air::constrain_hash_pair(
                &mut eval,
                left,
                right,
                poseidon2::DOMAIN_MERKLE,
            );
        }
        for j in 0..4 {
            eval.add_constraint(current[j].clone() - pub_cred_root[j].clone());
        }

        eval
    }
}

pub type TimeWindowComponent = FrameworkComponent<TimeWindowEval>;

pub struct TimeWindowPublicData {
    pub window_start: u32,
    pub window_end: u32,
    pub claimed_total: u64,
    pub cred_root: [u32; 4],
    pub cred_null: [u32; 4],
    pub epoch: u32,
}

impl TimeWindowPublicData {
    pub fn mix_into(&self, channel: &mut impl Channel) {
        channel.mix_u64(self.window_start as u64);
        channel.mix_u64(self.window_end as u64);
        channel.mix_u64(self.claimed_total);
        for &v in &self.cred_root {
            channel.mix_u64(v as u64);
        }
        for &v in &self.cred_null {
            channel.mix_u64(v as u64);
        }
        channel.mix_u64(self.epoch as u64);
    }
}

/// Proof result for independent verification (mirrors circuit::ProofResult).
pub struct AuditProofResult {
    pub proof: stwo::core::proof::StarkProof<ProverMerkleHasher>,
    pub component: TimeWindowComponent,
    pub public_data: TimeWindowPublicData,
    pub log_num_rows: u32,
}

/// Generate a time-window audit proof. Returns the proof for independent verification.
pub fn prove_time_window(witness: &TimeWindowWitness) -> Result<AuditProofResult, String> {
    let log_num_rows = LOG_N_LANES;
    let num_rows = 1 << log_num_rows;

    if witness.window_start >= witness.window_end {
        return Err("Invalid window: start must be before end".to_string());
    }

    let mut actual_total: u64 = 0;
    for i in 0..witness.tx_count {
        let ts = witness.tx_timestamps[i];
        if ts >= witness.window_start && ts <= witness.window_end {
            actual_total = actual_total.checked_add(witness.tx_amounts[i]).ok_or_else(|| {
                "time-window total overflow: sum of in-window amounts exceeds u64".to_string()
            })?;
        }
    }
    if actual_total != witness.claimed_total {
        return Err(format!(
            "Claimed total {} does not match actual total {} for the window",
            witness.claimed_total, actual_total
        ));
    }

    let sk = M31::from(witness.sk);
    let owner = poseidon2::derive_owner(sk);
    let issuer_id = poseidon2::derive_issuer_id(M31::from(witness.cred_issuer));
    let cred_cm = poseidon2::credential_commitment(
        issuer_id,
        owner,
        M31::from(witness.cred_expiry),
        M31::from(witness.cred_secret),
    );
    let cred_null = poseidon2::credential_nullifier(
        M31::from(witness.cred_secret),
        cred_cm,
        M31::from(witness.epoch),
    );

    let cred_root = poseidon2::u32_array_to_hashout(witness.cred_root);
    let cred_path: Vec<(poseidon2::HashOut, u32)> =
        witness.cred_path.iter().map(|&(s, d)| (poseidon2::u32_array_to_hashout(s), d)).collect();
    if !poseidon2::verify_merkle_path(cred_cm, &cred_path, cred_root) {
        return Err("Credential root mismatch".to_string());
    }
    if witness.cred_expiry <= witness.epoch {
        return Err("Credential expired".to_string());
    }

    let ct_limbs = amount_to_limbs(witness.claimed_total);
    let mut tx_limbs = [[0u32; NUM_LIMBS]; MAX_TX];
    let mut in_window_flags = [false; MAX_TX];
    let mut contrib_limb_sums = [0u64; NUM_LIMBS];
    for i in 0..MAX_TX {
        tx_limbs[i] = amount_to_limbs(witness.tx_amounts[i]);
        let ts = witness.tx_timestamps[i];
        in_window_flags[i] =
            i < witness.tx_count && ts >= witness.window_start && ts <= witness.window_end;
        if in_window_flags[i] {
            for k in 0..NUM_LIMBS {
                contrib_limb_sums[k] += u64::from(tx_limbs[i][k]);
            }
        }
    }
    let mut carry_vals = [0u32; NUM_CARRIES];
    for k in 0..NUM_CARRIES {
        let c_prev = if k == 0 { 0u64 } else { u64::from(carry_vals[k - 1]) };
        let lhs = contrib_limb_sums[k] + c_prev - u64::from(ct_limbs[k]);
        carry_vals[k] = (lhs / u64::from(RADIX_U32)) as u32;
    }

    let mut cols: Vec<BaseColumn> = (0..NUM_COLS).map(|_| BaseColumn::zeros(num_rows)).collect();
    let expiry_diff_val = witness.cred_expiry - witness.epoch - 1;
    let mut expiry_bits = [M31::from(0u32); 16];
    for b in 0..16 {
        expiry_bits[b] = M31::from((expiry_diff_val >> b) & 1);
    }

    let owner_hash_cols =
        poseidon2_air::gen_hash2_intermediates(sk, M31::from(0u32), poseidon2::DOMAIN_OWNER);
    let issuer_id_hash_cols = poseidon2_air::gen_hash2_intermediates(
        M31::from(witness.cred_issuer),
        M31::from(0u32),
        poseidon2::DOMAIN_ISSUER_ID,
    );
    let cm_sponge_inputs: Vec<M31> = vec![
        issuer_id[0],
        issuer_id[1],
        issuer_id[2],
        issuer_id[3],
        owner[0],
        owner[1],
        owner[2],
        owner[3],
        M31::from(witness.cred_expiry),
        M31::from(witness.cred_secret),
    ];
    let cm_hash_cols = poseidon2_air::gen_sponge_2block_intermediates(
        &cm_sponge_inputs,
        poseidon2::DOMAIN_CRED_CM,
    );
    let crednull_hash_cols = poseidon2_air::gen_hash_many_7_intermediates(
        M31::from(witness.cred_secret),
        cred_cm[0],
        cred_cm[1],
        cred_cm[2],
        cred_cm[3],
        M31::from(witness.epoch),
        M31::from(0u32),
        poseidon2::DOMAIN_CRED_NULL,
    );
    let merkle_data = gen_cred_merkle_trace(cred_cm, &witness.cred_path);
    let cred_root_arr = poseidon2::hashout_to_u32_array(cred_root);
    let cred_null_arr = poseidon2::hashout_to_u32_array(cred_null);

    for r in 0..num_rows {
        let mut c = 0usize;
        cols[c].set(r, M31::from(witness.window_start));
        c += 1;
        cols[c].set(r, M31::from(witness.window_end));
        c += 1;
        for k in 0..NUM_LIMBS {
            cols[c].set(r, M31::from(ct_limbs[k]));
            c += 1;
        }
        for j in 0..4 {
            cols[c].set(r, M31::from(cred_root_arr[j]));
            c += 1;
        }
        for j in 0..4 {
            cols[c].set(r, M31::from(cred_null_arr[j]));
            c += 1;
        }
        cols[c].set(r, M31::from(witness.epoch));
        c += 1;
        cols[c].set(r, sk);
        c += 1;
        cols[c].set(r, M31::from(witness.cred_issuer));
        c += 1;
        cols[c].set(r, M31::from(witness.cred_expiry));
        c += 1;
        cols[c].set(r, M31::from(witness.cred_secret));
        c += 1;
        for i in 0..MAX_TX {
            for k in 0..NUM_LIMBS {
                cols[c].set(r, M31::from(tx_limbs[i][k]));
                c += 1;
            }
        }
        for i in 0..MAX_TX {
            cols[c + i].set(r, M31::from(witness.tx_timestamps[i]));
        }
        c += MAX_TX;
        cols[c].set(r, M31::from(expiry_diff_val));
        c += 1;
        for b in 0..16 {
            cols[c + b].set(r, expiry_bits[b]);
        }
        c += 16;
        for k in 0..NUM_CARRIES {
            for b in 0..SUM_CARRY_BITS {
                cols[c].set(r, M31::from((carry_vals[k] >> b) & 1));
                c += 1;
            }
        }
        debug_assert_eq!(c, TX_COLS_START);

        for i in 0..MAX_TX {
            let base = TX_COLS_START + i * COLS_PER_TX;
            let in_window = if in_window_flags[i] { 1u32 } else { 0u32 };
            cols[base].set(r, M31::from(in_window));
            for k in 0..NUM_LIMBS {
                cols[base + 1 + k].set(r, M31::from(in_window * tx_limbs[i][k]));
            }
            let ts_offset = base + 1 + NUM_LIMBS;
            let ts = witness.tx_timestamps[i];
            let ts_lower = if in_window == 1 { ts - witness.window_start } else { 0 };
            let ts_upper = if in_window == 1 { witness.window_end - ts } else { 0 };
            cols[ts_offset].set(r, M31::from(ts_lower));
            for b in 0..RANGE_BITS {
                cols[ts_offset + 1 + b].set(r, M31::from((ts_lower >> b) & 1));
            }
            cols[ts_offset + 1 + RANGE_BITS].set(r, M31::from(ts_upper));
            for b in 0..RANGE_BITS {
                cols[ts_offset + 1 + RANGE_BITS + 1 + b].set(r, M31::from((ts_upper >> b) & 1));
            }
        }

        let mut h = HASH_COLS_START;
        for j in 0..owner_hash_cols.len() {
            cols[h + j].set(r, owner_hash_cols[j]);
        }
        h += owner_hash_cols.len();
        for j in 0..issuer_id_hash_cols.len() {
            cols[h + j].set(r, issuer_id_hash_cols[j]);
        }
        h += issuer_id_hash_cols.len();
        for j in 0..cm_hash_cols.len() {
            cols[h + j].set(r, cm_hash_cols[j]);
        }
        h += cm_hash_cols.len();
        for j in 0..crednull_hash_cols.len() {
            cols[h + j].set(r, crednull_hash_cols[j]);
        }
        for j in 0..merkle_data.len() {
            cols[MERKLE_START + j].set(r, merkle_data[j]);
        }
    }

    let domain = CanonicCoset::new(log_num_rows).circle_domain();
    let trace: ColumnVec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>> =
        cols.into_iter().map(|col| CircleEvaluation::new(domain, col)).collect();

    let public_data = TimeWindowPublicData {
        window_start: witness.window_start,
        window_end: witness.window_end,
        claimed_total: witness.claimed_total,
        cred_root: witness.cred_root,
        cred_null: cred_null_arr,
        epoch: witness.epoch,
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

    let component = TimeWindowComponent::new(
        &mut TraceLocationAllocator::default(),
        TimeWindowEval { log_size: log_num_rows },
        QM31::zero(),
    );
    let proof = prove(&[&component], channel, commitment_scheme)
        .map_err(|e| format!("Time-window proof generation failed: {e:?}"))?;

    Ok(AuditProofResult { proof, component, public_data, log_num_rows })
}

/// Verify a time-window audit proof independently.
pub fn verify_time_window(result: &AuditProofResult) -> Result<(), String> {
    let config = pcs_config();
    let channel = &mut ProverChannel::default();
    let commitment_scheme = &mut CommitmentSchemeVerifier::<ProverMerkleChannel>::new(config);
    let sizes = result.component.trace_log_degree_bounds();
    commitment_scheme.commit(result.proof.commitments[0], &sizes[0], channel);
    channel.mix_u64(result.log_num_rows as u64);
    result.public_data.mix_into(channel);
    commitment_scheme.commit(result.proof.commitments[1], &sizes[1], channel);
    verify(&[&result.component], channel, commitment_scheme, result.proof.clone())
        .map_err(|e| format!("Time-window verification failed: {e:?}"))
}

fn gen_cred_merkle_trace(
    leaf: poseidon2::HashOut,
    path: &[([u32; 4], u32); MERKLE_DEPTH],
) -> Vec<M31> {
    let mut result = Vec::with_capacity(MERKLE_DEPTH * MERKLE_LEVEL_COLS);
    let mut current = leaf;
    for &(sibling_arr, direction_val) in path.iter() {
        let sibling = poseidon2::u32_array_to_hashout(sibling_arr);
        let (left, right) =
            if direction_val == 0 { (current, sibling) } else { (sibling, current) };
        for j in 0..4 {
            result.push(sibling[j]);
        }
        result.push(M31::from(direction_val));
        for j in 0..4 {
            result.push(left[j]);
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

    fn valid_witness() -> TimeWindowWitness {
        let sk = M31::from(12345u32);
        let owner = poseidon2::derive_owner(sk);
        let issuer_id = poseidon2::derive_issuer_id(M31::from(1u32));
        let cred_cm = poseidon2::credential_commitment(
            issuer_id,
            owner,
            M31::from(2000u32),
            M31::from(777u32),
        );
        let mut cred_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
        cred_tree.set_leaf(0, cred_cm);
        let cred_root = cred_tree.root();
        let path_vec = cred_tree.path(0);
        let mut cred_path = [([0u32; 4], 0u32); MERKLE_DEPTH];
        for i in 0..MERKLE_DEPTH {
            cred_path[i] = (poseidon2::hashout_to_u32_array(path_vec[i].0), path_vec[i].1);
        }

        let mut amounts = [0u64; MAX_TX];
        let mut timestamps = [0u32; MAX_TX];
        amounts[0] = 50000;
        timestamps[0] = 100;
        amounts[1] = 30000;
        timestamps[1] = 200;
        amounts[2] = 20000;
        timestamps[2] = 300;
        amounts[3] = 10000;
        timestamps[3] = 400;

        TimeWindowWitness {
            window_start: 50,
            window_end: 500,
            claimed_total: 110000,
            cred_root: poseidon2::hashout_to_u32_array(cred_root),
            epoch: 1000,
            tx_amounts: amounts,
            tx_timestamps: timestamps,
            tx_count: 4,
            sk: 12345,
            cred_issuer: 1,
            cred_expiry: 2000,
            cred_secret: 777,
            cred_path,
        }
    }

    #[test]
    fn test_time_window_roundtrip() {
        let witness = valid_witness();
        let result = prove_time_window(&witness).expect("Prove should succeed");
        verify_time_window(&result).expect("Verify should succeed");
    }

    #[test]
    fn test_mismatched_total() {
        let mut witness = valid_witness();
        witness.claimed_total = 999999;
        assert!(prove_time_window(&witness).is_err());
    }

    #[test]
    fn test_invalid_window() {
        let mut witness = valid_witness();
        witness.window_start = witness.window_end + 1;
        assert!(prove_time_window(&witness).is_err());
    }

    #[test]
    fn test_large_u64_amounts() {
        let mut witness = valid_witness();
        witness.tx_amounts[0] = 5_000_000_000;
        witness.tx_amounts[1] = 3_000_000_000;
        witness.tx_amounts[2] = 2_000_000_000;
        witness.tx_amounts[3] = 500_000_000;
        witness.claimed_total = 10_500_000_000;
        let result = prove_time_window(&witness).expect("Prove should succeed");
        verify_time_window(&result).expect("Verify should succeed");
    }
}
