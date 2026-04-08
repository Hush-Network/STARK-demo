//! WASM bindings for browser proving.

use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn wasm_init() {
    #[cfg(feature = "debug_panic")]
    console_error_panic_hook::set_once();
}

use crate::{
    circuit,
    payment_tx::{compute_mode_a_tx_binding_hash, derive_sender_binding_tag, PAYMENT_TX_V1_REPLAY_DOMAIN},
    dual_fee_runtime::{
        dual_fee_review_snapshot, quote_payment, submit_wallet_payment, WalletQuoteRequest,
        WalletSubmissionRequest,
    },
    types::{PaymentWitness, MERKLE_DEPTH},
};

#[wasm_bindgen]
pub struct ProofOutput {
    success: bool,
    message: String,
    prove_time_ms: f64,
    verify_time_ms: f64,
    // Public outputs (populated on success)
    null_0: u32,
    null_1: u32,
    out_cm_0: u32,
    out_cm_1: u32,
    cred_null: u32,
    // Serialized proof for independent verification (base64-encoded JSON)
    proof_bytes: String,
    // Public state used in this proof
    note_root: u32,
    cred_root: u32,
    epoch: u32,
}

#[wasm_bindgen]
impl ProofOutput {
    #[wasm_bindgen(getter)]
    pub fn success(&self) -> bool {
        self.success
    }

    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn prove_time_ms(&self) -> f64 {
        self.prove_time_ms
    }

    #[wasm_bindgen(getter)]
    pub fn verify_time_ms(&self) -> f64 {
        self.verify_time_ms
    }

    #[wasm_bindgen(getter)]
    pub fn null_0(&self) -> u32 {
        self.null_0
    }

    #[wasm_bindgen(getter)]
    pub fn null_1(&self) -> u32 {
        self.null_1
    }

    #[wasm_bindgen(getter)]
    pub fn out_cm_0(&self) -> u32 {
        self.out_cm_0
    }

    #[wasm_bindgen(getter)]
    pub fn out_cm_1(&self) -> u32 {
        self.out_cm_1
    }

    #[wasm_bindgen(getter)]
    pub fn cred_null(&self) -> u32 {
        self.cred_null
    }

    #[wasm_bindgen(getter)]
    pub fn proof_bytes(&self) -> String {
        self.proof_bytes.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn note_root(&self) -> u32 {
        self.note_root
    }

    #[wasm_bindgen(getter)]
    pub fn cred_root(&self) -> u32 {
        self.cred_root
    }

    #[wasm_bindgen(getter)]
    pub fn epoch(&self) -> u32 {
        self.epoch
    }
}

fn parse_merkle_path(flat: &[u32]) -> [(u32, u32); MERKLE_DEPTH] {
    let mut path = [(0u32, 0u32); MERKLE_DEPTH];
    for i in 0..MERKLE_DEPTH {
        path[i] = (flat[2 * i], flat[2 * i + 1]);
    }
    path
}

fn error_output(message: String, prove_time_ms: f64) -> ProofOutput {
    ProofOutput {
        success: false,
        message,
        prove_time_ms,
        verify_time_ms: 0.0,
        null_0: 0,
        null_1: 0,
        out_cm_0: 0,
        out_cm_1: 0,
        cred_null: 0,
        proof_bytes: String::new(),
        note_root: 0,
        cred_root: 0,
        epoch: 0,
    }
}

fn json_result<T: Serialize>(result: Result<T, String>) -> String {
    match result {
        Ok(data) => serde_json::json!({ "ok": true, "data": data }).to_string(),
        Err(error) => serde_json::json!({ "ok": false, "error": error }).to_string(),
    }
}

#[wasm_bindgen]
pub fn dual_fee_review_json() -> String {
    serde_json::json!({
        "ok": true,
        "data": dual_fee_review_snapshot(),
    })
    .to_string()
}

#[wasm_bindgen]
pub fn dual_fee_quote_payment_json(payment_asset: u32, fee_asset: u32, amount: f64) -> String {
    json_result(quote_payment(&WalletQuoteRequest {
        payment_asset,
        fee_asset,
        amount: amount as u64,
    }))
}

#[wasm_bindgen]
pub fn dual_fee_submit_demo_payment_json(
    payment_asset: u32,
    fee_asset: u32,
    amount: f64,
    recipient_owner: u32,
    payment_balance: f64,
    hush_balance: f64,
    credential_expiry: u32,
) -> String {
    json_result(submit_wallet_payment(&WalletSubmissionRequest {
        payment_asset,
        fee_asset,
        amount: amount as u64,
        recipient_owner,
        payment_balance: payment_balance as u64,
        hush_balance: hush_balance as u64,
        credential_expiry: (credential_expiry != 0).then_some(credential_expiry),
    }))
}

#[wasm_bindgen]
pub fn prove_and_verify(
    epoch: u32,
    note_root: u32,
    cred_root: u32,
    sk: u32,
    in_asset: u32,
    in_amt_0: f64,
    in_rand_0: u32,
    in_amt_1: f64,
    in_rand_1: u32,
    out_amt_0: f64,
    out_owner_0: u32,
    out_rand_0: u32,
    out_amt_1: f64,
    out_rand_1: u32,
    cred_issuer: u32,
    cred_expiry: u32,
    cred_secret: u32,
    note_path_0_flat: &[u32],
    note_path_1_flat: &[u32],
    cred_path_flat: &[u32],
) -> ProofOutput {
    if note_path_0_flat.len() != 2 * MERKLE_DEPTH
        || note_path_1_flat.len() != 2 * MERKLE_DEPTH
        || cred_path_flat.len() != 2 * MERKLE_DEPTH
    {
        return error_output(
            format!("Merkle paths must each have {} elements", 2 * MERKLE_DEPTH),
            0.0,
        );
    }

    let in_amt_0 = in_amt_0 as u64;
    let in_amt_1 = in_amt_1 as u64;
    let out_amt_0 = out_amt_0 as u64;
    let out_amt_1 = out_amt_1 as u64;

    let tx_binding_hash = compute_mode_a_tx_binding_hash(
        PAYMENT_TX_V1_REPLAY_DOMAIN,
        in_asset,
        in_asset,
        1,
        0,
        1,
        out_amt_0,
        out_owner_0,
        out_rand_0,
        out_amt_1,
        out_rand_1,
    );
    let witness = PaymentWitness {
        epoch,
        note_root,
        cred_root,
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
        note_path_0: parse_merkle_path(note_path_0_flat),
        note_path_1: parse_merkle_path(note_path_1_flat),
        cred_path: parse_merkle_path(cred_path_flat),
    };

    // Prove
    let prove_start = js_sys::Date::now();
    let proof_result = match circuit::prove_payment(&witness) {
        Ok(r) => r,
        Err(e) => {
            return error_output(e, js_sys::Date::now() - prove_start);
        }
    };
    let prove_time = js_sys::Date::now() - prove_start;

    let pd = &proof_result.public_data;
    let null_0 = pd.null_0;
    let null_1 = pd.null_1;
    let out_cm_0 = pd.out_cm_0;
    let out_cm_1 = pd.out_cm_1;
    let cred_null = pd.cred_null;

    // Serialize proof for independent verification
    let serialized = serde_json::to_string(&proof_result.proof)
        .unwrap_or_else(|_| String::new());
    let proof_bytes = use_base64_encode(&serialized);

    // Verify
    let verify_start = js_sys::Date::now();
    match circuit::verify_payment(&proof_result) {
        Ok(()) => ProofOutput {
            success: true,
            message: "STARK proof verified successfully".to_string(),
            prove_time_ms: prove_time,
            verify_time_ms: js_sys::Date::now() - verify_start,
            null_0,
            null_1,
            out_cm_0,
            out_cm_1,
            cred_null,
            proof_bytes,
            note_root: witness.note_root,
            cred_root: witness.cred_root,
            epoch: witness.epoch,
        },
        Err(e) => ProofOutput {
            success: false,
            message: format!("Proof generated but verification failed: {e}"),
            prove_time_ms: prove_time,
            verify_time_ms: js_sys::Date::now() - verify_start,
            null_0: 0,
            null_1: 0,
            out_cm_0: 0,
            out_cm_1: 0,
            cred_null: 0,
            proof_bytes: String::new(),
            note_root: 0,
            cred_root: 0,
            epoch: 0,
        },
    }
}

fn use_base64_encode(s: &str) -> String {
    use std::fmt::Write;
    let bytes = s.as_bytes();
    let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut i = 0;
    while i + 2 < bytes.len() {
        let b0 = bytes[i] as usize;
        let b1 = bytes[i + 1] as usize;
        let b2 = bytes[i + 2] as usize;
        let _ = out.write_char(TABLE[b0 >> 2] as char);
        let _ = out.write_char(TABLE[((b0 & 3) << 4) | (b1 >> 4)] as char);
        let _ = out.write_char(TABLE[((b1 & 0xf) << 2) | (b2 >> 6)] as char);
        let _ = out.write_char(TABLE[b2 & 0x3f] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let b0 = bytes[i] as usize;
        let _ = out.write_char(TABLE[b0 >> 2] as char);
        let _ = out.write_char(TABLE[(b0 & 3) << 4] as char);
        out.push_str("==");
    } else if rem == 2 {
        let b0 = bytes[i] as usize;
        let b1 = bytes[i + 1] as usize;
        let _ = out.write_char(TABLE[b0 >> 2] as char);
        let _ = out.write_char(TABLE[((b0 & 3) << 4) | (b1 >> 4)] as char);
        let _ = out.write_char(TABLE[(b1 & 0xf) << 2] as char);
        out.push('=');
    }
    out
}

/// Simple result type for audit proofs.
#[wasm_bindgen]
pub struct AuditOutput {
    success: bool,
    message: String,
    prove_time_ms: f64,
}

#[wasm_bindgen]
impl AuditOutput {
    #[wasm_bindgen(getter)]
    pub fn success(&self) -> bool { self.success }
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String { self.message.clone() }
    #[wasm_bindgen(getter)]
    pub fn prove_time_ms(&self) -> f64 { self.prove_time_ms }
}

/// Proves a time-window audit for the browser demo.
#[wasm_bindgen]
pub fn prove_time_window_audit(
    window_start: u32,
    window_end: u32,
    amounts: &[u32],
    timestamps: &[u32],
    sk: u32,
    cred_issuer: u32,
    cred_expiry: u32,
    cred_secret: u32,
) -> AuditOutput {
    use stwo::core::fields::m31::M31;
    use crate::{poseidon2, time_window};
    use crate::types::MERKLE_DEPTH;

    const MAX_TX: usize = 16;

    // Build credential tree
    let owner = poseidon2::derive_owner(M31::from(sk));
    let cred_cm = poseidon2::credential_commitment(
        M31::from(cred_issuer), owner, M31::from(cred_expiry), M31::from(cred_secret),
    );
    let mut cred_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
    cred_tree.set_leaf(0, cred_cm);
    let cred_root = cred_tree.root().0;

    let cred_path_pairs = cred_tree.path(0);
    let mut cred_path = [(0u32, 0u32); MERKLE_DEPTH];
    for (i, (sib, dir)) in cred_path_pairs.iter().enumerate() {
        cred_path[i] = (sib.0, *dir);
    }

    // Fill tx arrays (up to MAX_TX, pad with zeros)
    let tx_count = amounts.len().min(MAX_TX);
    let mut tx_amounts = [0u32; MAX_TX];
    let mut tx_timestamps = [0u32; MAX_TX];
    for i in 0..tx_count {
        tx_amounts[i] = amounts[i];
        tx_timestamps[i] = if i < timestamps.len() { timestamps[i] } else { window_start };
    }

    let claimed_total: u32 = tx_amounts[..tx_count].iter().sum();

    let witness = time_window::TimeWindowWitness {
        window_start,
        window_end,
        claimed_total,
        cred_root,
        epoch: 1000,
        tx_amounts,
        tx_timestamps,
        tx_count,
        sk,
        cred_issuer,
        cred_expiry,
        cred_secret,
        cred_path,
    };

    let prove_start = js_sys::Date::now();
    match time_window::prove_time_window(&witness) {
        Ok(()) => AuditOutput {
            success: true,
            message: "Time-window audit proof verified".to_string(),
            prove_time_ms: js_sys::Date::now() - prove_start,
        },
        Err(e) => AuditOutput {
            success: false,
            message: e,
            prove_time_ms: js_sys::Date::now() - prove_start,
        },
    }
}

/// High-level wrapper for the browser demo: takes simple payment parameters,
/// computes randomness, builds Merkle trees and paths internally, proves and verifies.
/// Returns a ProofOutput including proof_bytes for independent verification.
#[wasm_bindgen]
pub fn build_witness_and_prove(
    epoch: u32,
    sk: u32,
    in_asset: u32,
    in_amt_0: f64,
    in_amt_1: f64,
    out_amt_0: f64,
    out_owner_0: u32,
    out_amt_1: f64,
    cred_issuer: u32,
    cred_expiry: u32,
    cred_secret: u32,
) -> ProofOutput {
    use stwo::core::fields::m31::M31;
    use crate::poseidon2;

    let in_amt_0 = in_amt_0 as u64;
    let in_amt_1 = in_amt_1 as u64;
    let out_amt_0 = out_amt_0 as u64;
    let out_amt_1 = out_amt_1 as u64;

    // Fixed demo randomness (not secret — demo only)
    let in_rand_0: u32 = 42;
    let in_rand_1: u32 = 43;
    let out_rand_0: u32 = 44;
    let out_rand_1: u32 = 45;

    // Derive owner
    let owner = poseidon2::derive_owner(M31::from(sk));
    let asset = M31::from(in_asset);

    // Compute note commitments (7-input: asset, a0, a1, a2, a3, owner, randomness)
    let cm0 = poseidon2::note_commitment_u64(asset, in_amt_0, owner, M31::from(in_rand_0));
    let cm1 = poseidon2::note_commitment_u64(asset, in_amt_1, owner, M31::from(in_rand_1));

    // Build note Merkle tree and extract paths
    let mut note_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
    note_tree.set_leaf(0, cm0);
    note_tree.set_leaf(1, cm1);
    let note_root = note_tree.root().0;

    let note_path_0_pairs = note_tree.path(0);
    let note_path_1_pairs = note_tree.path(1);

    let mut note_path_0 = [(0u32, 0u32); MERKLE_DEPTH];
    for (i, (sib, dir)) in note_path_0_pairs.iter().enumerate() {
        note_path_0[i] = (sib.0, *dir);
    }
    let mut note_path_1 = [(0u32, 0u32); MERKLE_DEPTH];
    for (i, (sib, dir)) in note_path_1_pairs.iter().enumerate() {
        note_path_1[i] = (sib.0, *dir);
    }

    // Compute credential commitment and tree
    let cred_cm = poseidon2::credential_commitment(
        M31::from(cred_issuer),
        owner,
        M31::from(cred_expiry),
        M31::from(cred_secret),
    );
    let mut cred_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
    cred_tree.set_leaf(0, cred_cm);
    let cred_root = cred_tree.root().0;

    let cred_path_pairs = cred_tree.path(0);
    let mut cred_path = [(0u32, 0u32); MERKLE_DEPTH];
    for (i, (sib, dir)) in cred_path_pairs.iter().enumerate() {
        cred_path[i] = (sib.0, *dir);
    }

    // Assemble witness (u64 amounts passed to binding hash)
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
    let witness = crate::types::PaymentWitness {
        epoch,
        note_root,
        cred_root,
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
    };

    // Prove
    let prove_start = js_sys::Date::now();
    let proof_result = match circuit::prove_payment(&witness) {
        Ok(r) => r,
        Err(e) => return error_output(e, js_sys::Date::now() - prove_start),
    };
    let prove_time = js_sys::Date::now() - prove_start;

    let pd = &proof_result.public_data;
    let null_0 = pd.null_0;
    let null_1 = pd.null_1;
    let out_cm_0 = pd.out_cm_0;
    let out_cm_1 = pd.out_cm_1;
    let cred_null = pd.cred_null;

    // Serialize proof for independent verification
    let serialized = serde_json::to_string(&proof_result.proof)
        .unwrap_or_else(|_| String::new());
    let proof_bytes = use_base64_encode(&serialized);

    // Verify
    let verify_start = js_sys::Date::now();
    match circuit::verify_payment(&proof_result) {
        Ok(()) => ProofOutput {
            success: true,
            message: "STARK proof verified successfully".to_string(),
            prove_time_ms: prove_time,
            verify_time_ms: js_sys::Date::now() - verify_start,
            null_0,
            null_1,
            out_cm_0,
            out_cm_1,
            cred_null,
            proof_bytes,
            note_root: witness.note_root,
            cred_root: witness.cred_root,
            epoch: witness.epoch,
        },
        Err(e) => ProofOutput {
            success: false,
            message: format!("Proof generated but verification failed: {e}"),
            prove_time_ms: prove_time,
            verify_time_ms: js_sys::Date::now() - verify_start,
            null_0: 0,
            null_1: 0,
            out_cm_0: 0,
            out_cm_1: 0,
            cred_null: 0,
            proof_bytes: String::new(),
            note_root: 0,
            cred_root: 0,
            epoch: 0,
        },
    }
}

/// Verify a serialized STARK proof against its public outputs.
/// proof_b64: base64-encoded JSON of the serialized StarkProof.
/// Returns a JS string: "ok" on success, error message on failure.
#[wasm_bindgen]
pub fn verify_serialized_proof(
    proof_b64: &str,
    note_root: u32,
    cred_root: u32,
    epoch: u32,
    null_0: u32,
    null_1: u32,
    out_cm_0: u32,
    out_cm_1: u32,
    cred_null: u32,
    tx_binding_hash: u32,
    sender_binding_tag: u32,
) -> String {
    use num_traits::Zero;
    use stwo::core::fields::qm31::QM31;
    use stwo::prover::backend::simd::m31::LOG_N_LANES;
    use stwo_constraint_framework::{FrameworkComponent, TraceLocationAllocator};
    use crate::circuit::{HushPaymentEval, PaymentPublicData, ProofResult};
    use crate::prover_common::ProverMerkleHasher;

    // Decode base64
    let json_bytes = match base64_decode(proof_b64) {
        Ok(b) => b,
        Err(e) => return format!("base64 decode error: {e}"),
    };
    let json_str = match std::str::from_utf8(&json_bytes) {
        Ok(s) => s,
        Err(e) => return format!("utf8 decode error: {e}"),
    };

    // Deserialize proof
    let proof: stwo::core::proof::StarkProof<ProverMerkleHasher> =
        match serde_json::from_str(json_str) {
            Ok(p) => p,
            Err(e) => return format!("proof deserialization error: {e}"),
        };

    let public_data = PaymentPublicData {
        epoch,
        note_root,
        cred_root,
        tx_binding_hash,
        sender_binding_tag,
        null_0,
        null_1,
        out_cm_0,
        out_cm_1,
        cred_null,
    };

    // Reconstruct the component with the same log_num_rows as used during proving
    let log_num_rows = LOG_N_LANES;
    let component = FrameworkComponent::<HushPaymentEval>::new(
        &mut TraceLocationAllocator::default(),
        HushPaymentEval { log_size: log_num_rows },
        QM31::zero(),
    );

    let proof_result = ProofResult { proof, component, public_data, log_num_rows };

    match circuit::verify_payment(&proof_result) {
        Ok(()) => "ok".to_string(),
        Err(e) => format!("verification failed: {e}"),
    }
}

fn base64_decode(s: &str) -> Result<Vec<u8>, &'static str> {
    let s = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    const TABLE: [u8; 256] = {
        let mut t = [255u8; 256];
        let mut i = 0u8;
        while i < 26 { t[(b'A' + i) as usize] = i; t[(b'a' + i) as usize] = 26 + i; i += 1; }
        let mut i = 0u8;
        while i < 10 { t[(b'0' + i) as usize] = 52 + i; i += 1; }
        t[b'+' as usize] = 62; t[b'/' as usize] = 63; t[b'=' as usize] = 0;
        t
    };
    let mut i = 0;
    while i + 3 < s.len() {
        let (a, b, c, d) = (TABLE[s[i] as usize], TABLE[s[i+1] as usize], TABLE[s[i+2] as usize], TABLE[s[i+3] as usize]);
        if a == 255 || b == 255 { return Err("invalid base64"); }
        out.push((a << 2) | (b >> 4));
        if s[i+2] != b'=' { out.push((b << 4) | (c >> 2)); }
        if s[i+3] != b'=' { out.push((c << 6) | d); }
        i += 4;
    }
    Ok(out)
}

#[wasm_bindgen]
pub fn compute_credential_root(sk: u32, issuer: u32, expiry: u32, secret: u32) -> u32 {
    use stwo::core::fields::m31::M31;

    use crate::poseidon2;

    let owner = poseidon2::derive_owner(M31::from(sk));
    let cred_cm = poseidon2::credential_commitment(
        M31::from(issuer),
        owner,
        M31::from(expiry),
        M31::from(secret),
    );
    let mut tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
    tree.set_leaf(0, cred_cm);
    tree.root().0
}

#[wasm_bindgen]
pub fn compute_note_root(
    sk: u32,
    in_asset: u32,
    in_amt_0: f64,
    in_rand_0: u32,
    in_amt_1: f64,
    in_rand_1: u32,
) -> u32 {
    use stwo::core::fields::m31::M31;

    use crate::poseidon2;

    let in_amt_0 = in_amt_0 as u64;
    let in_amt_1 = in_amt_1 as u64;

    let owner = poseidon2::derive_owner(M31::from(sk));
    let asset = M31::from(in_asset);
    let cm0 = poseidon2::note_commitment_u64(asset, in_amt_0, owner, M31::from(in_rand_0));
    let cm1 = poseidon2::note_commitment_u64(asset, in_amt_1, owner, M31::from(in_rand_1));
    let mut tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
    tree.set_leaf(0, cm0);
    tree.set_leaf(1, cm1);
    tree.root().0
}

#[wasm_bindgen]
pub fn compute_merkle_path(leaf_index: usize, leaf_values_flat: &[u32]) -> Vec<u32> {
    use stwo::core::fields::m31::M31;

    use crate::poseidon2;

    let leaves: Vec<M31> = leaf_values_flat.iter().map(|&v| M31::from(v)).collect();
    let tree = poseidon2::build_merkle_tree(&leaves);
    let path = poseidon2::merkle_path(&tree, leaf_index);
    let mut flat = Vec::with_capacity(2 * MERKLE_DEPTH);
    for (sibling, dir) in &path {
        flat.push(sibling.0);
        flat.push(*dir);
    }
    flat
}
