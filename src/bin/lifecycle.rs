// Full protocol lifecycle: issuance -> payment -> second spend -> audit -> double-spend rejection.
// Demonstrates all three circuits composing into a coherent ledger model.
// Run: cargo run --bin lifecycle --release

use std::collections::HashSet;

use hush_demo_stark::{
    circuit, credential_issuance, poseidon2, time_window,
    payment_tx::{compute_mode_a_tx_binding_hash, derive_sender_binding_tag, PAYMENT_TX_V1_REPLAY_DOMAIN},
    types::{PaymentWitness, MERKLE_DEPTH},
};
use stwo::core::fields::m31::M31;

struct LedgerState {
    note_tree: poseidon2::SparseMerkleTree,
    cred_tree: poseidon2::SparseMerkleTree,
    issuer_tree: poseidon2::SparseMerkleTree,
    nullifier_set: HashSet<u32>,
    cred_nullifier_set: HashSet<u32>,
    next_note_idx: usize,
}

impl LedgerState {
    fn new() -> Self {
        LedgerState {
            note_tree: poseidon2::SparseMerkleTree::new(MERKLE_DEPTH),
            cred_tree: poseidon2::SparseMerkleTree::new(MERKLE_DEPTH),
            issuer_tree: poseidon2::SparseMerkleTree::new(MERKLE_DEPTH),
            nullifier_set: HashSet::new(),
            cred_nullifier_set: HashSet::new(),
            next_note_idx: 0,
        }
    }

    fn add_note(&mut self, commitment: M31) -> usize {
        let idx = self.next_note_idx;
        self.note_tree.set_leaf(idx, commitment);
        self.next_note_idx += 1;
        idx
    }

    fn add_credential(&mut self, index: usize, commitment: M31) {
        self.cred_tree.set_leaf(index, commitment);
    }

    fn add_issuer(&mut self, index: usize, issuer_id: M31) {
        self.issuer_tree.set_leaf(index, issuer_id);
    }

    fn apply_payment(&mut self, public_data: &circuit::PaymentPublicData) -> Result<(), String> {
        if public_data.note_root != self.note_tree.root().0 {
            return Err("Note root mismatch".to_string());
        }
        if public_data.cred_root != self.cred_tree.root().0 {
            return Err("Credential root mismatch".to_string());
        }
        if self.nullifier_set.contains(&public_data.null_0) {
            return Err(format!("Double-spend: nullifier {} already spent", public_data.null_0));
        }
        if self.nullifier_set.contains(&public_data.null_1) {
            return Err(format!("Double-spend: nullifier {} already spent", public_data.null_1));
        }
        if self.cred_nullifier_set.contains(&public_data.cred_null) {
            return Err(format!(
                "Credential nullifier {} already used this epoch",
                public_data.cred_null
            ));
        }
        self.nullifier_set.insert(public_data.null_0);
        self.nullifier_set.insert(public_data.null_1);
        self.cred_nullifier_set.insert(public_data.cred_null);
        Ok(())
    }
}

fn build_payment_witness(
    ledger: &LedgerState,
    epoch: u32,
    sk: u32,
    in_asset: u32,
    in_idx_0: usize,
    in_amt_0: u64,
    in_rand_0: u32,
    in_idx_1: usize,
    in_amt_1: u64,
    in_rand_1: u32,
    out_amt_0: u64,
    out_owner_0: u32,
    out_rand_0: u32,
    out_amt_1: u64,
    out_rand_1: u32,
    cred_issuer: u32,
    cred_expiry: u32,
    cred_secret: u32,
    cred_idx: usize,
) -> PaymentWitness {
    let note_path_0_vec = ledger.note_tree.path(in_idx_0);
    let note_path_1_vec = ledger.note_tree.path(in_idx_1);
    let cred_path_vec = ledger.cred_tree.path(cred_idx);

    let mut note_path_0 = [(0u32, 0u32); MERKLE_DEPTH];
    let mut note_path_1 = [(0u32, 0u32); MERKLE_DEPTH];
    let mut cred_path = [(0u32, 0u32); MERKLE_DEPTH];
    for i in 0..MERKLE_DEPTH {
        note_path_0[i] = (note_path_0_vec[i].0 .0, note_path_0_vec[i].1);
        note_path_1[i] = (note_path_1_vec[i].0 .0, note_path_1_vec[i].1);
        cred_path[i] = (cred_path_vec[i].0 .0, cred_path_vec[i].1);
    }

    let tx_binding_hash = compute_mode_a_tx_binding_hash(
        PAYMENT_TX_V1_REPLAY_DOMAIN,
        in_asset,
        in_asset,
        1,
        0u64,
        1,
        out_amt_0,
        out_owner_0,
        out_rand_0,
        out_amt_1,
        out_rand_1,
    );

    PaymentWitness {
        epoch,
        note_root: ledger.note_tree.root().0,
        cred_root: ledger.cred_tree.root().0,
        sk,
        in_asset,
        in_amt_0,
        in_rand_0,
        in_amt_1,
        in_rand_1,
        out_amt_0,
        out_owner_0,
        out_rand_0,
        out_amt_1,
        out_rand_1,
        payment_fee_amount: 0,
        binding_fee_asset: in_asset,
        fee_amount: 0,
        fee_class: 1,
        replay_domain: PAYMENT_TX_V1_REPLAY_DOMAIN,
        tx_binding_hash,
        sender_binding_tag: derive_sender_binding_tag(sk, tx_binding_hash),
        cred_issuer,
        cred_expiry,
        cred_secret,
        note_path_0,
        note_path_1,
        cred_path,
    }
}

fn main() {
    println!("\n=== Hush Network: Full Protocol Lifecycle ===");
    println!("Three circuits: Credential Issuance + Payment + Time-Window Audit");
    println!("Merkle depth: {} ({} leaves)\n", MERKLE_DEPTH, 1u64 << MERKLE_DEPTH);

    let mut ledger = LedgerState::new();
    let epoch = 1000u32;

    // --- Step 1: Register authorized issuer ---
    println!("Step 1: Register authorized issuer");
    let issuer_key = 42u32;
    let issuer_id = poseidon2::derive_issuer_id(M31::from(issuer_key));
    ledger.add_issuer(0, issuer_id);
    println!("  Issuer ID: {} (derived from key)", issuer_id.0);
    println!("  Issuer tree root: {}", ledger.issuer_tree.root().0);

    // --- Step 2: Credential issuance (CIRCUIT 1) ---
    println!("\nStep 2: Credential issuance circuit");
    let alice_sk = 12345u32;
    let alice_owner = poseidon2::derive_owner(M31::from(alice_sk));
    let cred_expiry = 2000u32;
    let cred_secret = 777u32;
    let cred_cm = poseidon2::credential_commitment(
        issuer_id,
        alice_owner,
        M31::from(cred_expiry),
        M31::from(cred_secret),
    );

    let issuer_path_vec = ledger.issuer_tree.path(0);
    let mut issuer_path = [(0u32, 0u32); MERKLE_DEPTH];
    for i in 0..MERKLE_DEPTH {
        issuer_path[i] = (issuer_path_vec[i].0 .0, issuer_path_vec[i].1);
    }

    let issuance_witness = credential_issuance::IssuanceWitness {
        issuer_root: ledger.issuer_tree.root().0,
        credential_commitment: cred_cm.0,
        issuer_key,
        subject: alice_owner.0,
        expiry: cred_expiry,
        secret: cred_secret,
        issuer_path,
    };

    print!("  Proving credential issuance... ");
    let start = std::time::Instant::now();
    credential_issuance::prove_issuance(&issuance_witness).expect("Issuance proof failed");
    println!("done ({} ms, proved + verified)", start.elapsed().as_millis());
    println!("  Credential commitment: {}", cred_cm.0);

    ledger.add_credential(0, cred_cm);
    println!("  Credential registered in ledger (cred tree root: {})", ledger.cred_tree.root().0);

    // --- Step 3: Create initial notes for Alice ---
    println!("\nStep 3: Create initial notes for Alice");
    let asset = M31::from(1u32);
    let note_0 =
        poseidon2::note_commitment_u64(asset, 7000u64, alice_owner, M31::from(111u32));
    let note_1 =
        poseidon2::note_commitment_u64(asset, 3000u64, alice_owner, M31::from(222u32));
    let idx_0 = ledger.add_note(note_0);
    let idx_1 = ledger.add_note(note_1);
    println!("  Note {}: 7000 units (cm: {})", idx_0, note_0.0);
    println!("  Note {}: 3000 units (cm: {})", idx_1, note_1.0);

    // --- Step 4: Payment (CIRCUIT 2) - Alice sends 8000 to Bob ---
    println!("\nStep 4: Payment circuit (7000 + 3000 -> 8000 to Bob + 2000 change)");
    let bob_owner = 99999u32;
    let witness_1 = build_payment_witness(
        &ledger,
        epoch,
        alice_sk,
        1,
        idx_0,
        7000,
        111,
        idx_1,
        3000,
        222,
        8000,
        bob_owner,
        333,
        2000,
        444,
        issuer_id.0,
        cred_expiry,
        cred_secret,
        0,
    );

    print!("  Proving payment... ");
    let start = std::time::Instant::now();
    let result_1 = circuit::prove_payment(&witness_1).expect("Payment proof failed");
    let prove_ms = start.elapsed().as_millis();
    print!("done ({prove_ms} ms). Verifying... ");
    let start = std::time::Instant::now();
    circuit::verify_payment(&result_1).expect("Payment verification failed");
    println!("done ({} ms)", start.elapsed().as_millis());

    println!("  Public outputs:");
    println!("    Nullifier 0: {}", result_1.public_data.null_0);
    println!("    Nullifier 1: {}", result_1.public_data.null_1);
    println!("    Output cm 0 (Bob): {}", result_1.public_data.out_cm_0);
    println!("    Output cm 1 (change): {}", result_1.public_data.out_cm_1);
    println!("    Credential nullifier: {}", result_1.public_data.cred_null);

    ledger.apply_payment(&result_1.public_data).expect("Ledger rejected payment");

    // Insert output commitments as new notes
    let change_idx = ledger.add_note(M31::from(result_1.public_data.out_cm_1));
    let bob_note_idx = ledger.add_note(M31::from(result_1.public_data.out_cm_0));
    println!(
        "  Ledger updated: {} nullifiers, notes at indices {}, {}",
        ledger.nullifier_set.len(),
        change_idx,
        bob_note_idx
    );

    // --- Step 5: Second payment (Alice spends her change) ---
    // Alice's change note is at change_idx. She needs a second note — use a zero-value dummy.
    println!("\nStep 5: Second payment (Alice spends 2000 change -> 1500 + 500)");
    let dummy_note =
        poseidon2::note_commitment_u64(asset, 0u64, alice_owner, M31::from(555u32));
    let dummy_idx = ledger.add_note(dummy_note);

    // Reset credential nullifier set for new epoch
    ledger.cred_nullifier_set.clear();
    let epoch_2 = 1001u32;

    let witness_2 = build_payment_witness(
        &ledger,
        epoch_2,
        alice_sk,
        1,
        change_idx,
        2000,
        444,
        dummy_idx,
        0,
        555,
        1500,
        bob_owner,
        666,
        500,
        777,
        issuer_id.0,
        cred_expiry,
        cred_secret,
        0,
    );

    print!("  Proving second payment... ");
    let start = std::time::Instant::now();
    let result_2 = circuit::prove_payment(&witness_2).expect("Second payment proof failed");
    let prove_ms = start.elapsed().as_millis();
    print!("done ({prove_ms} ms). Verifying... ");
    let start = std::time::Instant::now();
    circuit::verify_payment(&result_2).expect("Second payment verification failed");
    println!("done ({} ms)", start.elapsed().as_millis());
    ledger.apply_payment(&result_2.public_data).expect("Ledger rejected second payment");
    println!("  Second spend accepted. Nullifiers: {}", ledger.nullifier_set.len());

    // --- Step 6: Time-window audit (CIRCUIT 3) ---
    println!("\nStep 6: Time-window audit circuit");
    println!("  Alice proves aggregate spend over epoch window without revealing individual txs");

    let cred_path_vec = ledger.cred_tree.path(0);
    let mut cred_path = [(0u32, 0u32); MERKLE_DEPTH];
    for i in 0..MERKLE_DEPTH {
        cred_path[i] = (cred_path_vec[i].0 .0, cred_path_vec[i].1);
    }

    let mut amounts = [0u32; 16];
    let mut timestamps = [0u32; 16];
    amounts[0] = 8000;
    timestamps[0] = epoch; // First payment output
    amounts[1] = 1500;
    timestamps[1] = epoch_2; // Second payment output
    let claimed_total = 9500;

    let tw_witness = time_window::TimeWindowWitness {
        window_start: 999,
        window_end: 1002,
        claimed_total,
        cred_root: ledger.cred_tree.root().0,
        epoch: epoch_2,
        tx_amounts: amounts,
        tx_timestamps: timestamps,
        tx_count: 2,
        sk: alice_sk,
        cred_issuer: issuer_id.0,
        cred_expiry,
        cred_secret,
        cred_path,
    };

    print!("  Proving time-window audit... ");
    let start = std::time::Instant::now();
    time_window::prove_time_window(&tw_witness).expect("Time-window proof failed");
    println!("done ({} ms, proved + verified)", start.elapsed().as_millis());
    println!("  Proved: total spend = {} over epochs [{}, {}]", claimed_total, 999, 1002);

    // --- Step 7: Double-spend rejection ---
    println!("\nStep 7: Attempting double-spend of first payment");
    match ledger.apply_payment(&result_1.public_data) {
        Err(e) => println!("  REJECTED: {e}"),
        Ok(()) => panic!("Double-spend should have been rejected"),
    }

    println!("\n=== Protocol Lifecycle Complete ===");
    println!("Demonstrated:");
    println!("  1. Credential issuance (issuer authorization via Merkle proof)");
    println!("  2. First payment (credential-gated, 2-in-2-out)");
    println!("  3. Second payment (spending change output, state continuity)");
    println!("  4. Time-window audit (aggregate disclosure without individual tx reveal)");
    println!("  5. Double-spend rejection (nullifier set enforcement)");
    println!(
        "All three circuits composed. {} total nullifiers tracked.\n",
        ledger.nullifier_set.len()
    );
}
