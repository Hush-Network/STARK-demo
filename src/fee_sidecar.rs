use num_traits::{One, Zero};
use poseidon2::HashOut;
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

// Merkle path: sibling(4) + direction(1) + left(4) + hash_pair intermediates
const MERKLE_LEVEL_COLS: usize = 9 + poseidon2_air::HASH_INTERMEDIATE_COLS;

// 4 amounts x 4 limbs = 16 limbs, each range-checked to 15 bits
const FEE_NUM_AMOUNTS: usize = 4;
const FEE_LIMB_RANGE_COLS: usize = FEE_NUM_AMOUNTS * NUM_LIMBS * LIMB_BITS; // 240

// Base/aux witness columns (HashOut fields expanded to 4 elements each):
// sk(1) + owner(4) + in_amt_0(4) + in_rand_0(1) + in_amt_1(4) + in_rand_1(1)
// + in_cm_0(4) + in_cm_1(4) + null_0(4) + null_1(4) + change_amt(4) + change_rand(1)
// + fee_limbs(4) + change_cm(4) + pub_note_root(4) + null_diff_inv(1)
// + carry_bits(3 x 2 = 6) = 49 + 6 = 55
const FEE_BASE_AUX_COLS: usize = 49 + NUM_CARRIES * CARRY_BITS;

// Hash intermediates:
// 3 single-block hashes (owner, null_0, null_1) + 3 two-block sponges (cm0, cm1, change_cm)
const NUM_SINGLE_BLOCK_HASHES: usize = 3;
const NUM_SPONGE_HASHES: usize = 3;
const NUM_COLS: usize = FEE_BASE_AUX_COLS
    + FEE_LIMB_RANGE_COLS
    + NUM_SINGLE_BLOCK_HASHES * poseidon2_air::HASH_INTERMEDIATE_COLS
    + NUM_SPONGE_HASHES * poseidon2_air::SPONGE_2BLOCK_INTERMEDIATE_COLS
    + 2 * MERKLE_DEPTH * MERKLE_LEVEL_COLS;

fn constrain_merkle_path<E: EvalAtRow>(eval: &mut E, leaf: [E::F; 4], pub_root: [E::F; 4]) {
    let mut current = leaf;
    for _ in 0..MERKLE_DEPTH {
        let sibling: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());
        let direction = eval.next_trace_mask();
        let left: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());

        // direction is boolean
        eval.add_constraint(direction.clone() * (direction.clone() - E::F::one()));

        // left = current + direction * (sibling - current)  (element-wise)
        for i in 0..4 {
            eval.add_constraint(
                left[i].clone()
                    - current[i].clone()
                    - direction.clone() * (sibling[i].clone() - current[i].clone()),
            );
        }

        // right = current + sibling - left  (element-wise)
        let right: [E::F; 4] =
            core::array::from_fn(|i| current[i].clone() + sibling[i].clone() - left[i].clone());

        current = poseidon2_air::constrain_hash_pair(eval, left, right, poseidon2::DOMAIN_MERKLE);
    }

    // Constrain all 4 elements of the root
    for i in 0..4 {
        eval.add_constraint(current[i].clone() - pub_root[i].clone());
    }
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
        let owner: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());

        // Four-limb amounts: 4 amounts x 4 limbs = 16 limb columns
        let in_amt_0: [E::F; NUM_LIMBS] = core::array::from_fn(|_| eval.next_trace_mask());
        let in_rand_0 = eval.next_trace_mask();
        let in_amt_1: [E::F; NUM_LIMBS] = core::array::from_fn(|_| eval.next_trace_mask());
        let in_rand_1 = eval.next_trace_mask();
        let in_cm_0: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());
        let in_cm_1: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());
        let null_0: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());
        let null_1: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());
        let change_amt: [E::F; NUM_LIMBS] = core::array::from_fn(|_| eval.next_trace_mask());
        let change_rand = eval.next_trace_mask();
        let fee_limbs: [E::F; NUM_LIMBS] = core::array::from_fn(|_| eval.next_trace_mask());
        let change_cm: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());
        let pub_note_root: [E::F; 4] = core::array::from_fn(|_| eval.next_trace_mask());

        // Nullifier inequality via linear combination + multiplicative inverse.
        // combined_diff = sum(c^i * (null_0[i] - null_1[i])) for fixed c=7.
        // Two different HashOut values collide with probability <= 3/p, which is negligible.
        let null_diff_inv = eval.next_trace_mask();
        let c = E::F::from(M31::from(7u32));
        let mut combined_diff = null_0[0].clone() - null_1[0].clone();
        let mut ci = c.clone();
        for i in 1..4 {
            combined_diff += ci.clone() * (null_0[i].clone() - null_1[i].clone());
            ci *= c.clone();
        }
        eval.add_constraint(combined_diff * null_diff_inv - E::F::one());

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
            let lhs = in_amt_0[k].clone() + in_amt_1[k].clone() + c_prev
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

        // --- Hash constraints ---

        // Owner derivation: H(sk, 0) with DOMAIN_OWNER -> [E::F; 4]
        let owner_out = poseidon2_air::constrain_hash2(
            &mut eval,
            sk.clone(),
            E::F::zero(),
            poseidon2::DOMAIN_OWNER,
        );
        for i in 0..4 {
            eval.add_constraint(owner[i].clone() - owner_out[i].clone());
        }

        // Nullifier 0: H(sk, cm[0..4]) with DOMAIN_NULLIFIER (5 inputs, single block)
        {
            let mut input: [E::F; poseidon2::WIDTH] = core::array::from_fn(|_| E::F::zero());
            input[0] = sk.clone();
            input[1] = in_cm_0[0].clone();
            input[2] = in_cm_0[1].clone();
            input[3] = in_cm_0[2].clone();
            input[4] = in_cm_0[3].clone();
            input[poseidon2::RATE] = E::F::from(M31::from(poseidon2::DOMAIN_NULLIFIER));
            let out = poseidon2_air::constrain_permutation(&mut eval, input);
            for i in 0..4 {
                eval.add_constraint(null_0[i].clone() - out[i].clone());
            }
        }

        // Nullifier 1: H(sk, cm[0..4]) with DOMAIN_NULLIFIER (5 inputs, single block)
        {
            let mut input: [E::F; poseidon2::WIDTH] = core::array::from_fn(|_| E::F::zero());
            input[0] = sk;
            input[1] = in_cm_1[0].clone();
            input[2] = in_cm_1[1].clone();
            input[3] = in_cm_1[2].clone();
            input[4] = in_cm_1[3].clone();
            input[poseidon2::RATE] = E::F::from(M31::from(poseidon2::DOMAIN_NULLIFIER));
            let out = poseidon2_air::constrain_permutation(&mut eval, input);
            for i in 0..4 {
                eval.add_constraint(null_1[i].clone() - out[i].clone());
            }
        }

        let hush_asset = E::F::from(M31::from(AssetId::Hush as u32));

        // Note commitment 0: sponge(asset, a0-a3, owner[0..4], rand) = 10 inputs, 2 blocks
        let cm0_inputs: Vec<E::F> = vec![
            hush_asset.clone(),
            in_amt_0[0].clone(),
            in_amt_0[1].clone(),
            in_amt_0[2].clone(),
            in_amt_0[3].clone(),
            owner_out[0].clone(),
            owner_out[1].clone(),
            owner_out[2].clone(),
            owner_out[3].clone(),
            in_rand_0,
        ];
        let cm0_out = poseidon2_air::constrain_sponge_2block(
            &mut eval,
            &cm0_inputs,
            poseidon2::DOMAIN_NOTE_CM,
        );
        for i in 0..4 {
            eval.add_constraint(in_cm_0[i].clone() - cm0_out[i].clone());
        }

        // Note commitment 1: sponge(asset, a0-a3, owner[0..4], rand) = 10 inputs, 2 blocks
        let cm1_inputs: Vec<E::F> = vec![
            hush_asset.clone(),
            in_amt_1[0].clone(),
            in_amt_1[1].clone(),
            in_amt_1[2].clone(),
            in_amt_1[3].clone(),
            owner_out[0].clone(),
            owner_out[1].clone(),
            owner_out[2].clone(),
            owner_out[3].clone(),
            in_rand_1,
        ];
        let cm1_out = poseidon2_air::constrain_sponge_2block(
            &mut eval,
            &cm1_inputs,
            poseidon2::DOMAIN_NOTE_CM,
        );
        for i in 0..4 {
            eval.add_constraint(in_cm_1[i].clone() - cm1_out[i].clone());
        }

        // Change commitment: sponge(asset, a0-a3, owner[0..4], rand) = 10 inputs, 2 blocks
        let change_inputs: Vec<E::F> = vec![
            hush_asset,
            change_amt[0].clone(),
            change_amt[1].clone(),
            change_amt[2].clone(),
            change_amt[3].clone(),
            owner_out[0].clone(),
            owner_out[1].clone(),
            owner_out[2].clone(),
            owner_out[3].clone(),
            change_rand,
        ];
        let change_cm_out = poseidon2_air::constrain_sponge_2block(
            &mut eval,
            &change_inputs,
            poseidon2::DOMAIN_NOTE_CM,
        );
        for i in 0..4 {
            eval.add_constraint(change_cm[i].clone() - change_cm_out[i].clone());
        }

        constrain_merkle_path(&mut eval, in_cm_0, pub_note_root.clone());
        constrain_merkle_path(&mut eval, in_cm_1, pub_note_root);

        eval
    }
}

pub type HushFeeSidecarComponent = FrameworkComponent<HushFeeSidecarEval>;

pub struct HushFeePublicData {
    pub note_root: [u32; 4],
    pub tx_binding_hash: [u32; 4],
    pub sender_binding_tag: [u32; 4],
    pub fee_amount: u64,
    pub null_0: [u32; 4],
    pub null_1: [u32; 4],
    pub change_cm: [u32; 4],
}

impl HushFeePublicData {
    pub fn mix_into(&self, channel: &mut impl Channel) {
        for &v in &self.note_root {
            channel.mix_u64(v as u64);
        }
        for &v in &self.tx_binding_hash {
            channel.mix_u64(v as u64);
        }
        for &v in &self.sender_binding_tag {
            channel.mix_u64(v as u64);
        }
        channel.mix_u64(self.fee_amount);
        for &v in &self.null_0 {
            channel.mix_u64(v as u64);
        }
        for &v in &self.null_1 {
            channel.mix_u64(v as u64);
        }
        for &v in &self.change_cm {
            channel.mix_u64(v as u64);
        }
    }
}

pub struct ProofResult {
    pub proof: stwo::core::proof::StarkProof<ProverMerkleHasher>,
    pub component: HushFeeSidecarComponent,
    pub public_data: HushFeePublicData,
    pub log_num_rows: u32,
}

fn gen_merkle_path_trace(leaf: HashOut, path: &[([u32; 4], u32); MERKLE_DEPTH]) -> Vec<M31> {
    let mut result = Vec::with_capacity(MERKLE_DEPTH * MERKLE_LEVEL_COLS);
    let mut current = leaf;

    for &(sibling_arr, direction_val) in path.iter() {
        let sibling = poseidon2::u32_array_to_hashout(sibling_arr);
        let direction = M31::from(direction_val);
        let (left, right) =
            if direction_val == 0 { (current, sibling) } else { (sibling, current) };

        // sibling (4 elements)
        for i in 0..4 {
            result.push(sibling[i]);
        }
        // direction (1 element)
        result.push(direction);
        // left (4 elements)
        for i in 0..4 {
            result.push(left[i]);
        }

        let hash_cols =
            poseidon2_air::gen_hash_pair_intermediates(left, right, poseidon2::DOMAIN_MERKLE);
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
            - i32::from(ch[k] as i16)
            - i32::from(fee[k] as i16);
        if k < NUM_CARRIES {
            debug_assert_eq!(
                delta % (crate::types::RADIX as i32),
                0,
                "carry not exact at limb {k}"
            );
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
    let owner: HashOut = poseidon2::derive_owner(sk);
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

    // HUSH fee notes are always unregulated: attestation_root = all-zeros sentinel.
    let att_root_zero: HashOut = [M31::from(0u32); 4];

    // Note commitments with 14 inputs: (asset, a0-a3, owner[0..4], randomness, ar[0..4])
    let in_cm_0: HashOut = poseidon2::note_commitment(
        hush_asset,
        in0_m31[0],
        in0_m31[1],
        in0_m31[2],
        in0_m31[3],
        owner,
        in_rand_0,
        att_root_zero,
    );
    let in_cm_1: HashOut = poseidon2::note_commitment(
        hush_asset,
        in1_m31[0],
        in1_m31[1],
        in1_m31[2],
        in1_m31[3],
        owner,
        in_rand_1,
        att_root_zero,
    );
    let null_0: HashOut = poseidon2::nullifier(sk, in_cm_0);
    let null_1: HashOut = poseidon2::nullifier(sk, in_cm_1);
    let change_cm: HashOut = poseidon2::note_commitment(
        hush_asset,
        ch_m31[0],
        ch_m31[1],
        ch_m31[2],
        ch_m31[3],
        owner,
        change_rand,
        att_root_zero,
    );
    let pub_note_root: HashOut = poseidon2::u32_array_to_hashout(witness.note_root);

    // Nullifier inequality inverse (linear combination with c=7)
    let c_val = M31::from(7u32);
    let mut combined_diff = null_0[0] - null_1[0];
    let mut ci = c_val;
    for i in 1..4 {
        combined_diff += ci * (null_0[i] - null_1[i]);
        ci *= c_val;
    }
    let null_diff_inv =
        if combined_diff == M31::from(0u32) { M31::from(0u32) } else { combined_diff.inverse() };

    // Compute carries for balance conservation
    let carries = compute_fee_carries(witness);
    let carry_bits: [[M31; CARRY_BITS]; NUM_CARRIES] = core::array::from_fn(|k| {
        let biased = (carries[k] + CARRY_BIAS as i32) as u32;
        core::array::from_fn(|b| M31::from((biased >> b) & 1))
    });

    // Hash intermediates
    // Owner: 2-input single block
    let owner_hash_cols =
        poseidon2_air::gen_hash2_intermediates(sk, M31::from(0u32), poseidon2::DOMAIN_OWNER);

    // Nullifiers: 5-input single block each
    let null0_hash_cols = {
        let mut input = [M31::from(0u32); poseidon2::WIDTH];
        input[0] = sk;
        input[1] = in_cm_0[0];
        input[2] = in_cm_0[1];
        input[3] = in_cm_0[2];
        input[4] = in_cm_0[3];
        input[poseidon2::RATE] = M31::from(poseidon2::DOMAIN_NULLIFIER);
        poseidon2_air::gen_permutation_intermediates(&input)
    };
    let null1_hash_cols = {
        let mut input = [M31::from(0u32); poseidon2::WIDTH];
        input[0] = sk;
        input[1] = in_cm_1[0];
        input[2] = in_cm_1[1];
        input[3] = in_cm_1[2];
        input[4] = in_cm_1[3];
        input[poseidon2::RATE] = M31::from(poseidon2::DOMAIN_NULLIFIER);
        poseidon2_air::gen_permutation_intermediates(&input)
    };

    // Note commitments: 10-input 2-block sponge each
    let cm0_sponge_inputs: Vec<M31> = vec![
        hush_asset, in0_m31[0], in0_m31[1], in0_m31[2], in0_m31[3], owner[0], owner[1], owner[2],
        owner[3], in_rand_0,
    ];
    let cm0_hash_cols = poseidon2_air::gen_sponge_2block_intermediates(
        &cm0_sponge_inputs,
        poseidon2::DOMAIN_NOTE_CM,
    );

    let cm1_sponge_inputs: Vec<M31> = vec![
        hush_asset, in1_m31[0], in1_m31[1], in1_m31[2], in1_m31[3], owner[0], owner[1], owner[2],
        owner[3], in_rand_1,
    ];
    let cm1_hash_cols = poseidon2_air::gen_sponge_2block_intermediates(
        &cm1_sponge_inputs,
        poseidon2::DOMAIN_NOTE_CM,
    );

    let change_sponge_inputs: Vec<M31> = vec![
        hush_asset,
        ch_m31[0],
        ch_m31[1],
        ch_m31[2],
        ch_m31[3],
        owner[0],
        owner[1],
        owner[2],
        owner[3],
        change_rand,
    ];
    let change_hash_cols = poseidon2_air::gen_sponge_2block_intermediates(
        &change_sponge_inputs,
        poseidon2::DOMAIN_NOTE_CM,
    );

    let note_path_0_data = gen_merkle_path_trace(in_cm_0, &witness.note_path_0);
    let note_path_1_data = gen_merkle_path_trace(in_cm_1, &witness.note_path_1);

    for r in 0..num_rows {
        let mut col = 0usize;
        let mut set = |c: &mut usize, val: M31| {
            cols[*c].set(r, val);
            *c += 1;
        };

        set(&mut col, sk);
        for &v in &owner {
            set(&mut col, v);
        }
        for &v in &in0_m31 {
            set(&mut col, v);
        }
        set(&mut col, in_rand_0);
        for &v in &in1_m31 {
            set(&mut col, v);
        }
        set(&mut col, in_rand_1);
        for &v in &in_cm_0 {
            set(&mut col, v);
        }
        for &v in &in_cm_1 {
            set(&mut col, v);
        }
        for &v in &null_0 {
            set(&mut col, v);
        }
        for &v in &null_1 {
            set(&mut col, v);
        }
        for &v in &ch_m31 {
            set(&mut col, v);
        }
        set(&mut col, change_rand);
        for &v in &fee_m31 {
            set(&mut col, v);
        }
        for &v in &change_cm {
            set(&mut col, v);
        }
        for &v in &pub_note_root {
            set(&mut col, v);
        }
        set(&mut col, null_diff_inv);
        // Carry bits
        for k in 0..NUM_CARRIES {
            for b in 0..CARRY_BITS {
                set(&mut col, carry_bits[k][b]);
            }
        }
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

        // Single-block hash intermediates: owner, null_0, null_1
        let h = poseidon2_air::HASH_INTERMEDIATE_COLS;
        let single_block_hashes: [&Vec<M31>; NUM_SINGLE_BLOCK_HASHES] =
            [&owner_hash_cols, &null0_hash_cols, &null1_hash_cols];
        for hash_cols in &single_block_hashes {
            for i in 0..h {
                cols[col + i].set(r, hash_cols[i]);
            }
            col += h;
        }

        // Sponge (2-block) hash intermediates: cm0, cm1, change_cm
        let s = poseidon2_air::SPONGE_2BLOCK_INTERMEDIATE_COLS;
        let sponge_hashes: [&Vec<M31>; NUM_SPONGE_HASHES] =
            [&cm0_hash_cols, &cm1_hash_cols, &change_hash_cols];
        for hash_cols in &sponge_hashes {
            for i in 0..s {
                cols[col + i].set(r, hash_cols[i]);
            }
            col += s;
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
    let total_in = witness
        .in_amt_0
        .checked_add(witness.in_amt_1)
        .ok_or_else(|| "HUSH fee input amount overflow".to_string())?;
    let total_out = witness
        .change_amt
        .checked_add(witness.fee_amount)
        .ok_or_else(|| "HUSH fee output amount overflow".to_string())?;
    if total_in != total_out {
        return Err(format!(
            "HUSH fee balance conservation failed: inputs {total_in} != change+fee {total_out}"
        ));
    }

    let expected_sender_binding_tag =
        derive_sender_binding_tag(witness.sk, witness.tx_binding_hash);
    if witness.sender_binding_tag != expected_sender_binding_tag {
        return Err(format!(
            "sender_binding_tag mismatch: witness {:?}, expected {:?}",
            witness.sender_binding_tag, expected_sender_binding_tag
        ));
    }

    let sk = M31::from(witness.sk);
    let owner: HashOut = poseidon2::derive_owner(sk);
    let hush_asset = M31::from(AssetId::Hush as u32);
    // HUSH fee notes are always unregulated: attestation_root = all-zeros sentinel.
    let att_root_zero: HashOut = [M31::from(0u32); 4];
    let in_cm_0: HashOut = poseidon2::note_commitment_u64(
        hush_asset,
        witness.in_amt_0,
        owner,
        M31::from(witness.in_rand_0),
        att_root_zero,
    );
    let in_cm_1: HashOut = poseidon2::note_commitment_u64(
        hush_asset,
        witness.in_amt_1,
        owner,
        M31::from(witness.in_rand_1),
        att_root_zero,
    );

    let note_root: HashOut = poseidon2::u32_array_to_hashout(witness.note_root);
    let note_path_0: Vec<(HashOut, u32)> =
        witness.note_path_0.iter().map(|&(s, d)| (poseidon2::u32_array_to_hashout(s), d)).collect();
    let note_path_1: Vec<(HashOut, u32)> =
        witness.note_path_1.iter().map(|&(s, d)| (poseidon2::u32_array_to_hashout(s), d)).collect();
    if !poseidon2::verify_merkle_path(in_cm_0, &note_path_0, note_root) {
        return Err("HUSH sidecar note Merkle path for input 0 is invalid".to_string());
    }
    if !poseidon2::verify_merkle_path(in_cm_1, &note_path_1, note_root) {
        return Err("HUSH sidecar note Merkle path for input 1 is invalid".to_string());
    }

    let null_0: HashOut = poseidon2::nullifier(sk, in_cm_0);
    let null_1: HashOut = poseidon2::nullifier(sk, in_cm_1);
    let change_cm: HashOut = poseidon2::note_commitment_u64(
        hush_asset,
        witness.change_amt,
        owner,
        M31::from(witness.change_rand),
        att_root_zero,
    );

    Ok(HushFeePublicData {
        note_root: witness.note_root,
        tx_binding_hash: witness.tx_binding_hash,
        sender_binding_tag: witness.sender_binding_tag,
        fee_amount: witness.fee_amount,
        null_0: poseidon2::hashout_to_u32_array(null_0),
        null_1: poseidon2::hashout_to_u32_array(null_1),
        change_cm: poseidon2::hashout_to_u32_array(change_cm),
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
        insufficient_hush_fee_coverage_fixture, invalid_hush_change_fixture,
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
