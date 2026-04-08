use std::time::Instant;

use hush_demo_stark::{
    accounting::{
        accepted_payment_record, accepted_protocol_action_record, BlockAccountingBuilder,
        EpochAccumulator, ProtocolActionTx, ValidatorBlockParticipation, ValidatorStakeInfo,
    },
    circuit, credential_issuance, poseidon2, time_window,
    measurement::{duration_to_ms, format_duration_ms},
    payment_fixtures::{valid_usdc_hush_fee_fixture, valid_usdc_same_asset_fixture},
    payment_tx::TxKind,
    payment_validation,
    types::MERKLE_DEPTH,
};
use stwo::core::fields::m31::M31;

const ITERATIONS: usize = 10;

fn stats(times: &[f64]) -> (f64, f64, f64) {
    let min = times.iter().cloned().fold(f64::MAX, f64::min);
    let avg = times.iter().sum::<f64>() / times.len() as f64;
    let max = times.iter().cloned().fold(0.0f64, f64::max);
    (min, avg, max)
}

fn bench_payment() -> ((f64, f64, f64), (f64, f64, f64)) {
    let witness = valid_usdc_same_asset_fixture().witness;

    let mut prove_times = Vec::new();
    let mut verify_times = Vec::new();

    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let result = circuit::prove_payment(&witness).unwrap();
        prove_times.push(duration_to_ms(start.elapsed()));

        let start = Instant::now();
        circuit::verify_payment(&result).unwrap();
        verify_times.push(duration_to_ms(start.elapsed()));
    }

    (stats(&prove_times), stats(&verify_times))
}

fn bench_payment_bundle_mode_a() -> ((f64, f64, f64), (f64, f64, f64)) {
    let fixture = valid_usdc_same_asset_fixture();
    let mut prove_times = Vec::new();
    let mut verify_times = Vec::new();

    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let bundle =
            payment_validation::prove_payment_bundle(&fixture.tx, &fixture.witness, None).unwrap();
        prove_times.push(duration_to_ms(start.elapsed()));

        let start = Instant::now();
        payment_validation::validate_payment_bundle(&fixture.tx, &bundle).unwrap();
        verify_times.push(duration_to_ms(start.elapsed()));
    }

    (stats(&prove_times), stats(&verify_times))
}

fn bench_payment_bundle_mode_b() -> ((f64, f64, f64), (f64, f64, f64)) {
    let fixture = valid_usdc_hush_fee_fixture();
    let mut prove_times = Vec::new();
    let mut verify_times = Vec::new();

    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let bundle = payment_validation::prove_payment_bundle(
            &fixture.tx,
            &fixture.witness,
            fixture.fee_sidecar_witness.as_ref(),
        )
        .unwrap();
        prove_times.push(duration_to_ms(start.elapsed()));

        let start = Instant::now();
        payment_validation::validate_payment_bundle(&fixture.tx, &bundle).unwrap();
        verify_times.push(duration_to_ms(start.elapsed()));
    }

    (stats(&prove_times), stats(&verify_times))
}

fn bench_accounting_accept() -> (f64, f64, f64) {
    let mode_a = valid_usdc_same_asset_fixture();
    let mode_a_bundle =
        payment_validation::prove_payment_bundle(&mode_a.tx, &mode_a.witness, None).unwrap();
    let mode_b = valid_usdc_hush_fee_fixture();
    let mode_b_bundle = payment_validation::prove_payment_bundle(
        &mode_b.tx,
        &mode_b.witness,
        mode_b.fee_sidecar_witness.as_ref(),
    )
    .unwrap();
    let mode_a_record = accepted_payment_record(&mode_a.tx, &mode_a_bundle).unwrap();
    let mode_b_record = accepted_payment_record(&mode_b.tx, &mode_b_bundle).unwrap();
    let action_record = accepted_protocol_action_record(
        &ProtocolActionTx::build(TxKind::ValidatorAction, 900, 9).unwrap(),
    )
    .unwrap();

    let mut times = Vec::new();
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let mut block = BlockAccountingBuilder::new(100, 1);
        block.record_accepted_tx_record(&mode_a_record).unwrap();
        block.record_accepted_tx_record(&mode_b_record).unwrap();
        block.record_accepted_tx_record(&action_record).unwrap();
        let record = block.finalize();
        record.validate().unwrap();
        times.push(duration_to_ms(start.elapsed()));
    }
    stats(&times)
}

fn bench_epoch_accrual() -> (f64, f64, f64) {
    let mode_a = valid_usdc_same_asset_fixture();
    let mode_a_bundle =
        payment_validation::prove_payment_bundle(&mode_a.tx, &mode_a.witness, None).unwrap();
    let mode_b = valid_usdc_hush_fee_fixture();
    let mode_b_bundle = payment_validation::prove_payment_bundle(
        &mode_b.tx,
        &mode_b.witness,
        mode_b.fee_sidecar_witness.as_ref(),
    )
    .unwrap();
    let mode_a_record = accepted_payment_record(&mode_a.tx, &mode_a_bundle).unwrap();
    let mode_b_record = accepted_payment_record(&mode_b.tx, &mode_b_bundle).unwrap();

    let mut blocks = Vec::new();
    for height in 0..4u64 {
        let mut mode_a_record = mode_a_record.clone();
        mode_a_record.tx_id += height * 10 + 1;
        let mut mode_b_record = mode_b_record.clone();
        mode_b_record.tx_id += height * 10 + 2;
        let mut block = BlockAccountingBuilder::new(200 + height, 1);
        block.record_accepted_tx_record(&mode_a_record).unwrap();
        block.record_accepted_tx_record(&mode_b_record).unwrap();
        blocks.push(block.finalize());
    }
    let validators = vec![
        ValidatorStakeInfo { validator_id: 1, payout_key: 101, effective_stake: 100 },
        ValidatorStakeInfo { validator_id: 2, payout_key: 202, effective_stake: 100 },
        ValidatorStakeInfo { validator_id: 3, payout_key: 303, effective_stake: 50 },
    ];
    let participation = vec![
        ValidatorBlockParticipation {
            validator_id: 1,
            signed_block: true,
            liveness_penalty_bps: 0,
            slash_penalty_bps: 0,
        },
        ValidatorBlockParticipation {
            validator_id: 2,
            signed_block: true,
            liveness_penalty_bps: 1_000,
            slash_penalty_bps: 0,
        },
        ValidatorBlockParticipation {
            validator_id: 3,
            signed_block: false,
            liveness_penalty_bps: 0,
            slash_penalty_bps: 0,
        },
    ];

    let mut times = Vec::new();
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let mut epoch = EpochAccumulator::new(9);
        for block in &blocks {
            epoch.apply_block(block, &validators, &participation).unwrap();
        }
        let _settlement = epoch.close().unwrap();
        times.push(duration_to_ms(start.elapsed()));
    }
    stats(&times)
}

fn bench_payout_generation() -> (f64, f64, f64) {
    let mode_a = valid_usdc_same_asset_fixture();
    let mode_a_bundle =
        payment_validation::prove_payment_bundle(&mode_a.tx, &mode_a.witness, None).unwrap();
    let mode_b = valid_usdc_hush_fee_fixture();
    let mode_b_bundle = payment_validation::prove_payment_bundle(
        &mode_b.tx,
        &mode_b.witness,
        mode_b.fee_sidecar_witness.as_ref(),
    )
    .unwrap();
    let mode_a_record = accepted_payment_record(&mode_a.tx, &mode_a_bundle).unwrap();
    let mode_b_record = accepted_payment_record(&mode_b.tx, &mode_b_bundle).unwrap();
    let validators = vec![
        ValidatorStakeInfo { validator_id: 1, payout_key: 101, effective_stake: 100 },
        ValidatorStakeInfo { validator_id: 2, payout_key: 202, effective_stake: 100 },
    ];
    let participation = vec![
        ValidatorBlockParticipation {
            validator_id: 1,
            signed_block: true,
            liveness_penalty_bps: 0,
            slash_penalty_bps: 0,
        },
        ValidatorBlockParticipation {
            validator_id: 2,
            signed_block: true,
            liveness_penalty_bps: 0,
            slash_penalty_bps: 0,
        },
    ];

    let mut times = Vec::new();
    for iteration in 0..ITERATIONS {
        let mut block = BlockAccountingBuilder::new(300 + iteration as u64, 1);
        block.record_accepted_tx_record(&mode_a_record).unwrap();
        block.record_accepted_tx_record(&mode_b_record).unwrap();
        let block = block.finalize();
        let mut epoch = EpochAccumulator::new(12);
        epoch.apply_block(&block, &validators, &participation).unwrap();

        let start = Instant::now();
        let settlement = epoch.close().unwrap();
        let _payout_totals = settlement.total_payouts().unwrap();
        times.push(duration_to_ms(start.elapsed()));
    }
    stats(&times)
}

fn bench_credential_issuance() -> (f64, f64, f64) {
    let issuer_key = M31::from(42u32);
    let issuer_id = poseidon2::derive_issuer_id(issuer_key);
    let subject = poseidon2::derive_owner(M31::from(12345u32));
    let cm =
        poseidon2::credential_commitment(issuer_id, subject, M31::from(2000u32), M31::from(777u32));

    let mut issuer_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
    issuer_tree.set_leaf(0, issuer_id);
    let path_vec = issuer_tree.path(0);
    let mut issuer_path = [(0u32, 0u32); MERKLE_DEPTH];
    for i in 0..MERKLE_DEPTH {
        issuer_path[i] = (path_vec[i].0 .0, path_vec[i].1);
    }

    let witness = credential_issuance::IssuanceWitness {
        issuer_root: issuer_tree.root().0,
        credential_commitment: cm.0,
        issuer_key: 42,
        subject: subject.0,
        expiry: 2000,
        secret: 777,
        issuer_path,
    };

    let mut times = Vec::new();
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        credential_issuance::prove_issuance(&witness).unwrap();
        times.push(duration_to_ms(start.elapsed()));
    }
    stats(&times)
}

fn bench_time_window() -> (f64, f64, f64) {
    let sk = M31::from(12345u32);
    let owner = poseidon2::derive_owner(sk);
    let cred_cm = poseidon2::credential_commitment(
        M31::from(1u32),
        owner,
        M31::from(2000u32),
        M31::from(777u32),
    );

    let mut cred_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
    cred_tree.set_leaf(0, cred_cm);
    let path_vec = cred_tree.path(0);
    let mut cred_path = [(0u32, 0u32); MERKLE_DEPTH];
    for i in 0..MERKLE_DEPTH {
        cred_path[i] = (path_vec[i].0 .0, path_vec[i].1);
    }

    let mut amounts = [0u32; 16];
    let mut timestamps = [0u32; 16];
    amounts[0] = 50000;
    timestamps[0] = 100;
    amounts[1] = 30000;
    timestamps[1] = 200;
    amounts[2] = 20000;
    timestamps[2] = 300;
    amounts[3] = 10000;
    timestamps[3] = 400;

    let witness = time_window::TimeWindowWitness {
        window_start: 50,
        window_end: 500,
        claimed_total: 110000,
        cred_root: cred_tree.root().0,
        epoch: 1000,
        tx_amounts: amounts,
        tx_timestamps: timestamps,
        tx_count: 4,
        sk: 12345,
        cred_issuer: 1,
        cred_expiry: 2000,
        cred_secret: 777,
        cred_path,
    };

    let mut times = Vec::new();
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        time_window::prove_time_window(&witness).unwrap();
        times.push(duration_to_ms(start.elapsed()));
    }
    stats(&times)
}

fn timing_cell(duration_ms: f64) -> String {
    format!("{:>11}", format_duration_ms(duration_ms))
}

fn main() {
    println!("\nHush Network STARK Benchmark Suite");
    println!(
        "Field: Mersenne31 | Prover: Stwo | Depth: {MERKLE_DEPTH} | Iterations: {ITERATIONS}\n"
    );

    let (prove, verify) = bench_payment();
    let (mode_a_bundle_prove, mode_a_bundle_verify) = bench_payment_bundle_mode_a();
    let (mode_b_bundle_prove, mode_b_bundle_verify) = bench_payment_bundle_mode_b();
    let accounting_accept = bench_accounting_accept();
    let epoch_accrual = bench_epoch_accrual();
    let payout_generation = bench_payout_generation();
    let issuance = bench_credential_issuance();
    let tw = bench_time_window();

    println!("| Circuit             | Prove (avg)  | Prove (min)  | Prove (max)  | Verify (avg) |");
    println!("|---------------------|-------------|-------------|-------------|-------------|");
    println!(
        "| {:<19} | {} | {} | {} | {} |",
        "Payment",
        timing_cell(prove.1),
        timing_cell(prove.0),
        timing_cell(prove.2),
        timing_cell(verify.1)
    );
    println!(
        "| {:<19} | {} | {} | {} | {} |",
        "Mode A Bundle",
        timing_cell(mode_a_bundle_prove.1),
        timing_cell(mode_a_bundle_prove.0),
        timing_cell(mode_a_bundle_prove.2),
        timing_cell(mode_a_bundle_verify.1)
    );
    println!(
        "| {:<19} | {} | {} | {} | {} |",
        "Mode B Bundle",
        timing_cell(mode_b_bundle_prove.1),
        timing_cell(mode_b_bundle_prove.0),
        timing_cell(mode_b_bundle_prove.2),
        timing_cell(mode_b_bundle_verify.1)
    );
    println!(
        "| {:<19} | {} | {} | {} | {:>11} |",
        "Credential Issuance",
        timing_cell(issuance.1),
        timing_cell(issuance.0),
        timing_cell(issuance.2),
        "(combined)"
    );
    println!(
        "| {:<19} | {} | {} | {} | {:>11} |",
        "Time-Window Audit",
        timing_cell(tw.1),
        timing_cell(tw.0),
        timing_cell(tw.2),
        "(combined)"
    );
    println!(
        "| {:<19} | {} | {} | {} | {:>11} |",
        "Accounting Accept",
        timing_cell(accounting_accept.1),
        timing_cell(accounting_accept.0),
        timing_cell(accounting_accept.2),
        "(state)"
    );
    println!(
        "| {:<19} | {} | {} | {} | {:>11} |",
        "Epoch Accrual",
        timing_cell(epoch_accrual.1),
        timing_cell(epoch_accrual.0),
        timing_cell(epoch_accrual.2),
        "(state)"
    );
    println!(
        "| {:<19} | {} | {} | {} | {:>11} |",
        "Payout Generation",
        timing_cell(payout_generation.1),
        timing_cell(payout_generation.0),
        timing_cell(payout_generation.2),
        "(state)"
    );
    println!();
    println!(
        "Mode B / Mode A bundle prove ratio: {:.2}x | verify ratio: {:.2}x",
        mode_b_bundle_prove.1 / mode_a_bundle_prove.1,
        mode_b_bundle_verify.1 / mode_a_bundle_verify.1
    );
    println!();
}
