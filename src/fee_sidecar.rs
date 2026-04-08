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
    payment_tx::{derive_sender_binding_tag, AssetId},
    poseidon2, poseidon2_air,
    prover_common::{pcs_config, ProverChannel, ProverMerkleChannel, ProverMerkleHasher},
    types::{
        amount_to_limbs, HushFeeWitness, CARRY_BIAS, CARRY_BITS, LIMB_BITS, MERKLE_DEPTH,
        NUM_CARRIES, NUM_LIMBS,
    },
};

const LOG_CONSTRAINT_EVAL_BLOWUP_FACTOR: u32 = 1;
const MERKLE_LEVEL_COLS: usize = 3 + poseidon2_air::HASH_INTERMEDIATE_COLS;

// 4 amounts x 4 limbs = 16 limbs, each range-checked to 15 bits
const FEE_NUM_AMOUNTS: usize = 4;
const FEE_LIMB_RANGE_COLS: usize = FEE_NUM_AMOUNTS * NUM_LIMBS * LIMB_BITS; // 240

// 34 base/aux + 240 limb range + 6 hashes + 2 Merkle paths
// Base: 28 witness + 6 carry bits (3 carries x 2 bits)
const FEE_BASE_AUX_COLS: usize = 28 + NUM_CARRIES * CARRY_BITS;
const NUM_HASHES: usize = 6;
const NUM_COLS: usize = FEE_BASE_AUX_COLS
    + FEE_LIMB_RANGE_COLS
    + NUM_HASHES * poseidon2_air::HASH_INTERMEDIATE_COLS
    + 2 * MERKLE_DEPTH * MERKLE_LEVEL_COLS;

fn constrain_merkle_path<E: EvalAtRow>(eval: &mut E, leaf: E::F, pub_root: E::F) {
    let mut current = leaf;
    for _ in 0..MERKLE_DEPTH {
        let sibling = eval.next_trace_mask();
        let direction = eval.next_trace_mask();
        let left = eval.next_trace_mask();

        eval.add_constraint(direction.clone() * (direction.clone() - E::F::one()));
        eval.add_constraint(
            left.clone() - current.clone() - direction * (sibling.clone() - current.clone()),
        );
        let right = current + sibling - left.clone();
        current = poseidon2_air::constrain_hash2(eval, left, right, poseidon2::DOMAIN_MERKLE);
    }
    eval.add_constraint(current - pub_root);
}

#[derive(Clone)]
pub struct HushFeeSidecarEval {
    pub log_size: u32,
}

impl FrameworkEval for HushFeeSidecarEval {
    fn log_size(&self) -> u32 {
        self.log_size
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.log_size + LOG_CONSTRAINT_EVAL_BLOWUP_FACTOR
    }

    fn evaluate<E: EvalAtRow>(&self, mut eval: E) -> E {
        let sk = eval.next_trace_mask();
        let owner = eval.next_trace_mask();

        // Four-limb amounts: 4 amounts x 4 limbs = 16 limb columns
        let in_amt_0: [E::F; NUM_LIMBS] = core::array::from_fn(|_| eval.next_trace_mask());
        let in_rand_0 = eval.next_trace_mask();
        let in_amt_1: [E::F; NUM_LIMBS] = core::array::from_fn(|_| eval.next_trace_mask());
        let in_rand_1 = eval.next_trace_mask();
        let in_cm_0 = eval.next_trace_mask();
        let in_cm_1 = eval.next_trace_mask();
        let null_0 = eval.next_trace_mask();
        let null_1 = eval.next_trace_mask();
        let change_amt: [E::F; NUM_LIMBS] = core::array::from_fn(|_| eval.next_trace_mask());
        let change_rand = eval.next_trace_mask();
        let fee_limbs: [E::F; NUM_LIMBS] = core::array::from_fn(|_| eval.next_trace_mask());
        let change_cm = eval.next_trace_mask();
        let pub_note_root = eval.next_trace_mask();

        // Nullifier inequality via multiplicative inverse
        let null_diff_inv = eval.next_trace_mask();
        eval.add_constraint((null_0.clone() - null_1.clone()) * null_diff_inv - E::F::one());

        let two = E::F::one() + E::F::one();

        // Carry columns for limb-by-limb balance conservation.
        // Conservation: in0 + in1 = change + fee
        // Carries are in [-2, 1]. Biased carry = carry + CARRY_BIAS is in [0, 3] (2-bit).
        let carry_bias = E::F::from(M31::from(CARRY_BIAS));
        let radix = E::F::from(M31::from(crate::types::RADIX as u32));
        let mut carries: [E::F; NUM_CARRIES] = core::array::from_fn(|_| E::F::zero());
        for k in 0..NUM_CARRIES {
            let c_b0 = eval.next_trace_mask();
            let c_b1 = eval.next_trace_mask();
            eval.add_constraint(c_b0.clone() * (c_b0.clone() - E::F::one()));
            eval.add_constraint(c_b1.clone() * (c_b1.clone() - E::F::one()));
            carries[k] = c_b0 + two.clone() * c_b1 - carry_bias.clone();
        }

        // Limb-by-limb balance conservation:
        // For k = 0..3: in0[k] + in1[k] + c_prev - change[k] - fee[k] - c_k * R = 0
        for k in 0..NUM_LIMBS {
            let c_prev = if k == 0 { E::F::zero() } else { carries[k - 1].clone() };
            let lhs = in_amt_0[k].clone()
                + in_amt_1[k].clone()
                + c_prev
                - change_amt[k].clone()
                - fee_limbs[k].clone();
            if k < NUM_CARRIES {
                eval.add_constraint(lhs - carries[k].clone() * radix.clone());
            } else {
                eval.add_constraint(lhs);
            }
        }

        // Limb range checks: each of 16 limbs must fit in LIMB_BITS bits
        let all_limbs: [&[E::F; NUM_LIMBS]; FEE_NUM_AMOUNTS] =
            [&in_amt_0, &in_amt_1, &change_amt, &fee_limbs];
        for limbs in all_limbs {
            for limb in limbs {
                let mut recon = E::F::zero();
                let mut p2 = E::F::one();
                for _ in 0..LIMB_BITS {
                    let bit = eval.next_trace_mask();
                    eval.add_constraint(bit.clone() * (bit.clone() - E::F::one()));
                    recon += bit * p2.clone();
                    p2 *= two.clone();
                }
                eval.add_constraint(recon - limb.clone());
            }
        }

        let owner_out = poseidon2_air::constrain_hash2(
            &mut eval,
            sk.clone(),
            E::F::zero(),
            poseidon2::DOMAIN_OWNER,
        );
        eval.add_constraint(owner - owner_out.clone());

        let null0_out = poseidon2_air::constrain_hash2(
            &mut eval,
            sk.clone(),
            in_cm_0.clone(),
            poseidon2::DOMAIN_NULLIFIER,
        );
        eval.add_constraint(null_0 - null0_out);

        let null1_out = poseidon2_air::constrain_hash2(
            &mut eval,
            sk,
            in_cm_1.clone(),
            poseidon2::DOMAIN_NULLIFIER,
        );
        eval.add_constraint(null_1 - null1_out);

        let hush_asset = E::F::from(M31::from(AssetId::Hush as u32));

        // Note commitments with 4-limb amounts: H(asset, L0, L1, L2, L3, owner, randomness)
        let cm0_out = poseidon2_air::constrain_hash_many_7(
            &mut eval,
            hush_asset.clone(),
            in_amt_0[0].clone(),
            in_amt_0[1].clone(),
            in_amt_0[2].clone(),
            in_amt_0[3].clone(),
            owner_out.clone(),
            in_rand_0,
            poseidon2::DOMAIN_NOTE_CM,
        );
        eval.add_constraint(in_cm_0.clone() - cm0_out);

        let cm1_out = poseidon2_air::constrain_hash_many_7(
            &mut eval,
            hush_asset.clone(),
            in_amt_1[0].clone(),
            in_amt_1[1].clone(),
            in_amt_1[2].clone(),
            in_amt_1[3].clone(),
            owner_out.clone(),
            in_rand_1,
            poseidon2::DOMAIN_NOTE_CM,
        );
        eval.add_constraint(in_cm_1.clone() - cm1_out);

        let change_cm_out = poseidon2_air::constrain_hash_many_7(
            &mut eval,
            hush_asset,
            change_amt[0].clone(),
            change_amt[1].clone(),
            change_amt[2].clone(),
            change_amt[3].clone(),
            owner_out,
            change_rand,
            poseidon2::DOMAIN_NOTE_CM,
        );
        eval.add_constraint(change_cm - change_cm_out);

        constrain_merkle_path(&mut eval, in_cm_0, pub_note_root.clone());
        constrain_merkle_path(&mut eval, in_cm_1, pub_note_root);

        eval
    }
}

pub type HushFeeSidecarComponent = FrameworkComponent<HushFeeSidecarEval>;

pub struct HushFeePublicData {
    pub note_root: u32,
    pub tx_binding_hash: u32,
    pub sender_binding_tag: u32,
    pub fee_amount: u64,
    pub null_0: u32,
    pub null_1: u32,
    pub change_cm: u32,
}

impl HushFeePublicData {
    pub fn mix_into(&self, channel: &mut impl Channel) {
        channel.mix_u64(self.note_root as u64);
        channel.mix_u64(self.tx_binding_hash as u64);
        channel.mix_u64(self.sender_binding_tag as u64);
        channel.mix_u64(self.fee_amount as u64);
        channel.mix_u64(self.null_0 as u64);
        channel.mix_u64(self.null_1 as u64);
        channel.mix_u64(self.change_cm as u64);
    }
}

pub struct ProofResult {
    pub proof: stwo::core::proof::StarkProof<ProverMerkleHasher>,
    pub component: HushFeeSidecarComponent,
    pub public_data: HushFeePublicData,
    pub log_num_rows: u32,
}

fn gen_merkle_path_trace(leaf: M31, path: &[(u32, u32); MERKLE_DEPTH]) -> Vec<M31> {
    let mut result = Vec::with_capacity(MERKLE_DEPTH * MERKLE_LEVEL_COLS);
    let mut current = leaf;

    for &(sibling_val, direction_val) in path.iter() {
        let sibling = M31::from(sibling_val);
        let direction = M31::from(direction_val);
        let (left, right) =
            if direction_val == 0 { (current, sibling) } else { (sibling, current) };
        result.push(sibling);
        result.push(direction);
        result.push(left);
        let hash_cols =
            poseidon2_air::gen_hash2_intermediates(left, right, poseidon2::DOMAIN_MERKLE);
        result.extend_from_slice(&hash_cols);
        current = poseidon2::merkle_hash(left, right);
    }

    result
}

/// Decompose u64 amounts into limbs and compute carries for HUSH fee balance conservation.
/// Conservation: in0 + in1 = change + fee
fn compute_fee_carries(witness: &HushFeeWitness) -> [i32; NUM_CARRIES] {
    let in0 = amount_to_limbs(witness.in_amt_0);
    let in1 = amount_to_limbs(witness.in_amt_1);
    let ch = amount_to_limbs(witness.change_amt);
    let fee = amount_to_limbs(witness.fee_amount);

    let mut carries = [0i32; NUM_CARRIES];
    let mut c_prev = 0i32;
    for k in 0..NUM_LIMBS {
        let delta = i32::from(in0[k] as i16) + i32::from(in1[k] as i16) + c_prev
            - i32::from(ch[k] as i16) - i32::from(fee[k] as i16);
        if k < NUM_CARRIES {
            debug_assert_eq!(delta % (crate::types::RADIX as i32), 0, "carry not exact at limb {k}");
            carries[k] = delta / (crate::types::RADIX as i32);
            c_prev = carries[k];
        } else {
            debug_assert_eq!(delta, 0, "top limb conservation failed");
        }
    }
    carries
}

fn gen_trace(
    witness: &HushFeeWitness,
    log_num_rows: u32,
) -> ColumnVec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>> {
    let num_rows = 1 << log_num_rows;
    let mut cols: Vec<BaseColumn> = (0..NUM_COLS).map(|_| BaseColumn::zeros(num_rows)).collect();

    let sk = M31::from(witness.sk);
    let owner = poseidon2::derive_owner(sk);
    let hush_asset = M31::from(AssetId::Hush as u32);
    let in_rand_0 = M31::from(witness.in_rand_0);
    let in_rand_1 = M31::from(witness.in_rand_1);
    let change_rand = M31::from(witness.change_rand);

    // Decompose amounts into 4 limbs each
    let in0_limbs = amount_to_limbs(witness.in_amt_0);
    let in1_limbs = amount_to_limbs(witness.in_amt_1);
    let ch_limbs = amount_to_limbs(witness.change_amt);
    let fee_limbs = amount_to_limbs(witness.fee_amount);

    let in0_m31: [M31; NUM_LIMBS] = core::array::from_fn(|i| M31::from(in0_limbs[i]));
    let in1_m31: [M31; NUM_LIMBS] = core::array::from_fn(|i| M31::from(in1_limbs[i]));
    let ch_m31: [M31; NUM_LIMBS] = core::array::from_fn(|i| M31::from(ch_limbs[i]));
    let fee_m31: [M31; NUM_LIMBS] = core::array::from_fn(|i| M31::from(fee_limbs[i]));

    // Note commitments with 7 inputs: (asset, L0, L1, L2, L3, owner, randomness)
    let in_cm_0 = poseidon2::note_commitment(
        hush_asset, in0_m31[0], in0_m31[1], in0_m31[2], in0_m31[3], owner, in_rand_0,
    );
    let in_cm_1 = poseidon2::note_commitment(
        hush_asset, in1_m31[0], in1_m31[1], in1_m31[2], in1_m31[3], owner, in_rand_1,
    );
    let null_0 = poseidon2::nullifier(sk, in_cm_0);
    let null_1 = poseidon2::nullifier(sk, in_cm_1);
    let change_cm = poseidon2::note_commitment(
        hush_asset, ch_m31[0], ch_m31[1], ch_m31[2], ch_m31[3], owner, change_rand,
    );
    let pub_note_root = M31::from(witness.note_root);
    let null_diff = null_0 - null_1;
    let null_diff_inv =
        if null_diff == M31::from(0u32) { M31::from(0u32) } else { null_diff.inverse() };

    // Compute carries for balance conservation
    let carries = compute_fee_carries(witness);
    let carry_bits: [[M31; CARRY_BITS]; NUM_CARRIES] = core::array::from_fn(|k| {
        let biased = (carries[k] + CARRY_BIAS as i32) as u32;
        core::array::from_fn(|b| M31::from((biased >> b) & 1))
    });

    // Hash intermediates (7-input for note commitments, 2-input for owner/nullifiers)
    let owner_hash_cols =
        poseidon2_air::gen_hash2_intermediates(sk, M31::from(0u32), poseidon2::DOMAIN_OWNER);
    let null0_hash_cols =
        poseidon2_air::gen_hash2_intermediates(sk, in_cm_0, poseidon2::DOMAIN_NULLIFIER);
    let null1_hash_cols =
        poseidon2_air::gen_hash2_intermediates(sk, in_cm_1, poseidon2::DOMAIN_NULLIFIER);
    let cm0_hash_cols = poseidon2_air::gen_hash_many_7_intermediates(
        hush_asset, in0_m31[0], in0_m31[1], in0_m31[2], in0_m31[3], owner, in_rand_0,
        poseidon2::DOMAIN_NOTE_CM,
    );
    let cm1_hash_cols = poseidon2_air::gen_hash_many_7_intermediates(
        hush_asset, in1_m31[0], in1_m31[1], in1_m31[2], in1_m31[3], owner, in_rand_1,
        poseidon2::DOMAIN_NOTE_CM,
    );
    let change_hash_cols = poseidon2_air::gen_hash_many_7_intermediates(
        hush_asset, ch_m31[0], ch_m31[1], ch_m31[2], ch_m31[3], owner, change_rand,
        poseidon2::DOMAIN_NOTE_CM,
    );
    let note_path_0_data = gen_merkle_path_trace(in_cm_0, &witness.note_path_0);
    let note_path_1_data = gen_merkle_path_trace(in_cm_1, &witness.note_path_1);

    for r in 0..num_rows {
        let mut col = 0usize;
        let mut set = |c: &mut usize, val: M31| { cols[*c].set(r, val); *c += 1; };

        set(&mut col, sk);           // 0
        set(&mut col, owner);        // 1
        for &v in &in0_m31 { set(&mut col, v); }  // 2-5
        set(&mut col, in_rand_0);    // 6
        for &v in &in1_m31 { set(&mut col, v); }  // 7-10
        set(&mut col, in_rand_1);    // 11
        set(&mut col, in_cm_0);      // 12
        set(&mut col, in_cm_1);      // 13
        set(&mut col, null_0);       // 14
        set(&mut col, null_1);       // 15
        for &v in &ch_m31 { set(&mut col, v); }   // 16-19
        set(&mut col, change_rand);  // 20
        for &v in &fee_m31 { set(&mut col, v); }   // 21-24
        set(&mut col, change_cm);    // 25
        set(&mut col, pub_note_root);// 26
        set(&mut col, null_diff_inv);// 27
        // Carry bits
        for k in 0..NUM_CARRIES {
            for b in 0..CARRY_BITS {
                set(&mut col, carry_bits[k][b]);
            }
        } // 28-33
        assert_eq!(col, FEE_BASE_AUX_COLS);

        // Limb range decomposition: 4 amounts x 4 limbs x 15 bits
        let all_limb_vals = [in0_limbs, in1_limbs, ch_limbs, fee_limbs];
        for limbs in &all_limb_vals {
            for &lv in limbs {
                for b in 0..LIMB_BITS {
                    cols[col].set(r, M31::from((lv >> b) & 1));
                    col += 1;
                }
            }
        }
        assert_eq!(col, FEE_BASE_AUX_COLS + FEE_LIMB_RANGE_COLS);

        let h = poseidon2_air::HASH_INTERMEDIATE_COLS;
        let all_hashes: [&Vec<M31>; NUM_HASHES] = [
            &owner_hash_cols,
            &null0_hash_cols,
            &null1_hash_cols,
            &cm0_hash_cols,
            &cm1_hash_cols,
            &change_hash_cols,
        ];
        for hash_cols in &all_hashes {
            for i in 0..h {
                cols[col + i].set(r, hash_cols[i]);
            }
            col += h;
        }

        let path_cols = MERKLE_DEPTH * MERKLE_LEVEL_COLS;
        let all_paths: [&Vec<M31>; 2] = [&note_path_0_data, &note_path_1_data];
        for path_data in &all_paths {
            for i in 0..path_cols {
                cols[col + i].set(r, path_data[i]);
            }
            col += path_cols;
        }
        assert_eq!(col, NUM_COLS);
    }

    let domain = CanonicCoset::new(log_num_rows).circle_domain();
    cols.into_iter().map(|col| CircleEvaluation::new(domain, col)).collect()
}

fn validate_witness(witness: &HushFeeWitness) -> Result<HushFeePublicData, String> {
    let total_in = witness.in_amt_0.checked_add(witness.in_amt_1)
        .ok_or_else(|| "HUSH fee input amount overflow".to_string())?;
    let total_out = witness.change_amt.checked_add(witness.fee_amount)
        .ok_or_else(|| "HUSH fee output amount overflow".to_string())?;
    if total_in != total_out {
        return Err(format!(
            "HUSH fee balance conservation failed: inputs {total_in} != change+fee {total_out}"
        ));
    }

    let expected_sender_binding_tag = derive_sender_binding_tag(witness.sk, witness.tx_binding_hash);
    if witness.sender_binding_tag != expected_sender_binding_tag {
        return Err(format!(
            "sender_binding_tag mismatch: witness {}, expected {}",
            witness.sender_binding_tag, expected_sender_binding_tag
        ));
    }

    let sk = M31::from(witness.sk);
    let owner = poseidon2::derive_owner(sk);
    let hush_asset = M31::from(AssetId::Hush as u32);
    let in_cm_0 = poseidon2::note_commitment_u64(
        hush_asset, witness.in_amt_0, owner, M31::from(witness.in_rand_0),
    );
    let in_cm_1 = poseidon2::note_commitment_u64(
        hush_asset, witness.in_amt_1, owner, M31::from(witness.in_rand_1),
    );

    let note_root = M31::from(witness.note_root);
    let note_path_0: Vec<(M31, u32)> =
        witness.note_path_0.iter().map(|&(s, d)| (M31::from(s), d)).collect();
    let note_path_1: Vec<(M31, u32)> =
        witness.note_path_1.iter().map(|&(s, d)| (M31::from(s), d)).collect();
    if !poseidon2::verify_merkle_path(in_cm_0, &note_path_0, note_root) {
        return Err("HUSH sidecar note Merkle path for input 0 is invalid".to_string());
    }
    if !poseidon2::verify_merkle_path(in_cm_1, &note_path_1, note_root) {
        return Err("HUSH sidecar note Merkle path for input 1 is invalid".to_string());
    }

    let null_0 = poseidon2::nullifier(sk, in_cm_0);
    let null_1 = poseidon2::nullifier(sk, in_cm_1);
    let change_cm = poseidon2::note_commitment_u64(
        hush_asset, witness.change_amt, owner, M31::from(witness.change_rand),
    );

    Ok(HushFeePublicData {
        note_root: witness.note_root,
        tx_binding_hash: witness.tx_binding_hash,
        sender_binding_tag: witness.sender_binding_tag,
        fee_amount: witness.fee_amount,
        null_0: null_0.0,
        null_1: null_1.0,
        change_cm: change_cm.0,
    })
}

pub fn prove_hush_fee(witness: &HushFeeWitness) -> Result<ProofResult, String> {
    let log_num_rows = LOG_N_LANES;
    let public_data = validate_witness(witness)?;
    let trace = gen_trace(witness, log_num_rows);

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

    let component = HushFeeSidecarComponent::new(
        &mut TraceLocationAllocator::default(),
        HushFeeSidecarEval { log_size: log_num_rows },
        QM31::zero(),
    );

    let proof = prove(&[&component], channel, commitment_scheme)
        .map_err(|e| format!("HUSH fee proof generation failed: {e:?}"))?;

    Ok(ProofResult { proof, component, public_data, log_num_rows })
}

pub fn verify_hush_fee(result: &ProofResult) -> Result<(), String> {
    let config = pcs_config();
    let channel = &mut ProverChannel::default();
    let commitment_scheme = &mut CommitmentSchemeVerifier::<ProverMerkleChannel>::new(config);
    let sizes = result.component.trace_log_degree_bounds();

    commitment_scheme.commit(result.proof.commitments[0], &sizes[0], channel);
    channel.mix_u64(result.log_num_rows as u64);
    result.public_data.mix_into(channel);
    commitment_scheme.commit(result.proof.commitments[1], &sizes[1], channel);

    verify(&[&result.component], channel, commitment_scheme, result.proof.clone())
        .map_err(|e| format!("HUSH fee verification failed: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payment_fixtures::{
        invalid_hush_change_fixture, insufficient_hush_fee_coverage_fixture,
        valid_usdc_hush_fee_fixture, valid_usdt_hush_fee_fixture,
        wrong_sender_binding_tag_hush_fee_fixture, wrong_tx_binding_hash_hush_fee_fixture,
    };

    #[test]
    fn test_hush_fee_roundtrip_usdc_mode_b() {
        let fixture = valid_usdc_hush_fee_fixture();
        let witness = fixture.fee_sidecar_witness.expect("Mode B fixture should include sidecar");
        let result = prove_hush_fee(&witness).expect("Mode B HUSH sidecar proof should succeed");
        verify_hush_fee(&result).expect("Mode B HUSH sidecar verification should succeed");
        assert_eq!(result.public_data.tx_binding_hash, fixture.tx.tx_binding_hash);
        assert_eq!(result.public_data.sender_binding_tag, fixture.sender_binding_tag);
    }

    #[test]
    fn test_hush_fee_roundtrip_usdt_mode_b() {
        let fixture = valid_usdt_hush_fee_fixture();
        let witness = fixture.fee_sidecar_witness.expect("Mode B fixture should include sidecar");
        let result = prove_hush_fee(&witness).expect("Mode B HUSH sidecar proof should succeed");
        verify_hush_fee(&result).expect("Mode B HUSH sidecar verification should succeed");
    }

    #[test]
    fn test_insufficient_hush_fee_coverage_rejected() {
        let fixture = insufficient_hush_fee_coverage_fixture();
        let witness = fixture.fee_sidecar_witness.expect("invalid fixture should include sidecar");
        assert!(prove_hush_fee(&witness).is_err());
    }

    #[test]
    fn test_invalid_hush_change_rejected() {
        let fixture = invalid_hush_change_fixture();
        let witness = fixture.fee_sidecar_witness.expect("invalid fixture should include sidecar");
        assert!(prove_hush_fee(&witness).is_err());
    }

    #[test]
    fn test_wrong_sender_binding_tag_rejected() {
        let fixture = wrong_sender_binding_tag_hush_fee_fixture();
        let witness = fixture.fee_sidecar_witness.expect("invalid fixture should include sidecar");
        assert!(prove_hush_fee(&witness).is_err());
    }

    #[test]
    fn test_wrong_tx_binding_hash_rejected() {
        let fixture = wrong_tx_binding_hash_hush_fee_fixture();
        let witness = fixture.fee_sidecar_witness.expect("invalid fixture should include sidecar");
        assert!(prove_hush_fee(&witness).is_err());
    }
}
