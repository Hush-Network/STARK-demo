//! Poseidon2 AIR constraints. S-box x^5 decomposed into (x^2, x^4, x^5)
//! so all constraints stay degree 2.

use num_traits::Zero;
use stwo::core::fields::m31::M31;
use stwo_constraint_framework::EvalAtRow;

use crate::poseidon2::{
    EXTERNAL_CONSTANTS, INTERNAL_CONSTANTS, INTERNAL_DIAG_M1, M4, NUM_FULL_ROUNDS_FIRST,
    NUM_FULL_ROUNDS_LAST, NUM_PARTIAL_ROUNDS, RATE, TOTAL_ROUNDS, WIDTH,
};

#[cfg(test)]
const LOG_CONSTRAINT_EVAL_BLOWUP_FACTOR: u32 = 1;

const FULL_ROUND_COLS: usize = WIDTH + 2 * WIDTH; // 48
const PARTIAL_ROUND_COLS: usize = WIDTH + 2; // 18
const NUM_FULL_ROUNDS: usize = NUM_FULL_ROUNDS_FIRST + NUM_FULL_ROUNDS_LAST;

pub const HASH_INTERMEDIATE_COLS: usize =
    NUM_FULL_ROUNDS * FULL_ROUND_COLS + NUM_PARTIAL_ROUNDS * PARTIAL_ROUND_COLS;

fn is_full_round(round_index: usize) -> bool {
    !(NUM_FULL_ROUNDS_FIRST..NUM_FULL_ROUNDS_FIRST + NUM_PARTIAL_ROUNDS).contains(&round_index)
}

fn get_external_constants(round_index: usize) -> &'static [u32; 16] {
    if round_index < NUM_FULL_ROUNDS_FIRST {
        &EXTERNAL_CONSTANTS[round_index]
    } else {
        let idx =
            NUM_FULL_ROUNDS_FIRST + (round_index - NUM_FULL_ROUNDS_FIRST - NUM_PARTIAL_ROUNDS);
        &EXTERNAL_CONSTANTS[idx]
    }
}

fn get_internal_constant(round_index: usize) -> u32 {
    INTERNAL_CONSTANTS[round_index - NUM_FULL_ROUNDS_FIRST]
}

fn diag_m1_as_m31(i: usize) -> M31 {
    let v = INTERNAL_DIAG_M1[i];
    if v < 0 {
        let p = (1u64 << 31) - 1;
        let abs_v = (-v) as u64;
        M31::from(((p - abs_v % p) % p) as u32)
    } else {
        M31::from(v as u32)
    }
}

pub fn gen_permutation_intermediates(input: &[M31; WIDTH]) -> Vec<M31> {
    let mut cols = Vec::with_capacity(HASH_INTERMEDIATE_COLS);
    let mut state = *input;
    let p = (1u64 << 31) - 1;

    // Initial external linear layer (must match poseidon2::permute_state)
    {
        let mut chunks = [[M31::from(0u32); 4]; 4];
        for c in 0..4 {
            let inp = [state[4 * c], state[4 * c + 1], state[4 * c + 2], state[4 * c + 3]];
            for i in 0..4 {
                chunks[c][i] = M31::from(0u32);
                for j in 0..4 {
                    chunks[c][i] += M31::from(M4[i][j]) * inp[j];
                }
            }
        }
        let mut col_sums = [M31::from(0u32); 4];
        for j in 0..4 {
            for c in 0..4 {
                col_sums[j] += chunks[c][j];
            }
        }
        for c in 0..4 {
            for j in 0..4 {
                state[4 * c + j] = chunks[c][j] + col_sums[j];
            }
        }
    }

    for round in 0..TOTAL_ROUNDS {
        if is_full_round(round) {
            let rc = get_external_constants(round);
            let mut s5 = [M31::from(0u32); WIDTH];
            let mut s_sq_arr = [M31::from(0u32); WIDTH];
            let mut s_4th_arr = [M31::from(0u32); WIDTH];
            for i in 0..WIDTH {
                let after_rc = state[i] + M31::from(rc[i]);
                let s_sq = after_rc * after_rc;
                let s_4th = s_sq * s_sq;
                s5[i] = s_4th * after_rc;
                s_sq_arr[i] = s_sq;
                s_4th_arr[i] = s_4th;
            }
            // External linear layer on s5
            let mut chunks = [[M31::from(0u32); 4]; 4];
            for c in 0..4 {
                let inp = [s5[4 * c], s5[4 * c + 1], s5[4 * c + 2], s5[4 * c + 3]];
                for i in 0..4 {
                    chunks[c][i] = M31::from(0u32);
                    for j in 0..4 {
                        chunks[c][i] += M31::from(M4[i][j]) * inp[j];
                    }
                }
            }
            let mut col_sums = [M31::from(0u32); 4];
            for j in 0..4 {
                for c in 0..4 {
                    col_sums[j] += chunks[c][j];
                }
            }
            for c in 0..4 {
                for j in 0..4 {
                    state[4 * c + j] = chunks[c][j] + col_sums[j];
                }
            }
            // Store: 16 state, 16 s_sq, 16 s_4th
            for i in 0..WIDTH {
                cols.push(state[i]);
            }
            for i in 0..WIDTH {
                cols.push(s_sq_arr[i]);
            }
            for i in 0..WIDTH {
                cols.push(s_4th_arr[i]);
            }
        } else {
            let rc = get_internal_constant(round);
            let after_rc = state[0] + M31::from(rc);
            let s0_sq = after_rc * after_rc;
            let s0_4th = s0_sq * s0_sq;
            let s5_0 = s0_4th * after_rc;

            let mut sum = s5_0;
            for i in 1..WIDTH {
                sum += state[i];
            }
            let mut new_state = [M31::from(0u32); WIDTH];
            for i in 0..WIDTH {
                let si = if i == 0 { s5_0 } else { state[i] };
                let v = INTERNAL_DIAG_M1[i];
                let vi_si = if v < 0 {
                    let abs_v = (-v) as u64;
                    let product = (si.0 as u64 * abs_v) % p;
                    M31::from(((p - product) % p) as u32)
                } else {
                    let product = (si.0 as u64 * v as u64) % p;
                    M31::from(product as u32)
                };
                new_state[i] = sum + vi_si;
            }
            state = new_state;
            for i in 0..WIDTH {
                cols.push(state[i]);
            }
            cols.push(s0_sq);
            cols.push(s0_4th);
        }
    }
    assert_eq!(cols.len(), HASH_INTERMEDIATE_COLS);
    cols
}

pub fn gen_hash2_intermediates(a: M31, b: M31, domain: u32) -> Vec<M31> {
    let mut input = [M31::from(0u32); WIDTH];
    input[0] = a;
    input[1] = b;
    input[RATE] = M31::from(domain);
    gen_permutation_intermediates(&input)
}

pub fn gen_hash_many_4_intermediates(i0: M31, i1: M31, i2: M31, i3: M31, domain: u32) -> Vec<M31> {
    let mut input = [M31::from(0u32); WIDTH];
    input[0] = i0;
    input[1] = i1;
    input[2] = i2;
    input[3] = i3;
    input[RATE] = M31::from(domain);
    gen_permutation_intermediates(&input)
}

pub fn constrain_permutation<E: EvalAtRow>(eval: &mut E, input: [E::F; WIDTH]) -> [E::F; WIDTH] {
    // Initial external linear layer (linear, no extra columns needed)
    let mut prev: Vec<E::F> = {
        let inp = &input;
        let mut result = vec![E::F::zero(); WIDTH];
        for c in 0..4usize {
            for j in 0..4usize {
                let mut chunk_val = E::F::zero();
                for k in 0..4usize {
                    chunk_val += E::F::from(M31::from(M4[j][k])) * inp[4 * c + k].clone();
                }
                result[4 * c + j] = chunk_val;
            }
        }
        let mut col_sums: [E::F; 4] = core::array::from_fn(|_| E::F::zero());
        for j in 0..4usize {
            for c in 0..4usize {
                let mut chunk_val = E::F::zero();
                for k in 0..4usize {
                    chunk_val += E::F::from(M31::from(M4[j][k])) * inp[4 * c + k].clone();
                }
                col_sums[j] = col_sums[j].clone() + chunk_val;
            }
        }
        for c in 0..4usize {
            for j in 0..4usize {
                result[4 * c + j] = result[4 * c + j].clone() + col_sums[j].clone();
            }
        }
        result
    };

    for round in 0..TOTAL_ROUNDS {
        if is_full_round(round) {
            let rc = get_external_constants(round);
            let mut new_state = Vec::with_capacity(WIDTH);
            for _ in 0..WIDTH {
                new_state.push(eval.next_trace_mask());
            }
            let mut s_sq = Vec::with_capacity(WIDTH);
            for _ in 0..WIDTH {
                s_sq.push(eval.next_trace_mask());
            }
            let mut s_4th = Vec::with_capacity(WIDTH);
            for _ in 0..WIDTH {
                s_4th.push(eval.next_trace_mask());
            }

            for i in 0..WIDTH {
                let after_rc = prev[i].clone() + E::F::from(M31::from(rc[i]));
                eval.add_constraint(s_sq[i].clone() - after_rc.clone() * after_rc.clone());
                eval.add_constraint(s_4th[i].clone() - s_sq[i].clone() * s_sq[i].clone());
            }

            let mut col_sum: [E::F; 4] = core::array::from_fn(|_| E::F::zero());
            for j in 0..4usize {
                for c2 in 0..4usize {
                    for k in 0..4usize {
                        let idx = 4 * c2 + k;
                        let after_rc_k = prev[idx].clone() + E::F::from(M31::from(rc[idx]));
                        let s5_k = s_4th[idx].clone() * after_rc_k;
                        col_sum[j] = col_sum[j].clone() + E::F::from(M31::from(M4[j][k])) * s5_k;
                    }
                }
            }

            for c in 0..4usize {
                for j in 0..4usize {
                    let mut chunk_val = E::F::zero();
                    for k in 0..4usize {
                        let idx = 4 * c + k;
                        let after_rc_k = prev[idx].clone() + E::F::from(M31::from(rc[idx]));
                        let s5_k = s_4th[idx].clone() * after_rc_k;
                        chunk_val += E::F::from(M31::from(M4[j][k])) * s5_k;
                    }
                    eval.add_constraint(
                        new_state[4 * c + j].clone() - chunk_val - col_sum[j].clone(),
                    );
                }
            }
            prev = new_state;
        } else {
            let rc = get_internal_constant(round);
            let mut new_state = Vec::with_capacity(WIDTH);
            for _ in 0..WIDTH {
                new_state.push(eval.next_trace_mask());
            }
            let s0_sq = eval.next_trace_mask();
            let s0_4th = eval.next_trace_mask();

            let after_rc = prev[0].clone() + E::F::from(M31::from(rc));
            eval.add_constraint(s0_sq.clone() - after_rc.clone() * after_rc.clone());
            eval.add_constraint(s0_4th.clone() - s0_sq.clone() * s0_sq.clone());

            let mut sum_tail = E::F::zero();
            for i in 1..WIDTH {
                sum_tail += prev[i].clone();
            }

            for i in 0..WIDTH {
                let vi = E::F::from(diag_m1_as_m31(i));
                let sbox_out_i =
                    if i == 0 { s0_4th.clone() * after_rc.clone() } else { prev[i].clone() };
                eval.add_constraint(
                    new_state[i].clone()
                        - s0_4th.clone() * after_rc.clone()
                        - sum_tail.clone()
                        - vi * sbox_out_i,
                );
            }
            prev = new_state;
        }
    }

    core::array::from_fn(|i| prev[i].clone())
}

pub fn constrain_hash2<E: EvalAtRow>(eval: &mut E, a: E::F, b: E::F, domain: u32) -> E::F {
    let mut input = core::array::from_fn(|_| E::F::zero());
    input[0] = a;
    input[1] = b;
    input[RATE] = E::F::from(M31::from(domain));
    let output = constrain_permutation(eval, input);
    output[0].clone()
}

pub fn constrain_hash_many_4<E: EvalAtRow>(
    eval: &mut E,
    i0: E::F,
    i1: E::F,
    i2: E::F,
    i3: E::F,
    domain: u32,
) -> E::F {
    let mut input = core::array::from_fn(|_| E::F::zero());
    input[0] = i0;
    input[1] = i1;
    input[2] = i2;
    input[3] = i3;
    input[RATE] = E::F::from(M31::from(domain));
    let output = constrain_permutation(eval, input);
    output[0].clone()
}

// Standalone hash2 prove/verify, only used in tests.
#[cfg(test)]
use stwo::core::air::Component;
#[cfg(test)]
use stwo::core::channel::Channel;
#[cfg(test)]
use stwo::core::fields::qm31::QM31;
#[cfg(test)]
use stwo::core::pcs::CommitmentSchemeVerifier;
#[cfg(test)]
use stwo::core::poly::circle::CanonicCoset;
#[cfg(test)]
use stwo::core::verifier::verify;
#[cfg(test)]
use stwo::core::ColumnVec;
#[cfg(test)]
use stwo::prover::backend::simd::column::BaseColumn;
#[cfg(test)]
use stwo::prover::backend::simd::m31::LOG_N_LANES;
#[cfg(test)]
use stwo::prover::backend::simd::SimdBackend;
#[cfg(test)]
use stwo::prover::backend::Column;
#[cfg(test)]
use stwo::prover::poly::circle::{CircleEvaluation, PolyOps};
#[cfg(test)]
use stwo::prover::poly::BitReversedOrder;
#[cfg(test)]
use stwo::prover::{prove, CommitmentSchemeProver};
#[cfg(test)]
use stwo_constraint_framework::{FrameworkComponent, FrameworkEval, TraceLocationAllocator};

#[cfg(test)]
use crate::poseidon2;
#[cfg(test)]
use crate::prover_common::{pcs_config, ProverChannel, ProverMerkleChannel, ProverMerkleHasher};

#[cfg(test)]
const STANDALONE_NUM_COLS: usize = WIDTH + HASH_INTERMEDIATE_COLS + 1;

#[cfg(test)]
#[derive(Clone)]
struct Poseidon2HashEval {
    log_size: u32,
}

#[cfg(test)]
impl FrameworkEval for Poseidon2HashEval {
    fn log_size(&self) -> u32 {
        self.log_size
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.log_size + LOG_CONSTRAINT_EVAL_BLOWUP_FACTOR
    }

    fn evaluate<E: EvalAtRow>(&self, mut eval: E) -> E {
        let input: [E::F; WIDTH] = core::array::from_fn(|_| eval.next_trace_mask());
        let output = constrain_permutation(&mut eval, input);
        let expected_output = eval.next_trace_mask();
        eval.add_constraint(output[0].clone() - expected_output);
        eval
    }
}

#[cfg(test)]
type Poseidon2HashComponent = FrameworkComponent<Poseidon2HashEval>;

#[cfg(test)]
struct Hash2Witness {
    a: u32,
    b: u32,
    domain: u32,
    expected_output: u32,
}

#[cfg(test)]
fn gen_hash_trace(
    witness: &Hash2Witness,
    log_num_rows: u32,
) -> ColumnVec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>> {
    let num_rows = 1 << log_num_rows;
    let mut cols: Vec<BaseColumn> =
        (0..STANDALONE_NUM_COLS).map(|_| BaseColumn::zeros(num_rows)).collect();

    let mut input = [M31::from(0u32); WIDTH];
    input[0] = M31::from(witness.a);
    input[1] = M31::from(witness.b);
    input[RATE] = M31::from(witness.domain);

    for i in 0..WIDTH {
        for row in 0..num_rows {
            cols[i].set(row, input[i]);
        }
    }

    let intermediates = gen_permutation_intermediates(&input);
    for (idx, &val) in intermediates.iter().enumerate() {
        for row in 0..num_rows {
            cols[WIDTH + idx].set(row, val);
        }
    }

    let expected = M31::from(witness.expected_output);
    for row in 0..num_rows {
        cols[WIDTH + HASH_INTERMEDIATE_COLS].set(row, expected);
    }

    let domain = CanonicCoset::new(log_num_rows).circle_domain();
    cols.into_iter().map(|col| CircleEvaluation::new(domain, col)).collect()
}

#[cfg(test)]
struct Hash2PublicData {
    expected_output: u32,
}

#[cfg(test)]
impl Hash2PublicData {
    fn mix_into(&self, channel: &mut impl Channel) {
        channel.mix_u64(self.expected_output as u64);
    }
}

#[cfg(test)]
struct Hash2ProofResult {
    proof: stwo::core::proof::StarkProof<ProverMerkleHasher>,
    component: Poseidon2HashComponent,
    public_data: Hash2PublicData,
    log_num_rows: u32,
}

#[cfg(test)]
fn prove_hash2(witness: &Hash2Witness) -> Result<Hash2ProofResult, String> {
    let log_num_rows = LOG_N_LANES;

    let mut input = [M31::from(0u32); WIDTH];
    input[0] = M31::from(witness.a);
    input[1] = M31::from(witness.b);
    input[RATE] = M31::from(witness.domain);
    let mut state = input;
    poseidon2::permute_state(&mut state);
    if state[0].0 != witness.expected_output {
        return Err(format!(
            "Hash mismatch: got {} but expected {}",
            state[0].0, witness.expected_output
        ));
    }

    let trace = gen_hash_trace(witness, log_num_rows);

    let public_data = Hash2PublicData { expected_output: witness.expected_output };

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

    let component = Poseidon2HashComponent::new(
        &mut TraceLocationAllocator::default(),
        Poseidon2HashEval { log_size: log_num_rows },
        QM31::zero(),
    );

    let proof = prove(&[&component], channel, commitment_scheme)
        .map_err(|e| format!("Proof generation failed: {e:?}"))?;

    Ok(Hash2ProofResult { proof, component, public_data, log_num_rows })
}

#[cfg(test)]
fn verify_hash2(result: &Hash2ProofResult) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash2_roundtrip() {
        let a = 12345u32;
        let b = 67890u32;
        // Use raw permutation (domain=0 in standalone component)
        let mut state = [M31::from(0u32); WIDTH];
        state[0] = M31::from(a);
        state[1] = M31::from(b);
        poseidon2::permute_state(&mut state);
        let expected = state[0].0;

        let witness = Hash2Witness { a, b, domain: 0, expected_output: expected };
        let result = prove_hash2(&witness).expect("Proof should succeed");
        verify_hash2(&result).expect("Verification should succeed");
    }

    #[test]
    fn test_bad_output_fails() {
        let witness = Hash2Witness { a: 12345, b: 67890, domain: 0, expected_output: 99999 };
        match prove_hash2(&witness) {
            Err(e) => assert!(e.contains("Hash mismatch"), "Got: {e}"),
            Ok(_) => panic!("Should have rejected wrong hash output"),
        }
    }

    #[test]
    fn test_poseidon2_air_owner_derivation() {
        let sk = 42u32;
        let owner = poseidon2::derive_owner(M31::from(sk));
        let witness =
            Hash2Witness { a: sk, b: 0, domain: poseidon2::DOMAIN_OWNER, expected_output: owner.0 };
        let result = prove_hash2(&witness).expect("Proof should succeed");
        verify_hash2(&result).expect("Verification should succeed");
    }

    #[test]
    fn test_column_count() {
        let expected = NUM_FULL_ROUNDS * FULL_ROUND_COLS + NUM_PARTIAL_ROUNDS * PARTIAL_ROUND_COLS;
        assert_eq!(HASH_INTERMEDIATE_COLS, expected);
    }
}
