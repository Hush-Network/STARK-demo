//! Payment circuit (2-in-2-out, credential-gated).

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
    payment_tx::{
        compute_mode_a_tx_binding_hash, derive_sender_binding_tag,
        PAYMENT_STANDARD_FEE_SCHEDULE_VERSION,
    },
    poseidon2, poseidon2_air,
    prover_common::{pcs_config, ProverChannel, ProverMerkleChannel, ProverMerkleHasher},
    types::{
        amount_to_limbs, PaymentWitness, CARRY_BIAS, CARRY_BITS, LIMB_BITS, MERKLE_DEPTH,
        NUM_CARRIES, NUM_LIMBS, RADIX,
    },
};

const LOG_CONSTRAINT_EVAL_BLOWUP_FACTOR: u32 = 1;
const MERKLE_LEVEL_COLS: usize = 3 + poseidon2_air::HASH_INTERMEDIATE_COLS; // 639

// 5 amounts x 4 limbs = 20 limbs, each range-checked to 15 bits
const NUM_AMOUNTS: usize = 5;
const LIMB_RANGE_COLS: usize = NUM_AMOUNTS * NUM_LIMBS * LIMB_BITS; // 300

// 66 base/aux + 300 limb range + 9 hashes + 3 Merkle paths
// Base: 42 witness + 18 aux (null_diff_inv, expiry_diff, 16 expiry bits) + 6 carry bits
const BASE_AUX_COLS: usize = 42 + 18 + NUM_CARRIES * CARRY_BITS;
const NUM_COLS: usize = BASE_AUX_COLS
    + LIMB_RANGE_COLS
    + 9 * poseidon2_air::HASH_INTERMEDIATE_COLS
    + 3 * MERKLE_DEPTH * MERKLE_LEVEL_COLS;

fn constrain_merkle_path<E: EvalAtRow>(eval: &mut E, leaf: E::F, pub_root: E::F) {
    let mut current = leaf;
    for _ in 0..MERKLE_DEPTH {
        let sibling = eval.next_trace_mask();
        let direction = eval.next_trace_mask();
        let left = eval.next_trace_mask();

        // direction in {0, 1}
        eval.add_constraint(direction.clone() * (direction.clone() - E::F::one()));

        // left = (1-dir)*current + dir*sibling
        eval.add_constraint(
            left.clone() - current.clone() - direction * (sibling.clone() - current.clone()),
        );

        // right = current + sibling - left (degree 1)
        let right = current + sibling - left.clone();

        // hash2(left, right) with DOMAIN_MERKLE
        current = poseidon2_air::constrain_hash2(eval, left, right, poseidon2::DOMAIN_MERKLE);
    }
    eval.add_constraint(current - pub_root);
}

#[derive(Clone)]
// TODO(prod): variable fan-in/fan-out, fee output, multi-asset type enforcement
pub struct HushPaymentEval {
    pub log_size: u32,
}

impl FrameworkEval for HushPaymentEval {
    fn log_size(&self) -> u32 {
        self.log_size
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.log_size + LOG_CONSTRAINT_EVAL_BLOWUP_FACTOR
    }

    fn evaluate<E: EvalAtRow>(&self, mut eval: E) -> E {
        // Base trace columns (order must match gen_trace)
        let sk = eval.next_trace_mask();
        let owner = eval.next_trace_mask();
        let in_asset = eval.next_trace_mask();

        // Four-limb amounts: 5 amounts x 4 limbs = 20 limb columns
        let in_amt_0: [E::F; NUM_LIMBS] = core::array::from_fn(|_| eval.next_trace_mask());
        let in_rand_0 = eval.next_trace_mask();
        let in_amt_1: [E::F; NUM_LIMBS] = core::array::from_fn(|_| eval.next_trace_mask());
        let in_rand_1 = eval.next_trace_mask();
        let in_cm_0 = eval.next_trace_mask();
        let in_cm_1 = eval.next_trace_mask();
        let null_0 = eval.next_trace_mask();
        let null_1 = eval.next_trace_mask();
        let out_amt_0: [E::F; NUM_LIMBS] = core::array::from_fn(|_| eval.next_trace_mask());
        let out_owner_0 = eval.next_trace_mask();
        let out_rand_0 = eval.next_trace_mask();
        let out_amt_1: [E::F; NUM_LIMBS] = core::array::from_fn(|_| eval.next_trace_mask());
        let out_rand_1 = eval.next_trace_mask();
        let fee_limbs: [E::F; NUM_LIMBS] = core::array::from_fn(|_| eval.next_trace_mask());
        let out_cm_0 = eval.next_trace_mask();
        let out_cm_1 = eval.next_trace_mask();
        let cred_issuer = eval.next_trace_mask();
        let cred_expiry = eval.next_trace_mask();
        let cred_secret = eval.next_trace_mask();
        let cred_cm = eval.next_trace_mask();
        let cred_null = eval.next_trace_mask();
        let epoch = eval.next_trace_mask();
        let pub_note_root = eval.next_trace_mask();
        let pub_cred_root = eval.next_trace_mask();

        // Nullifier inequality via multiplicative inverse
        let null_diff_inv = eval.next_trace_mask();
        eval.add_constraint((null_0.clone() - null_1.clone()) * null_diff_inv - E::F::one());

        // Credential not expired: cred_expiry - epoch - 1 >= 0
        let expiry_diff = eval.next_trace_mask();
        let mut reconstructed = E::F::zero();
        let mut power_of_two = E::F::one();
        let two = E::F::one() + E::F::one();
        for _ in 0..16 {
            let bit = eval.next_trace_mask();
            eval.add_constraint(bit.clone() * (bit.clone() - E::F::one()));
            reconstructed += bit * power_of_two.clone();
            power_of_two *= two.clone();
        }
        eval.add_constraint(reconstructed - expiry_diff.clone());
        eval.add_constraint(expiry_diff - (cred_expiry.clone() - epoch.clone() - E::F::one()));

        // Carry columns for limb-by-limb balance conservation.
        // Carries are in [-2, 1]. Biased carry = carry + CARRY_BIAS is in [0, 3] (2-bit).
        let carry_bias = E::F::from(M31::from(CARRY_BIAS));
        let radix = E::F::from(M31::from(RADIX as u32));
        let mut carries: [E::F; NUM_CARRIES] = core::array::from_fn(|_| E::F::zero());
        for k in 0..NUM_CARRIES {
            let c_b0 = eval.next_trace_mask();
            let c_b1 = eval.next_trace_mask();
            eval.add_constraint(c_b0.clone() * (c_b0.clone() - E::F::one()));
            eval.add_constraint(c_b1.clone() * (c_b1.clone() - E::F::one()));
            // carry_biased = b0 + 2*b1, carry = carry_biased - CARRY_BIAS
            carries[k] = c_b0 + two.clone() * c_b1 - carry_bias.clone();
        }

        // Limb-by-limb balance conservation:
        // For k = 0..3: in0[k] + in1[k] + c_prev - out0[k] - out1[k] - fee[k] - c_k * R = 0
        // c_prev for k=0 is 0, c_k for k=3 does not exist (enforced directly)
        for k in 0..NUM_LIMBS {
            let c_prev = if k == 0 { E::F::zero() } else { carries[k - 1].clone() };
            let lhs = in_amt_0[k].clone()
                + in_amt_1[k].clone()
                + c_prev
                - out_amt_0[k].clone()
                - out_amt_1[k].clone()
                - fee_limbs[k].clone();
            if k < NUM_CARRIES {
                eval.add_constraint(lhs - carries[k].clone() * radix.clone());
            } else {
                // Top limb: no outgoing carry, constraint is just lhs = 0
                eval.add_constraint(lhs);
            }
        }

        // Limb range checks: each of 20 limbs must fit in LIMB_BITS bits
        let all_limbs: [&[E::F; NUM_LIMBS]; NUM_AMOUNTS] =
            [&in_amt_0, &in_amt_1, &out_amt_0, &out_amt_1, &fee_limbs];
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

        // Note commitments with 4-limb amounts: H(asset, L0, L1, L2, L3, owner, randomness)
        let cm0_out = poseidon2_air::constrain_hash_many_7(
            &mut eval,
            in_asset.clone(),
            in_amt_0[0].clone(),
            in_amt_0[1].clone(),
            in_amt_0[2].clone(),
            in_amt_0[3].clone(),
            owner_out.clone(),
            in_rand_0.clone(),
            poseidon2::DOMAIN_NOTE_CM,
        );
        eval.add_constraint(in_cm_0.clone() - cm0_out);

        let cm1_out = poseidon2_air::constrain_hash_many_7(
            &mut eval,
            in_asset.clone(),
            in_amt_1[0].clone(),
            in_amt_1[1].clone(),
            in_amt_1[2].clone(),
            in_amt_1[3].clone(),
            owner_out.clone(),
            in_rand_1,
            poseidon2::DOMAIN_NOTE_CM,
        );
        eval.add_constraint(in_cm_1.clone() - cm1_out);

        let credcm_out = poseidon2_air::constrain_hash_many_4(
            &mut eval,
            cred_issuer,
            owner_out.clone(),
            cred_expiry.clone(),
            cred_secret.clone(),
            poseidon2::DOMAIN_CRED_CM,
        );
        eval.add_constraint(cred_cm.clone() - credcm_out);

        // Output commitments use same asset as inputs (enforced via shared in_asset column)
        let outcm0_out = poseidon2_air::constrain_hash_many_7(
            &mut eval,
            in_asset.clone(),
            out_amt_0[0].clone(),
            out_amt_0[1].clone(),
            out_amt_0[2].clone(),
            out_amt_0[3].clone(),
            out_owner_0.clone(),
            out_rand_0.clone(),
            poseidon2::DOMAIN_NOTE_CM,
        );
        eval.add_constraint(out_cm_0 - outcm0_out);

        // Output 1 is change back to sender (owner_out), output 0 goes to out_owner_0
        let outcm1_out = poseidon2_air::constrain_hash_many_7(
            &mut eval,
            in_asset.clone(),
            out_amt_1[0].clone(),
            out_amt_1[1].clone(),
            out_amt_1[2].clone(),
            out_amt_1[3].clone(),
            owner_out,
            out_rand_1.clone(),
            poseidon2::DOMAIN_NOTE_CM,
        );
        eval.add_constraint(out_cm_1 - outcm1_out);

        // Credential nullifier is bound to cred_cm to prevent cross-credential reuse
        let crednull_out = poseidon2_air::constrain_hash_many_4(
            &mut eval,
            cred_secret,
            cred_cm.clone(),
            epoch,
            E::F::zero(),
            poseidon2::DOMAIN_CRED_NULL,
        );
        eval.add_constraint(cred_null - crednull_out);

        // Merkle inclusion: two note paths + one credential path
        constrain_merkle_path(&mut eval, in_cm_0, pub_note_root.clone());
        constrain_merkle_path(&mut eval, in_cm_1, pub_note_root);
        constrain_merkle_path(&mut eval, cred_cm, pub_cred_root);

        eval
    }
}

pub type HushPaymentComponent = FrameworkComponent<HushPaymentEval>;

/// Decompose u64 amounts into limbs and compute carries for balance conservation.
fn compute_carries(witness: &PaymentWitness) -> [i32; NUM_CARRIES] {
    let in0 = amount_to_limbs(witness.in_amt_0);
    let in1 = amount_to_limbs(witness.in_amt_1);
    let out0 = amount_to_limbs(witness.out_amt_0);
    let out1 = amount_to_limbs(witness.out_amt_1);
    let fee = amount_to_limbs(witness.payment_fee_amount);

    let mut carries = [0i32; NUM_CARRIES];
    let mut c_prev = 0i32;
    for k in 0..NUM_LIMBS {
        let delta = i32::from(in0[k] as i16) + i32::from(in1[k] as i16) + c_prev
            - i32::from(out0[k] as i16) - i32::from(out1[k] as i16) - i32::from(fee[k] as i16);
        if k < NUM_CARRIES {
            // delta must be divisible by RADIX
            debug_assert_eq!(delta % (RADIX as i32), 0, "carry not exact at limb {k}");
            carries[k] = delta / (RADIX as i32);
            c_prev = carries[k];
        } else {
            debug_assert_eq!(delta, 0, "top limb conservation failed");
        }
    }
    carries
}

pub fn gen_trace(
    witness: &PaymentWitness,
    log_num_rows: u32,
) -> ColumnVec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>> {
    let num_rows = 1 << log_num_rows;
    let mut cols: Vec<BaseColumn> = (0..NUM_COLS).map(|_| BaseColumn::zeros(num_rows)).collect();

    let sk = M31::from(witness.sk);
    let owner = poseidon2::derive_owner(sk);
    let in_asset = M31::from(witness.in_asset);
    let in_rand_0 = M31::from(witness.in_rand_0);
    let in_rand_1 = M31::from(witness.in_rand_1);
    let out_owner_0 = M31::from(witness.out_owner_0);
    let out_rand_0 = M31::from(witness.out_rand_0);
    let out_rand_1 = M31::from(witness.out_rand_1);

    // Decompose amounts into 4 limbs each
    let in0_limbs = amount_to_limbs(witness.in_amt_0);
    let in1_limbs = amount_to_limbs(witness.in_amt_1);
    let out0_limbs = amount_to_limbs(witness.out_amt_0);
    let out1_limbs = amount_to_limbs(witness.out_amt_1);
    let fee_limbs = amount_to_limbs(witness.payment_fee_amount);

    let in0_m31: [M31; NUM_LIMBS] = core::array::from_fn(|i| M31::from(in0_limbs[i]));
    let in1_m31: [M31; NUM_LIMBS] = core::array::from_fn(|i| M31::from(in1_limbs[i]));
    let out0_m31: [M31; NUM_LIMBS] = core::array::from_fn(|i| M31::from(out0_limbs[i]));
    let out1_m31: [M31; NUM_LIMBS] = core::array::from_fn(|i| M31::from(out1_limbs[i]));
    let fee_m31: [M31; NUM_LIMBS] = core::array::from_fn(|i| M31::from(fee_limbs[i]));

    // Note commitments with 7 inputs: (asset, L0, L1, L2, L3, owner, randomness)
    let in_cm_0 = poseidon2::note_commitment(
        in_asset, in0_m31[0], in0_m31[1], in0_m31[2], in0_m31[3], owner, in_rand_0,
    );
    let in_cm_1 = poseidon2::note_commitment(
        in_asset, in1_m31[0], in1_m31[1], in1_m31[2], in1_m31[3], owner, in_rand_1,
    );
    let null_0 = poseidon2::nullifier(sk, in_cm_0);
    let null_1 = poseidon2::nullifier(sk, in_cm_1);
    let out_cm_0 = poseidon2::note_commitment(
        in_asset, out0_m31[0], out0_m31[1], out0_m31[2], out0_m31[3], out_owner_0, out_rand_0,
    );
    let out_cm_1 = poseidon2::note_commitment(
        in_asset, out1_m31[0], out1_m31[1], out1_m31[2], out1_m31[3], owner, out_rand_1,
    );

    let cred_issuer = M31::from(witness.cred_issuer);
    let cred_expiry = M31::from(witness.cred_expiry);
    let cred_secret = M31::from(witness.cred_secret);
    let cred_cm = poseidon2::credential_commitment(cred_issuer, owner, cred_expiry, cred_secret);
    let epoch = M31::from(witness.epoch);
    let cred_null = poseidon2::credential_nullifier(cred_secret, cred_cm, epoch);

    let pub_note_root = M31::from(witness.note_root);
    let pub_cred_root = M31::from(witness.cred_root);

    let null_diff = null_0 - null_1;
    let null_diff_inv =
        if null_diff == M31::from(0u32) { M31::from(0u32) } else { null_diff.inverse() };

    let expiry_diff_val = witness.cred_expiry.wrapping_sub(witness.epoch).wrapping_sub(1);
    let expiry_diff = M31::from(expiry_diff_val);
    let mut expiry_bits = [M31::from(0u32); 16];
    for i in 0..16 {
        expiry_bits[i] = M31::from((expiry_diff_val >> i) & 1);
    }

    // Compute carries for balance conservation
    let carries = compute_carries(witness);
    let carry_bits: [[M31; CARRY_BITS]; NUM_CARRIES] = core::array::from_fn(|k| {
        let biased = (carries[k] + CARRY_BIAS as i32) as u32;
        core::array::from_fn(|b| M31::from((biased >> b) & 1))
    });

    // Hash intermediates (7-input for note commitments, 4-input for credential)
    let owner_hash_cols =
        poseidon2_air::gen_hash2_intermediates(sk, M31::from(0u32), poseidon2::DOMAIN_OWNER);
    let null0_hash_cols =
        poseidon2_air::gen_hash2_intermediates(sk, in_cm_0, poseidon2::DOMAIN_NULLIFIER);
    let null1_hash_cols =
        poseidon2_air::gen_hash2_intermediates(sk, in_cm_1, poseidon2::DOMAIN_NULLIFIER);
    let cm0_hash_cols = poseidon2_air::gen_hash_many_7_intermediates(
        in_asset, in0_m31[0], in0_m31[1], in0_m31[2], in0_m31[3], owner, in_rand_0,
        poseidon2::DOMAIN_NOTE_CM,
    );
    let cm1_hash_cols = poseidon2_air::gen_hash_many_7_intermediates(
        in_asset, in1_m31[0], in1_m31[1], in1_m31[2], in1_m31[3], owner, in_rand_1,
        poseidon2::DOMAIN_NOTE_CM,
    );
    let credcm_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
        cred_issuer, owner, cred_expiry, cred_secret, poseidon2::DOMAIN_CRED_CM,
    );
    let outcm0_hash_cols = poseidon2_air::gen_hash_many_7_intermediates(
        in_asset, out0_m31[0], out0_m31[1], out0_m31[2], out0_m31[3], out_owner_0, out_rand_0,
        poseidon2::DOMAIN_NOTE_CM,
    );
    let outcm1_hash_cols = poseidon2_air::gen_hash_many_7_intermediates(
        in_asset, out1_m31[0], out1_m31[1], out1_m31[2], out1_m31[3], owner, out_rand_1,
        poseidon2::DOMAIN_NOTE_CM,
    );
    let crednull_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
        cred_secret, cred_cm, epoch, M31::from(0u32), poseidon2::DOMAIN_CRED_NULL,
    );
    // Merkle path intermediates
    let note_path_0_data = gen_merkle_path_trace(in_cm_0, &witness.note_path_0);
    let note_path_1_data = gen_merkle_path_trace(in_cm_1, &witness.note_path_1);
    let cred_path_data = gen_merkle_path_trace(cred_cm, &witness.cred_path);

    for r in 0..num_rows {
        let mut col = 0usize;
        let mut set = |c: &mut usize, val: M31| { cols[*c].set(r, val); *c += 1; };

        set(&mut col, sk);           // 0
        set(&mut col, owner);        // 1
        set(&mut col, in_asset);     // 2
        for &v in &in0_m31 { set(&mut col, v); }  // 3-6
        set(&mut col, in_rand_0);    // 7
        for &v in &in1_m31 { set(&mut col, v); }  // 8-11
        set(&mut col, in_rand_1);    // 12
        set(&mut col, in_cm_0);      // 13
        set(&mut col, in_cm_1);      // 14
        set(&mut col, null_0);       // 15
        set(&mut col, null_1);       // 16
        for &v in &out0_m31 { set(&mut col, v); }  // 17-20
        set(&mut col, out_owner_0);  // 21
        set(&mut col, out_rand_0);   // 22
        for &v in &out1_m31 { set(&mut col, v); }  // 23-26
        set(&mut col, out_rand_1);   // 27
        for &v in &fee_m31 { set(&mut col, v); }   // 28-31
        set(&mut col, out_cm_0);     // 32
        set(&mut col, out_cm_1);     // 33
        set(&mut col, cred_issuer);  // 34
        set(&mut col, cred_expiry);  // 35
        set(&mut col, cred_secret);  // 36
        set(&mut col, cred_cm);      // 37
        set(&mut col, cred_null);    // 38
        set(&mut col, epoch);        // 39
        set(&mut col, pub_note_root);// 40
        set(&mut col, pub_cred_root);// 41
        // Auxiliary
        set(&mut col, null_diff_inv);// 42
        set(&mut col, expiry_diff);  // 43
        for i in 0..16 { set(&mut col, expiry_bits[i]); } // 44-59
        // Carry bits
        for k in 0..NUM_CARRIES {
            for b in 0..CARRY_BITS {
                set(&mut col, carry_bits[k][b]);
            }
        } // 60-65
        assert_eq!(col, BASE_AUX_COLS);

        // Limb range decomposition: 5 amounts x 4 limbs x 15 bits
        let all_limb_vals = [in0_limbs, in1_limbs, out0_limbs, out1_limbs, fee_limbs];
        for limbs in &all_limb_vals {
            for &lv in limbs {
                for b in 0..LIMB_BITS {
                    cols[col].set(r, M31::from((lv >> b) & 1));
                    col += 1;
                }
            }
        }
        assert_eq!(col, BASE_AUX_COLS + LIMB_RANGE_COLS);

        let h = poseidon2_air::HASH_INTERMEDIATE_COLS;
        let all_hashes: [&Vec<M31>; 9] = [
            &owner_hash_cols,
            &null0_hash_cols,
            &null1_hash_cols,
            &cm0_hash_cols,
            &cm1_hash_cols,
            &credcm_hash_cols,
            &outcm0_hash_cols,
            &outcm1_hash_cols,
            &crednull_hash_cols,
        ];
        for hash_cols in &all_hashes {
            for i in 0..h {
                cols[col + i].set(r, hash_cols[i]);
            }
            col += h;
        }

        let path_cols = MERKLE_DEPTH * MERKLE_LEVEL_COLS;
        let all_paths: [&Vec<M31>; 3] = [&note_path_0_data, &note_path_1_data, &cred_path_data];
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

    assert_eq!(result.len(), MERKLE_DEPTH * MERKLE_LEVEL_COLS);
    result
}

pub struct PaymentPublicData {
    pub epoch: u32,
    pub note_root: u32,
    pub cred_root: u32,
    pub tx_binding_hash: u32,
    pub sender_binding_tag: u32,
    // Public outputs: nullifiers for spent-set, commitments for note tree
    pub null_0: u32,
    pub null_1: u32,
    pub out_cm_0: u32,
    pub out_cm_1: u32,
    pub cred_null: u32,
}

impl PaymentPublicData {
    pub fn mix_into(&self, channel: &mut impl Channel) {
        channel.mix_u64(self.epoch as u64);
        channel.mix_u64(self.note_root as u64);
        channel.mix_u64(self.cred_root as u64);
        channel.mix_u64(self.tx_binding_hash as u64);
        channel.mix_u64(self.sender_binding_tag as u64);
        channel.mix_u64(self.null_0 as u64);
        channel.mix_u64(self.null_1 as u64);
        channel.mix_u64(self.out_cm_0 as u64);
        channel.mix_u64(self.out_cm_1 as u64);
        channel.mix_u64(self.cred_null as u64);
    }
}

pub struct ProofResult {
    pub proof: stwo::core::proof::StarkProof<ProverMerkleHasher>,
    pub component: HushPaymentComponent,
    pub public_data: PaymentPublicData,
    pub log_num_rows: u32,
}

pub fn prove_payment(witness: &PaymentWitness) -> Result<ProofResult, String> {
    let log_num_rows = LOG_N_LANES;

    let total_in = witness.in_amt_0.checked_add(witness.in_amt_1)
        .ok_or_else(|| "input amount overflow".to_string())?;
    let total_out = witness.out_amt_0.checked_add(witness.out_amt_1)
        .and_then(|v| v.checked_add(witness.payment_fee_amount))
        .ok_or_else(|| "output amount overflow".to_string())?;
    if total_in != total_out {
        return Err(format!(
            "Balance conservation failed: inputs {total_in} != recipient+change+fee {total_out}"
        ));
    }

    if witness.cred_expiry <= witness.epoch {
        return Err(format!(
            "Credential expired: expiry {} <= epoch {}",
            witness.cred_expiry, witness.epoch
        ));
    }

    #[cfg(debug_assertions)]
    eprintln!("[payment] trace: {NUM_COLS} cols, log_rows={log_num_rows}");

    let sk = M31::from(witness.sk);
    let owner = poseidon2::derive_owner(sk);
    let in_asset = M31::from(witness.in_asset);
    let in_cm_0 = poseidon2::note_commitment_u64(in_asset, witness.in_amt_0, owner, M31::from(witness.in_rand_0));
    let in_cm_1 = poseidon2::note_commitment_u64(in_asset, witness.in_amt_1, owner, M31::from(witness.in_rand_1));

    // Verify Merkle paths
    let note_root = M31::from(witness.note_root);
    let note_path_0: Vec<(M31, u32)> =
        witness.note_path_0.iter().map(|&(s, d)| (M31::from(s), d)).collect();
    let note_path_1: Vec<(M31, u32)> =
        witness.note_path_1.iter().map(|&(s, d)| (M31::from(s), d)).collect();
    if !poseidon2::verify_merkle_path(in_cm_0, &note_path_0, note_root) {
        return Err("Note Merkle path for input 0 is invalid".to_string());
    }
    if !poseidon2::verify_merkle_path(in_cm_1, &note_path_1, note_root) {
        return Err("Note Merkle path for input 1 is invalid".to_string());
    }

    let cred_cm = poseidon2::credential_commitment(
        M31::from(witness.cred_issuer),
        owner,
        M31::from(witness.cred_expiry),
        M31::from(witness.cred_secret),
    );
    let cred_root = M31::from(witness.cred_root);
    let cred_path: Vec<(M31, u32)> =
        witness.cred_path.iter().map(|&(s, d)| (M31::from(s), d)).collect();
    if !poseidon2::verify_merkle_path(cred_cm, &cred_path, cred_root) {
        return Err(
            "Credential root mismatch: computed credential does not match the valid credential set"
                .to_string(),
        );
    }

    let expected_binding_hash = compute_mode_a_tx_binding_hash(
        witness.replay_domain,
        witness.in_asset,
        witness.binding_fee_asset,
        witness.fee_class,
        witness.fee_amount,
        PAYMENT_STANDARD_FEE_SCHEDULE_VERSION,
        witness.out_amt_0,
        witness.out_owner_0,
        witness.out_rand_0,
        witness.out_amt_1,
        witness.out_rand_1,
    );
    if witness.tx_binding_hash != expected_binding_hash {
        return Err(format!(
            "tx_binding_hash mismatch: witness {}, expected {}",
            witness.tx_binding_hash, expected_binding_hash
        ));
    }
    let expected_sender_binding_tag = derive_sender_binding_tag(witness.sk, witness.tx_binding_hash);
    if witness.sender_binding_tag != expected_sender_binding_tag {
        return Err(format!(
            "sender_binding_tag mismatch: witness {}, expected {}",
            witness.sender_binding_tag, expected_sender_binding_tag
        ));
    }

    // Compute public outputs
    let null_0 = poseidon2::nullifier(sk, in_cm_0);
    let null_1 = poseidon2::nullifier(sk, in_cm_1);
    let out_cm_0 = poseidon2::note_commitment_u64(
        in_asset, witness.out_amt_0, M31::from(witness.out_owner_0), M31::from(witness.out_rand_0),
    );
    let out_cm_1 = poseidon2::note_commitment_u64(
        in_asset, witness.out_amt_1, owner, M31::from(witness.out_rand_1),
    );
    let cred_null = poseidon2::credential_nullifier(
        M31::from(witness.cred_secret),
        cred_cm,
        M31::from(witness.epoch),
    );

    let trace = gen_trace(witness, log_num_rows);

    let public_data = PaymentPublicData {
        epoch: witness.epoch,
        note_root: witness.note_root,
        cred_root: witness.cred_root,
        tx_binding_hash: witness.tx_binding_hash,
        sender_binding_tag: witness.sender_binding_tag,
        null_0: null_0.0,
        null_1: null_1.0,
        out_cm_0: out_cm_0.0,
        out_cm_1: out_cm_1.0,
        cred_null: cred_null.0,
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

    let component = HushPaymentComponent::new(
        &mut TraceLocationAllocator::default(),
        HushPaymentEval { log_size: log_num_rows },
        QM31::zero(),
    );

    let proof = prove(&[&component], channel, commitment_scheme)
        .map_err(|e| format!("Proof generation failed: {e:?}"))?;

    Ok(ProofResult { proof, component, public_data, log_num_rows })
}

// FIXME: proof.clone() on verify is wasteful, should take &StarkProof
pub fn verify_payment(result: &ProofResult) -> Result<(), String> {
    let config = pcs_config();
    let channel = &mut ProverChannel::default();
    let commitment_scheme = &mut CommitmentSchemeVerifier::<ProverMerkleChannel>::new(config);

    let sizes = result.component.trace_log_degree_bounds();

    commitment_scheme.commit(result.proof.commitments[0], &sizes[0], channel);
    channel.mix_u64(result.log_num_rows as u64);
    result.public_data.mix_into(channel);
    commitment_scheme.commit(result.proof.commitments[1], &sizes[1], channel);

    verify(&[&result.component], channel, commitment_scheme, result.proof.clone())
        .map_err(|e| format!("Verification failed: {e:?}"))
}

// TODO(marty): extract common trace gen into a macro, three circuits repeat this pattern

pub struct BatchProofResult {
    pub proof: stwo::core::proof::StarkProof<ProverMerkleHasher>,
    pub component: HushPaymentComponent,
    pub public_data: Vec<PaymentPublicData>,
    pub log_num_rows: u32,
}

fn validate_witness(witness: &PaymentWitness) -> Result<PaymentPublicData, String> {
    let total_in = witness.in_amt_0.checked_add(witness.in_amt_1)
        .ok_or_else(|| "input amount overflow".to_string())?;
    let total_out = witness.out_amt_0.checked_add(witness.out_amt_1)
        .and_then(|v| v.checked_add(witness.payment_fee_amount))
        .ok_or_else(|| "output amount overflow".to_string())?;
    if total_in != total_out {
        return Err(format!(
            "Balance conservation failed: inputs {total_in} != recipient+change+fee {total_out}"
        ));
    }
    if witness.cred_expiry <= witness.epoch {
        return Err(format!(
            "Credential expired: expiry {} <= epoch {}",
            witness.cred_expiry, witness.epoch
        ));
    }

    let sk = M31::from(witness.sk);
    let owner = poseidon2::derive_owner(sk);
    let in_asset = M31::from(witness.in_asset);
    let in_cm_0 = poseidon2::note_commitment_u64(in_asset, witness.in_amt_0, owner, M31::from(witness.in_rand_0));
    let in_cm_1 = poseidon2::note_commitment_u64(in_asset, witness.in_amt_1, owner, M31::from(witness.in_rand_1));

    let note_root = M31::from(witness.note_root);
    let note_path_0: Vec<(M31, u32)> =
        witness.note_path_0.iter().map(|&(s, d)| (M31::from(s), d)).collect();
    let note_path_1: Vec<(M31, u32)> =
        witness.note_path_1.iter().map(|&(s, d)| (M31::from(s), d)).collect();
    if !poseidon2::verify_merkle_path(in_cm_0, &note_path_0, note_root) {
        return Err("Note Merkle path for input 0 is invalid".to_string());
    }
    if !poseidon2::verify_merkle_path(in_cm_1, &note_path_1, note_root) {
        return Err("Note Merkle path for input 1 is invalid".to_string());
    }

    let cred_cm = poseidon2::credential_commitment(
        M31::from(witness.cred_issuer),
        owner,
        M31::from(witness.cred_expiry),
        M31::from(witness.cred_secret),
    );
    let cred_root = M31::from(witness.cred_root);
    let cred_path: Vec<(M31, u32)> =
        witness.cred_path.iter().map(|&(s, d)| (M31::from(s), d)).collect();
    if !poseidon2::verify_merkle_path(cred_cm, &cred_path, cred_root) {
        return Err("Credential Merkle path is invalid".to_string());
    }

    let expected_binding_hash = compute_mode_a_tx_binding_hash(
        witness.replay_domain,
        witness.in_asset,
        witness.binding_fee_asset,
        witness.fee_class,
        witness.fee_amount,
        PAYMENT_STANDARD_FEE_SCHEDULE_VERSION,
        witness.out_amt_0,
        witness.out_owner_0,
        witness.out_rand_0,
        witness.out_amt_1,
        witness.out_rand_1,
    );
    if witness.tx_binding_hash != expected_binding_hash {
        return Err(format!(
            "tx_binding_hash mismatch: witness {}, expected {}",
            witness.tx_binding_hash, expected_binding_hash
        ));
    }
    let expected_sender_binding_tag = derive_sender_binding_tag(witness.sk, witness.tx_binding_hash);
    if witness.sender_binding_tag != expected_sender_binding_tag {
        return Err(format!(
            "sender_binding_tag mismatch: witness {}, expected {}",
            witness.sender_binding_tag, expected_sender_binding_tag
        ));
    }

    let null_0 = poseidon2::nullifier(sk, in_cm_0);
    let null_1 = poseidon2::nullifier(sk, in_cm_1);
    let out_cm_0 = poseidon2::note_commitment_u64(
        in_asset, witness.out_amt_0, M31::from(witness.out_owner_0), M31::from(witness.out_rand_0),
    );
    let out_cm_1 = poseidon2::note_commitment_u64(
        in_asset, witness.out_amt_1, owner, M31::from(witness.out_rand_1),
    );
    let cred_null = poseidon2::credential_nullifier(
        M31::from(witness.cred_secret),
        cred_cm,
        M31::from(witness.epoch),
    );

    Ok(PaymentPublicData {
        epoch: witness.epoch,
        note_root: witness.note_root,
        cred_root: witness.cred_root,
        tx_binding_hash: witness.tx_binding_hash,
        sender_binding_tag: witness.sender_binding_tag,
        null_0: null_0.0,
        null_1: null_1.0,
        out_cm_0: out_cm_0.0,
        out_cm_1: out_cm_1.0,
        cred_null: cred_null.0,
    })
}

fn gen_trace_batch(
    witnesses: &[PaymentWitness],
    log_num_rows: u32,
) -> ColumnVec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>> {
    let num_rows = 1 << log_num_rows;
    let mut cols: Vec<BaseColumn> = (0..NUM_COLS).map(|_| BaseColumn::zeros(num_rows)).collect();

    for r in 0..num_rows {
        let w = &witnesses[r % witnesses.len()];
        let sk = M31::from(w.sk);
        let owner = poseidon2::derive_owner(sk);
        let in_asset = M31::from(w.in_asset);
        let in_rand_0 = M31::from(w.in_rand_0);
        let in_rand_1 = M31::from(w.in_rand_1);
        let out_owner_0 = M31::from(w.out_owner_0);
        let out_rand_0 = M31::from(w.out_rand_0);
        let out_rand_1 = M31::from(w.out_rand_1);

        // Decompose amounts into 4 limbs each
        let in0_limbs = amount_to_limbs(w.in_amt_0);
        let in1_limbs = amount_to_limbs(w.in_amt_1);
        let out0_limbs = amount_to_limbs(w.out_amt_0);
        let out1_limbs = amount_to_limbs(w.out_amt_1);
        let fee_limbs = amount_to_limbs(w.payment_fee_amount);

        let in0_m31: [M31; NUM_LIMBS] = core::array::from_fn(|i| M31::from(in0_limbs[i]));
        let in1_m31: [M31; NUM_LIMBS] = core::array::from_fn(|i| M31::from(in1_limbs[i]));
        let out0_m31: [M31; NUM_LIMBS] = core::array::from_fn(|i| M31::from(out0_limbs[i]));
        let out1_m31: [M31; NUM_LIMBS] = core::array::from_fn(|i| M31::from(out1_limbs[i]));
        let fee_m31: [M31; NUM_LIMBS] = core::array::from_fn(|i| M31::from(fee_limbs[i]));

        // Note commitments with 7 inputs: (asset, L0, L1, L2, L3, owner, randomness)
        let in_cm_0 = poseidon2::note_commitment(
            in_asset, in0_m31[0], in0_m31[1], in0_m31[2], in0_m31[3], owner, in_rand_0,
        );
        let in_cm_1 = poseidon2::note_commitment(
            in_asset, in1_m31[0], in1_m31[1], in1_m31[2], in1_m31[3], owner, in_rand_1,
        );
        let null_0 = poseidon2::nullifier(sk, in_cm_0);
        let null_1 = poseidon2::nullifier(sk, in_cm_1);
        let out_cm_0 = poseidon2::note_commitment(
            in_asset, out0_m31[0], out0_m31[1], out0_m31[2], out0_m31[3], out_owner_0, out_rand_0,
        );
        let out_cm_1 = poseidon2::note_commitment(
            in_asset, out1_m31[0], out1_m31[1], out1_m31[2], out1_m31[3], owner, out_rand_1,
        );
        let cred_issuer = M31::from(w.cred_issuer);
        let cred_expiry = M31::from(w.cred_expiry);
        let cred_secret = M31::from(w.cred_secret);
        let cred_cm =
            poseidon2::credential_commitment(cred_issuer, owner, cred_expiry, cred_secret);
        let epoch = M31::from(w.epoch);
        let cred_null = poseidon2::credential_nullifier(cred_secret, cred_cm, epoch);
        let pub_note_root = M31::from(w.note_root);
        let pub_cred_root = M31::from(w.cred_root);

        let null_diff = null_0 - null_1;
        let null_diff_inv =
            if null_diff == M31::from(0u32) { M31::from(0u32) } else { null_diff.inverse() };

        let expiry_diff_val = w.cred_expiry.wrapping_sub(w.epoch).wrapping_sub(1);
        let expiry_diff = M31::from(expiry_diff_val);
        let mut expiry_bits = [M31::from(0u32); 16];
        for i in 0..16 {
            expiry_bits[i] = M31::from((expiry_diff_val >> i) & 1);
        }

        // Compute carries for balance conservation
        let carries = compute_carries(w);
        let carry_bits: [[M31; CARRY_BITS]; NUM_CARRIES] = core::array::from_fn(|k| {
            let biased = (carries[k] + CARRY_BIAS as i32) as u32;
            core::array::from_fn(|b| M31::from((biased >> b) & 1))
        });

        // Hash intermediates (7-input for note commitments, 4-input for credential)
        let owner_hash_cols =
            poseidon2_air::gen_hash2_intermediates(sk, M31::from(0u32), poseidon2::DOMAIN_OWNER);
        let null0_hash_cols =
            poseidon2_air::gen_hash2_intermediates(sk, in_cm_0, poseidon2::DOMAIN_NULLIFIER);
        let null1_hash_cols =
            poseidon2_air::gen_hash2_intermediates(sk, in_cm_1, poseidon2::DOMAIN_NULLIFIER);
        let cm0_hash_cols = poseidon2_air::gen_hash_many_7_intermediates(
            in_asset, in0_m31[0], in0_m31[1], in0_m31[2], in0_m31[3], owner, in_rand_0,
            poseidon2::DOMAIN_NOTE_CM,
        );
        let cm1_hash_cols = poseidon2_air::gen_hash_many_7_intermediates(
            in_asset, in1_m31[0], in1_m31[1], in1_m31[2], in1_m31[3], owner, in_rand_1,
            poseidon2::DOMAIN_NOTE_CM,
        );
        let credcm_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
            cred_issuer, owner, cred_expiry, cred_secret, poseidon2::DOMAIN_CRED_CM,
        );
        let outcm0_hash_cols = poseidon2_air::gen_hash_many_7_intermediates(
            in_asset, out0_m31[0], out0_m31[1], out0_m31[2], out0_m31[3], out_owner_0, out_rand_0,
            poseidon2::DOMAIN_NOTE_CM,
        );
        let outcm1_hash_cols = poseidon2_air::gen_hash_many_7_intermediates(
            in_asset, out1_m31[0], out1_m31[1], out1_m31[2], out1_m31[3], owner, out_rand_1,
            poseidon2::DOMAIN_NOTE_CM,
        );
        let crednull_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
            cred_secret, cred_cm, epoch, M31::from(0u32), poseidon2::DOMAIN_CRED_NULL,
        );
        let note_path_0_data = gen_merkle_path_trace(in_cm_0, &w.note_path_0);
        let note_path_1_data = gen_merkle_path_trace(in_cm_1, &w.note_path_1);
        let cred_path_data = gen_merkle_path_trace(cred_cm, &w.cred_path);

        let mut col = 0usize;
        let mut set = |c: &mut usize, val: M31| { cols[*c].set(r, val); *c += 1; };

        set(&mut col, sk);           // 0
        set(&mut col, owner);        // 1
        set(&mut col, in_asset);     // 2
        for &v in &in0_m31 { set(&mut col, v); }  // 3-6
        set(&mut col, in_rand_0);    // 7
        for &v in &in1_m31 { set(&mut col, v); }  // 8-11
        set(&mut col, in_rand_1);    // 12
        set(&mut col, in_cm_0);      // 13
        set(&mut col, in_cm_1);      // 14
        set(&mut col, null_0);       // 15
        set(&mut col, null_1);       // 16
        for &v in &out0_m31 { set(&mut col, v); }  // 17-20
        set(&mut col, out_owner_0);  // 21
        set(&mut col, out_rand_0);   // 22
        for &v in &out1_m31 { set(&mut col, v); }  // 23-26
        set(&mut col, out_rand_1);   // 27
        for &v in &fee_m31 { set(&mut col, v); }   // 28-31
        set(&mut col, out_cm_0);     // 32
        set(&mut col, out_cm_1);     // 33
        set(&mut col, cred_issuer);  // 34
        set(&mut col, cred_expiry);  // 35
        set(&mut col, cred_secret);  // 36
        set(&mut col, cred_cm);      // 37
        set(&mut col, cred_null);    // 38
        set(&mut col, epoch);        // 39
        set(&mut col, pub_note_root);// 40
        set(&mut col, pub_cred_root);// 41
        // Auxiliary
        set(&mut col, null_diff_inv);// 42
        set(&mut col, expiry_diff);  // 43
        for i in 0..16 { set(&mut col, expiry_bits[i]); } // 44-59
        // Carry bits
        for k in 0..NUM_CARRIES {
            for b in 0..CARRY_BITS {
                set(&mut col, carry_bits[k][b]);
            }
        } // 60-65
        assert_eq!(col, BASE_AUX_COLS);

        // Limb range decomposition: 5 amounts x 4 limbs x 15 bits
        let all_limb_vals = [in0_limbs, in1_limbs, out0_limbs, out1_limbs, fee_limbs];
        for limbs in &all_limb_vals {
            for &lv in limbs {
                for b in 0..LIMB_BITS {
                    cols[col].set(r, M31::from((lv >> b) & 1));
                    col += 1;
                }
            }
        }
        assert_eq!(col, BASE_AUX_COLS + LIMB_RANGE_COLS);

        let h = poseidon2_air::HASH_INTERMEDIATE_COLS;
        let all_hashes: [&Vec<M31>; 9] = [
            &owner_hash_cols,
            &null0_hash_cols,
            &null1_hash_cols,
            &cm0_hash_cols,
            &cm1_hash_cols,
            &credcm_hash_cols,
            &outcm0_hash_cols,
            &outcm1_hash_cols,
            &crednull_hash_cols,
        ];
        for hash_cols in &all_hashes {
            for i in 0..h {
                cols[col + i].set(r, hash_cols[i]);
            }
            col += h;
        }

        let path_cols = MERKLE_DEPTH * MERKLE_LEVEL_COLS;
        let all_paths: [&Vec<M31>; 3] = [&note_path_0_data, &note_path_1_data, &cred_path_data];
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

pub fn prove_payment_batch(witnesses: &[PaymentWitness]) -> Result<BatchProofResult, String> {
    if witnesses.is_empty() {
        return Err("Batch must contain at least one transaction".to_string());
    }

    let mut all_public_data = Vec::with_capacity(witnesses.len());
    for (i, w) in witnesses.iter().enumerate() {
        match validate_witness(w) {
            Ok(pd) => all_public_data.push(pd),
            Err(e) => return Err(format!("Transaction {i} failed validation: {e}")),
        }
    }

    let min_rows = witnesses.len().next_power_of_two();
    let log_num_rows = std::cmp::max((min_rows as f64).log2().ceil() as u32, LOG_N_LANES);

    let trace = gen_trace_batch(witnesses, log_num_rows);

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
    channel.mix_u64(witnesses.len() as u64);
    for pd in &all_public_data {
        pd.mix_into(channel);
    }

    let mut tree_builder = commitment_scheme.tree_builder();
    tree_builder.extend_evals(trace);
    tree_builder.commit(channel);

    let component = HushPaymentComponent::new(
        &mut TraceLocationAllocator::default(),
        HushPaymentEval { log_size: log_num_rows },
        QM31::zero(),
    );

    let proof = prove(&[&component], channel, commitment_scheme)
        .map_err(|e| format!("Batch proof generation failed: {e:?}"))?;

    Ok(BatchProofResult { proof, component, public_data: all_public_data, log_num_rows })
}

pub fn verify_payment_batch(result: &BatchProofResult) -> Result<(), String> {
    let config = pcs_config();
    let channel = &mut ProverChannel::default();
    let commitment_scheme = &mut CommitmentSchemeVerifier::<ProverMerkleChannel>::new(config);

    let sizes = result.component.trace_log_degree_bounds();
    commitment_scheme.commit(result.proof.commitments[0], &sizes[0], channel);

    channel.mix_u64(result.log_num_rows as u64);
    channel.mix_u64(result.public_data.len() as u64);
    for pd in &result.public_data {
        pd.mix_into(channel);
    }

    commitment_scheme.commit(result.proof.commitments[1], &sizes[1], channel);

    verify(&[&result.component], channel, commitment_scheme, result.proof.clone())
        .map_err(|e| format!("Batch verification failed: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        payment_fixtures::{valid_usdc_same_asset_fixture, valid_usdt_same_asset_fixture},
        payment_tx::{
            validate_payment_tx, AssetId, NoteInput, PaymentTxV1, RecipientIntent,
            PAYMENT_TX_V1_REPLAY_DOMAIN,
        },
    };

    fn valid_witness() -> PaymentWitness {
        valid_usdc_same_asset_fixture().witness
    }

    #[test]
    fn test_payment_roundtrip() {
        let witness = valid_witness();
        let result = prove_payment(&witness).expect("Proof generation should succeed");
        verify_payment(&result).expect("Verification should succeed");

        // Verify public outputs are populated
        assert_ne!(result.public_data.null_0, 0);
        assert_ne!(result.public_data.null_1, 0);
        assert_ne!(result.public_data.out_cm_0, 0);
        assert_ne!(result.public_data.out_cm_1, 0);
        assert_ne!(result.public_data.cred_null, 0);
        assert_eq!(result.public_data.tx_binding_hash, witness.tx_binding_hash);
        assert_ne!(result.public_data.null_0, result.public_data.null_1);
    }

    #[test]
    fn test_payment_roundtrip_usdt_same_asset() {
        let witness = valid_usdt_same_asset_fixture().witness;
        let result = prove_payment(&witness).expect("USDT same-asset proof should succeed");
        verify_payment(&result).expect("USDT same-asset verification should succeed");
        assert_eq!(result.public_data.tx_binding_hash, witness.tx_binding_hash);
    }

    #[test]
    fn test_balance_mismatch() {
        let mut witness = valid_witness();
        witness.out_amt_0 = 9000;
        match prove_payment(&witness) {
            Err(e) => assert!(e.contains("Balance conservation failed"), "Got: {e}"),
            Ok(_) => panic!("Should have rejected bad balance"),
        }
    }

    #[test]
    fn test_expired_cred() {
        let mut witness = valid_witness();
        witness.cred_expiry = 500;
        match prove_payment(&witness) {
            Err(e) => assert!(e.contains("Credential expired"), "Got: {e}"),
            Ok(_) => panic!("Should have rejected expired credential"),
        }
    }

    #[test]
    fn test_reject_revoked_credential() {
        let mut witness = valid_witness();
        witness.cred_issuer = 9999;
        assert!(prove_payment(&witness).is_err());
    }

    #[test]
    fn test_m31_wrapping_attack() {
        // Attempt value creation via modular wrap: in=0+0, out=(p-1)+1+fee
        // With multi-limb encoding, amounts can exceed p without field wrapping
        // because limb-by-limb conservation checks integer equality, not field
        // equality (all intermediate values are small relative to p).
        // The prover-side u64 balance check catches this before trace generation.
        let mut witness = valid_witness();
        let p = (1u64 << 31) - 1;
        witness.in_amt_0 = 0;
        witness.in_amt_1 = 0;
        witness.out_amt_0 = p - 1;
        witness.out_amt_1 = 1;
        assert!(prove_payment(&witness).is_err());
    }

    #[test]
    fn test_wrong_fee_amount_rejected() {
        let mut witness = valid_witness();
        witness.fee_amount += 1;
        match prove_payment(&witness) {
            Err(e) => assert!(e.contains("Balance conservation failed") || e.contains("tx_binding_hash mismatch")),
            Ok(_) => panic!("Should have rejected wrong fee amount"),
        }
    }

    #[test]
    fn test_wrong_binding_hash_rejected() {
        let mut witness = valid_witness();
        witness.tx_binding_hash = witness.tx_binding_hash.wrapping_add(1);
        match prove_payment(&witness) {
            Err(e) => assert!(e.contains("tx_binding_hash mismatch"), "Got: {e}"),
            Ok(_) => panic!("Should have rejected wrong tx binding hash"),
        }
    }

    #[test]
    fn test_receiver_full_amount_and_sender_change_preserved() {
        let fixture = valid_usdc_same_asset_fixture();
        assert_eq!(fixture.witness.out_amt_0, fixture.tx.recipient.amount);
        assert_eq!(fixture.witness.out_amt_1, fixture.tx.sender_change.amount);
        assert_eq!(
            fixture.witness.in_amt_0 + fixture.witness.in_amt_1,
            fixture.witness.out_amt_0
                + fixture.witness.out_amt_1
                + fixture.witness.payment_fee_amount
        );
    }

    fn make_witness(
        sk_val: u32,
        amt_0: u32,
        amt_1: u32,
        rand_0: u32,
        rand_1: u32,
        out_split: u32,
    ) -> PaymentWitness {
        let tx = PaymentTxV1::build_same_asset(
            AssetId::Usdc,
            [
                NoteInput { amount: u64::from(amt_0), randomness: rand_0 },
                NoteInput { amount: u64::from(amt_1), randomness: rand_1 },
            ],
            RecipientIntent { amount: u64::from(out_split), owner: 99_999, randomness: rand_0 + 1_000 },
            rand_1 + 1_000,
            sk_val,
        )
        .expect("test tx should build");
        validate_payment_tx(&tx).expect("test tx should validate");

        let owner = poseidon2::derive_owner(M31::from(sk_val));
        let in_asset = M31::from(AssetId::Usdc as u32);
        let in_cm_0 =
            poseidon2::note_commitment_u64(in_asset, u64::from(amt_0), owner, M31::from(rand_0));
        let in_cm_1 =
            poseidon2::note_commitment_u64(in_asset, u64::from(amt_1), owner, M31::from(rand_1));

        let mut note_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
        note_tree.set_leaf(0, in_cm_0);
        note_tree.set_leaf(1, in_cm_1);
        let note_path_0_vec = note_tree.path(0);
        let note_path_1_vec = note_tree.path(1);

        let cred_cm = poseidon2::credential_commitment(
            M31::from(1u32),
            owner,
            M31::from(2000u32),
            M31::from(sk_val + 100),
        );
        let mut cred_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
        cred_tree.set_leaf(0, cred_cm);
        let cred_path_vec = cred_tree.path(0);

        let mut note_path_0 = [(0u32, 0u32); MERKLE_DEPTH];
        let mut note_path_1 = [(0u32, 0u32); MERKLE_DEPTH];
        let mut cred_path = [(0u32, 0u32); MERKLE_DEPTH];
        for i in 0..MERKLE_DEPTH {
            note_path_0[i] = (note_path_0_vec[i].0 .0, note_path_0_vec[i].1);
            note_path_1[i] = (note_path_1_vec[i].0 .0, note_path_1_vec[i].1);
            cred_path[i] = (cred_path_vec[i].0 .0, cred_path_vec[i].1);
        }

        PaymentWitness {
            epoch: 1000,
            note_root: note_tree.root().0,
            cred_root: cred_tree.root().0,
            sk: sk_val,
            in_asset: AssetId::Usdc as u32,
            in_amt_0: u64::from(amt_0),
            in_rand_0: rand_0,
            in_amt_1: u64::from(amt_1),
            in_rand_1: rand_1,
            out_amt_0: u64::from(tx.recipient.amount),
            out_owner_0: tx.recipient.owner,
            out_rand_0: tx.recipient.randomness,
            out_amt_1: u64::from(tx.sender_change.amount),
            out_rand_1: tx.sender_change.randomness,
            payment_fee_amount: u64::from(tx.descriptor.fee_amount),
            binding_fee_asset: tx.descriptor.fee_asset,
            fee_amount: u64::from(tx.descriptor.fee_amount),
            fee_class: tx.descriptor.fee_class,
            replay_domain: PAYMENT_TX_V1_REPLAY_DOMAIN,
            tx_binding_hash: tx.tx_binding_hash,
            sender_binding_tag: tx.attachment.sender_binding_tag,
            cred_issuer: 1,
            cred_expiry: 2000,
            cred_secret: sk_val + 100,
            note_path_0,
            note_path_1,
            cred_path,
        }
    }

    #[test]
    fn test_batch_4tx() {
        let witnesses = vec![
            make_witness(100, 5000, 3000, 11, 22, 4000),
            make_witness(200, 6000, 2000, 33, 44, 5000),
            make_witness(300, 4000, 4000, 55, 66, 3000),
            make_witness(400, 7000, 1000, 77, 88, 6000),
        ];

        let result = prove_payment_batch(&witnesses).expect("Batch proof should succeed");
        verify_payment_batch(&result).expect("Batch verification should succeed");

        assert_eq!(result.public_data.len(), 4);
        for pd in &result.public_data {
            assert_ne!(pd.null_0, 0);
            assert_ne!(pd.null_1, 0);
            assert_ne!(pd.null_0, pd.null_1);
        }
    }

    #[test]
    fn test_batch_with_bad_witness() {
        let mut bad = make_witness(500, 5000, 3000, 11, 22, 4000);
        bad.cred_expiry = 500; // expired credential
        let witnesses = vec![make_witness(100, 5000, 3000, 11, 22, 4000), bad];
        match prove_payment_batch(&witnesses) {
            Err(e) => assert!(e.contains("Transaction 1 failed"), "Got: {e}"),
            Ok(_) => panic!("Batch should reject invalid witness"),
        }
    }

    #[test]
    fn test_batch_single_eq_individual() {
        let w = valid_witness();
        let single = prove_payment(&w).expect("Single proof should succeed");
        let batch = prove_payment_batch(&[w]).expect("Batch of 1 should succeed");

        assert_eq!(batch.public_data.len(), 1);
        assert_eq!(batch.public_data[0].null_0, single.public_data.null_0);
        assert_eq!(batch.public_data[0].null_1, single.public_data.null_1);
        assert_eq!(batch.public_data[0].out_cm_0, single.public_data.out_cm_0);
        assert_eq!(batch.public_data[0].out_cm_1, single.public_data.out_cm_1);
        assert_eq!(batch.public_data[0].cred_null, single.public_data.cred_null);
    }

    #[test]
    fn test_zero_value_transfer() {
        // Inputs must cover the fee (50) even when recipient amount is zero.
        let w = make_witness(42, 50, 0, 10, 20, 0);
        let result = prove_payment(&w).expect("zero-value transfer should prove");
        verify_payment(&result).expect("zero-value transfer should verify");
    }

    #[test]
    #[ignore] // slow (~3s)
    fn test_payment_determinism() {
        let w = valid_witness();
        let r1 = prove_payment(&w).unwrap();
        let r2 = prove_payment(&w).unwrap();
        assert_eq!(r1.public_data.null_0, r2.public_data.null_0);
        assert_eq!(r1.public_data.out_cm_0, r2.public_data.out_cm_0);
    }
}
