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
    poseidon2, poseidon2_air,
    prover_common::{pcs_config, ProverChannel, ProverMerkleChannel, ProverMerkleHasher},
    types::{PaymentWitness, MERKLE_DEPTH},
};

const LOG_CONSTRAINT_EVAL_BLOWUP_FACTOR: u32 = 1;
const MERKLE_LEVEL_COLS: usize = 3 + poseidon2_air::HASH_INTERMEDIATE_COLS; // 639

// 21-bit range check prevents M31 wrapping
const AMT_BITS: usize = 21;
const AMT_RANGE_COLS: usize = 4 * AMT_BITS; // 84 cols for 4 amounts

// 44 base + 84 amount range + 9*636 hash + 3*20*639 Merkle = 44,492
const NUM_COLS: usize = 44
    + AMT_RANGE_COLS
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
        let in_amt_0 = eval.next_trace_mask();
        let in_rand_0 = eval.next_trace_mask();
        let in_amt_1 = eval.next_trace_mask();
        let in_rand_1 = eval.next_trace_mask();
        let in_cm_0 = eval.next_trace_mask();
        let in_cm_1 = eval.next_trace_mask();
        let null_0 = eval.next_trace_mask();
        let null_1 = eval.next_trace_mask();
        let out_amt_0 = eval.next_trace_mask();
        let out_owner_0 = eval.next_trace_mask();
        let out_rand_0 = eval.next_trace_mask();
        let out_amt_1 = eval.next_trace_mask();
        let out_rand_1 = eval.next_trace_mask();
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

        // Balance conservation
        eval.add_constraint(
            in_amt_0.clone() + in_amt_1.clone() - out_amt_0.clone() - out_amt_1.clone(),
        );

        // Nullifier inequality via multiplicative inverse
        let null_diff_inv = eval.next_trace_mask();
        eval.add_constraint((null_0.clone() - null_1.clone()) * null_diff_inv - E::F::one());

        // Credential not expired: cred_expiry - epoch - 1 >= 0
        let expiry_diff = eval.next_trace_mask();
        let mut reconstructed = E::F::zero();
        let mut power_of_two = E::F::one();
        let two = E::F::one() + E::F::one();
        for _ in 0..16 {
            let bit = eval.next_trace_mask(); // 28..43
            eval.add_constraint(bit.clone() * (bit.clone() - E::F::one()));
            reconstructed += bit * power_of_two.clone();
            power_of_two *= two.clone();
        }
        eval.add_constraint(reconstructed - expiry_diff.clone());
        eval.add_constraint(expiry_diff - (cred_expiry.clone() - epoch.clone() - E::F::one()));

        // Amount range checks: each amount must fit in AMT_BITS bits
        for amt in [in_amt_0.clone(), in_amt_1.clone(), out_amt_0.clone(), out_amt_1.clone()] {
            let mut recon = E::F::zero();
            let mut p2 = E::F::one();
            for _ in 0..AMT_BITS {
                let bit = eval.next_trace_mask();
                eval.add_constraint(bit.clone() * (bit.clone() - E::F::one()));
                recon += bit * p2.clone();
                p2 *= two.clone();
            }
            eval.add_constraint(recon - amt);
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

        let cm0_out = poseidon2_air::constrain_hash_many_4(
            &mut eval,
            in_asset.clone(),
            in_amt_0.clone(),
            owner_out.clone(),
            in_rand_0.clone(),
            poseidon2::DOMAIN_NOTE_CM,
        );
        eval.add_constraint(in_cm_0.clone() - cm0_out);

        let cm1_out = poseidon2_air::constrain_hash_many_4(
            &mut eval,
            in_asset.clone(),
            in_amt_1.clone(),
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
        let outcm0_out = poseidon2_air::constrain_hash_many_4(
            &mut eval,
            in_asset.clone(),
            out_amt_0,
            out_owner_0,
            out_rand_0,
            poseidon2::DOMAIN_NOTE_CM,
        );
        eval.add_constraint(out_cm_0 - outcm0_out);

        // Output 1 is change back to sender (owner_out), output 0 goes to out_owner_0
        let outcm1_out = poseidon2_air::constrain_hash_many_4(
            &mut eval,
            in_asset,
            out_amt_1,
            owner_out,
            out_rand_1,
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

        // Merkle inclusion (depth 8): two note paths + one credential path
        constrain_merkle_path(&mut eval, in_cm_0, pub_note_root.clone());
        constrain_merkle_path(&mut eval, in_cm_1, pub_note_root);
        constrain_merkle_path(&mut eval, cred_cm, pub_cred_root);

        eval
    }
}

pub type HushPaymentComponent = FrameworkComponent<HushPaymentEval>;

pub fn gen_trace(
    witness: &PaymentWitness,
    log_num_rows: u32,
) -> ColumnVec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>> {
    let num_rows = 1 << log_num_rows;
    let mut cols: Vec<BaseColumn> = (0..NUM_COLS).map(|_| BaseColumn::zeros(num_rows)).collect();

    let sk = M31::from(witness.sk);
    let owner = poseidon2::derive_owner(sk);
    let in_asset = M31::from(witness.in_asset);

    let in_amt_0 = M31::from(witness.in_amt_0);
    let in_rand_0 = M31::from(witness.in_rand_0);
    let in_amt_1 = M31::from(witness.in_amt_1);
    let in_rand_1 = M31::from(witness.in_rand_1);

    let in_cm_0 = poseidon2::note_commitment(in_asset, in_amt_0, owner, in_rand_0);
    let in_cm_1 = poseidon2::note_commitment(in_asset, in_amt_1, owner, in_rand_1);

    let null_0 = poseidon2::nullifier(sk, in_cm_0);
    let null_1 = poseidon2::nullifier(sk, in_cm_1);

    let out_amt_0 = M31::from(witness.out_amt_0);
    let out_owner_0 = M31::from(witness.out_owner_0);
    let out_rand_0 = M31::from(witness.out_rand_0);
    let out_amt_1 = M31::from(witness.out_amt_1);
    let out_rand_1 = M31::from(witness.out_rand_1);

    let out_cm_0 = poseidon2::note_commitment(in_asset, out_amt_0, out_owner_0, out_rand_0);
    let out_cm_1 = poseidon2::note_commitment(in_asset, out_amt_1, owner, out_rand_1);

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

    let expiry_val = witness.cred_expiry;
    let epoch_val = witness.epoch;
    let expiry_diff_val = expiry_val.wrapping_sub(epoch_val).wrapping_sub(1);
    let expiry_diff = M31::from(expiry_diff_val);
    let mut expiry_bits = [M31::from(0u32); 16];
    for i in 0..16 {
        expiry_bits[i] = M31::from((expiry_diff_val >> i) & 1);
    }

    let owner_hash_cols =
        poseidon2_air::gen_hash2_intermediates(sk, M31::from(0u32), poseidon2::DOMAIN_OWNER);
    let null0_hash_cols =
        poseidon2_air::gen_hash2_intermediates(sk, in_cm_0, poseidon2::DOMAIN_NULLIFIER);
    let null1_hash_cols =
        poseidon2_air::gen_hash2_intermediates(sk, in_cm_1, poseidon2::DOMAIN_NULLIFIER);
    let cm0_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
        in_asset,
        in_amt_0,
        owner,
        in_rand_0,
        poseidon2::DOMAIN_NOTE_CM,
    );
    let cm1_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
        in_asset,
        in_amt_1,
        owner,
        in_rand_1,
        poseidon2::DOMAIN_NOTE_CM,
    );
    let credcm_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
        cred_issuer,
        owner,
        cred_expiry,
        cred_secret,
        poseidon2::DOMAIN_CRED_CM,
    );
    let outcm0_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
        in_asset,
        out_amt_0,
        out_owner_0,
        out_rand_0,
        poseidon2::DOMAIN_NOTE_CM,
    );
    let outcm1_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
        in_asset,
        out_amt_1,
        owner,
        out_rand_1,
        poseidon2::DOMAIN_NOTE_CM,
    );
    let crednull_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
        cred_secret,
        cred_cm,
        epoch,
        M31::from(0u32),
        poseidon2::DOMAIN_CRED_NULL,
    );

    // Merkle path intermediates
    let note_path_0_data = gen_merkle_path_trace(in_cm_0, &witness.note_path_0);
    let note_path_1_data = gen_merkle_path_trace(in_cm_1, &witness.note_path_1);
    let cred_path_data = gen_merkle_path_trace(cred_cm, &witness.cred_path);

    for r in 0..num_rows {
        cols[0].set(r, sk);
        cols[1].set(r, owner);
        cols[2].set(r, in_asset);
        cols[3].set(r, in_amt_0);
        cols[4].set(r, in_rand_0);
        cols[5].set(r, in_amt_1);
        cols[6].set(r, in_rand_1);
        cols[7].set(r, in_cm_0);
        cols[8].set(r, in_cm_1);
        cols[9].set(r, null_0);
        cols[10].set(r, null_1);
        cols[11].set(r, out_amt_0);
        cols[12].set(r, out_owner_0);
        cols[13].set(r, out_rand_0);
        cols[14].set(r, out_amt_1);
        cols[15].set(r, out_rand_1);
        cols[16].set(r, out_cm_0);
        cols[17].set(r, out_cm_1);
        cols[18].set(r, cred_issuer);
        cols[19].set(r, cred_expiry);
        cols[20].set(r, cred_secret);
        cols[21].set(r, cred_cm);
        cols[22].set(r, cred_null);
        cols[23].set(r, epoch);
        cols[24].set(r, pub_note_root);
        cols[25].set(r, pub_cred_root);
        cols[26].set(r, null_diff_inv);
        cols[27].set(r, expiry_diff);
        for i in 0..16 {
            cols[28 + i].set(r, expiry_bits[i]);
        }

        // Amount range decomposition
        let amts = [witness.in_amt_0, witness.in_amt_1, witness.out_amt_0, witness.out_amt_1];
        for (ai, &av) in amts.iter().enumerate() {
            for b in 0..AMT_BITS {
                cols[44 + ai * AMT_BITS + b].set(r, M31::from((av >> b) & 1));
            }
        }

        let hash_base = 44 + AMT_RANGE_COLS;
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
        for (hi, hash_cols) in all_hashes.iter().enumerate() {
            for i in 0..h {
                cols[hash_base + hi * h + i].set(r, hash_cols[i]);
            }
        }

        let merkle_base = hash_base + 9 * h;
        let path_cols = MERKLE_DEPTH * MERKLE_LEVEL_COLS;
        let all_paths: [&Vec<M31>; 3] = [&note_path_0_data, &note_path_1_data, &cred_path_data];
        for (pi, path_data) in all_paths.iter().enumerate() {
            for i in 0..path_cols {
                cols[merkle_base + pi * path_cols + i].set(r, path_data[i]);
            }
        }
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

    let total_in = witness.in_amt_0 + witness.in_amt_1;
    let total_out = witness.out_amt_0 + witness.out_amt_1;
    if total_in != total_out {
        return Err(format!(
            "Balance conservation failed: inputs {total_in} != outputs {total_out}"
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
    let in_cm_0 = poseidon2::note_commitment(
        in_asset,
        M31::from(witness.in_amt_0),
        owner,
        M31::from(witness.in_rand_0),
    );
    let in_cm_1 = poseidon2::note_commitment(
        in_asset,
        M31::from(witness.in_amt_1),
        owner,
        M31::from(witness.in_rand_1),
    );

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

    // Compute public outputs
    let null_0 = poseidon2::nullifier(sk, in_cm_0);
    let null_1 = poseidon2::nullifier(sk, in_cm_1);
    let out_cm_0 = poseidon2::note_commitment(
        in_asset,
        M31::from(witness.out_amt_0),
        M31::from(witness.out_owner_0),
        M31::from(witness.out_rand_0),
    );
    let out_cm_1 = poseidon2::note_commitment(
        in_asset,
        M31::from(witness.out_amt_1),
        owner,
        M31::from(witness.out_rand_1),
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
    let total_in = witness.in_amt_0 + witness.in_amt_1;
    let total_out = witness.out_amt_0 + witness.out_amt_1;
    if total_in != total_out {
        return Err(format!(
            "Balance conservation failed: inputs {total_in} != outputs {total_out}"
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
    let in_cm_0 = poseidon2::note_commitment(
        in_asset,
        M31::from(witness.in_amt_0),
        owner,
        M31::from(witness.in_rand_0),
    );
    let in_cm_1 = poseidon2::note_commitment(
        in_asset,
        M31::from(witness.in_amt_1),
        owner,
        M31::from(witness.in_rand_1),
    );

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

    let null_0 = poseidon2::nullifier(sk, in_cm_0);
    let null_1 = poseidon2::nullifier(sk, in_cm_1);
    let out_cm_0 = poseidon2::note_commitment(
        in_asset,
        M31::from(witness.out_amt_0),
        M31::from(witness.out_owner_0),
        M31::from(witness.out_rand_0),
    );
    let out_cm_1 = poseidon2::note_commitment(
        in_asset,
        M31::from(witness.out_amt_1),
        owner,
        M31::from(witness.out_rand_1),
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
        let in_amt_0 = M31::from(w.in_amt_0);
        let in_rand_0 = M31::from(w.in_rand_0);
        let in_amt_1 = M31::from(w.in_amt_1);
        let in_rand_1 = M31::from(w.in_rand_1);
        let in_cm_0 = poseidon2::note_commitment(in_asset, in_amt_0, owner, in_rand_0);
        let in_cm_1 = poseidon2::note_commitment(in_asset, in_amt_1, owner, in_rand_1);
        let null_0 = poseidon2::nullifier(sk, in_cm_0);
        let null_1 = poseidon2::nullifier(sk, in_cm_1);
        let out_amt_0 = M31::from(w.out_amt_0);
        let out_owner_0 = M31::from(w.out_owner_0);
        let out_rand_0 = M31::from(w.out_rand_0);
        let out_amt_1 = M31::from(w.out_amt_1);
        let out_rand_1 = M31::from(w.out_rand_1);
        let out_cm_0 = poseidon2::note_commitment(in_asset, out_amt_0, out_owner_0, out_rand_0);
        let out_cm_1 = poseidon2::note_commitment(in_asset, out_amt_1, owner, out_rand_1);
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

        let owner_hash_cols =
            poseidon2_air::gen_hash2_intermediates(sk, M31::from(0u32), poseidon2::DOMAIN_OWNER);
        let null0_hash_cols =
            poseidon2_air::gen_hash2_intermediates(sk, in_cm_0, poseidon2::DOMAIN_NULLIFIER);
        let null1_hash_cols =
            poseidon2_air::gen_hash2_intermediates(sk, in_cm_1, poseidon2::DOMAIN_NULLIFIER);
        let cm0_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
            in_asset,
            in_amt_0,
            owner,
            in_rand_0,
            poseidon2::DOMAIN_NOTE_CM,
        );
        let cm1_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
            in_asset,
            in_amt_1,
            owner,
            in_rand_1,
            poseidon2::DOMAIN_NOTE_CM,
        );
        let credcm_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
            cred_issuer,
            owner,
            cred_expiry,
            cred_secret,
            poseidon2::DOMAIN_CRED_CM,
        );
        let outcm0_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
            in_asset,
            out_amt_0,
            out_owner_0,
            out_rand_0,
            poseidon2::DOMAIN_NOTE_CM,
        );
        let outcm1_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
            in_asset,
            out_amt_1,
            owner,
            out_rand_1,
            poseidon2::DOMAIN_NOTE_CM,
        );
        let crednull_hash_cols = poseidon2_air::gen_hash_many_4_intermediates(
            cred_secret,
            cred_cm,
            epoch,
            M31::from(0u32),
            poseidon2::DOMAIN_CRED_NULL,
        );

        let note_path_0_data = gen_merkle_path_trace(in_cm_0, &w.note_path_0);
        let note_path_1_data = gen_merkle_path_trace(in_cm_1, &w.note_path_1);
        let cred_path_data = gen_merkle_path_trace(cred_cm, &w.cred_path);

        cols[0].set(r, sk);
        cols[1].set(r, owner);
        cols[2].set(r, in_asset);
        cols[3].set(r, in_amt_0);
        cols[4].set(r, in_rand_0);
        cols[5].set(r, in_amt_1);
        cols[6].set(r, in_rand_1);
        cols[7].set(r, in_cm_0);
        cols[8].set(r, in_cm_1);
        cols[9].set(r, null_0);
        cols[10].set(r, null_1);
        cols[11].set(r, out_amt_0);
        cols[12].set(r, out_owner_0);
        cols[13].set(r, out_rand_0);
        cols[14].set(r, out_amt_1);
        cols[15].set(r, out_rand_1);
        cols[16].set(r, out_cm_0);
        cols[17].set(r, out_cm_1);
        cols[18].set(r, cred_issuer);
        cols[19].set(r, cred_expiry);
        cols[20].set(r, cred_secret);
        cols[21].set(r, cred_cm);
        cols[22].set(r, cred_null);
        cols[23].set(r, epoch);
        cols[24].set(r, pub_note_root);
        cols[25].set(r, pub_cred_root);
        cols[26].set(r, null_diff_inv);
        cols[27].set(r, expiry_diff);
        for i in 0..16 {
            cols[28 + i].set(r, expiry_bits[i]);
        }

        let amts = [w.in_amt_0, w.in_amt_1, w.out_amt_0, w.out_amt_1];
        for (ai, &av) in amts.iter().enumerate() {
            for b in 0..AMT_BITS {
                cols[44 + ai * AMT_BITS + b].set(r, M31::from((av >> b) & 1));
            }
        }

        let hash_base = 44 + AMT_RANGE_COLS;
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
        for (hi, hash_cols) in all_hashes.iter().enumerate() {
            for i in 0..h {
                cols[hash_base + hi * h + i].set(r, hash_cols[i]);
            }
        }

        let merkle_base = hash_base + 9 * h;
        let path_cols = MERKLE_DEPTH * MERKLE_LEVEL_COLS;
        let all_paths: [&Vec<M31>; 3] = [&note_path_0_data, &note_path_1_data, &cred_path_data];
        for (pi, path_data) in all_paths.iter().enumerate() {
            for i in 0..path_cols {
                cols[merkle_base + pi * path_cols + i].set(r, path_data[i]);
            }
        }
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

    fn valid_witness() -> PaymentWitness {
        let sk = M31::from(12345u32);
        let owner = poseidon2::derive_owner(sk);
        let in_asset = M31::from(1u32);

        let in_cm_0 =
            poseidon2::note_commitment(in_asset, M31::from(7000u32), owner, M31::from(111u32));
        let in_cm_1 =
            poseidon2::note_commitment(in_asset, M31::from(3000u32), owner, M31::from(222u32));

        let mut note_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
        note_tree.set_leaf(0, in_cm_0);
        note_tree.set_leaf(1, in_cm_1);
        let note_root = note_tree.root();
        let note_path_0_vec = note_tree.path(0);
        let note_path_1_vec = note_tree.path(1);

        let cred_cm = poseidon2::credential_commitment(
            M31::from(1u32),
            owner,
            M31::from(2000u32),
            M31::from(777u32),
        );
        let mut cred_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
        cred_tree.set_leaf(0, cred_cm);
        let cred_root = cred_tree.root();
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
            note_root: note_root.0,
            cred_root: cred_root.0,
            sk: 12345,
            in_asset: 1,
            in_amt_0: 7000,
            in_rand_0: 111,
            in_amt_1: 3000,
            in_rand_1: 222,
            out_amt_0: 8000,
            out_owner_0: 99999,
            out_rand_0: 333,
            out_amt_1: 2000,
            out_rand_1: 444,
            cred_issuer: 1,
            cred_expiry: 2000,
            cred_secret: 777,
            note_path_0,
            note_path_1,
            cred_path,
        }
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
        assert_ne!(result.public_data.null_0, result.public_data.null_1);
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
        // Attempt value creation via modular wrap: in=0+0, out=(p-1)+1
        let mut witness = valid_witness();
        let p = (1u32 << 31) - 1;
        witness.in_amt_0 = 0;
        witness.in_amt_1 = 0;
        witness.out_amt_0 = p - 1; // > 2^21, should fail range check
        witness.out_amt_1 = 1;
        // Balance: 0+0 = (p-1)+1 = p = 0 mod p, so balance passes.
        // But the prover-side check uses u32 addition, not field addition,
        // so it catches this before trace generation.
        assert!(prove_payment(&witness).is_err());
    }

    fn make_witness(
        sk_val: u32,
        amt_0: u32,
        amt_1: u32,
        rand_0: u32,
        rand_1: u32,
        out_split: u32,
    ) -> PaymentWitness {
        let sk = M31::from(sk_val);
        let owner = poseidon2::derive_owner(sk);
        let in_asset = M31::from(1u32);

        let in_cm_0 =
            poseidon2::note_commitment(in_asset, M31::from(amt_0), owner, M31::from(rand_0));
        let in_cm_1 =
            poseidon2::note_commitment(in_asset, M31::from(amt_1), owner, M31::from(rand_1));

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
            in_asset: 1,
            in_amt_0: amt_0,
            in_rand_0: rand_0,
            in_amt_1: amt_1,
            in_rand_1: rand_1,
            out_amt_0: out_split,
            out_owner_0: 99999,
            out_rand_0: rand_0 + 1000,
            out_amt_1: amt_0 + amt_1 - out_split,
            out_rand_1: rand_1 + 1000,
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
        let w = make_witness(42, 0, 0, 10, 20, 0);
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
