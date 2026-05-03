//! WASM bindings for browser proving.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn wasm_init() {
    #[cfg(feature = "debug_panic")]
    console_error_panic_hook::set_once();
}

/// Validate an f64 value from JavaScript before casting to u64.
/// Rejects NaN, infinity, negative values, non-integers, and values
/// above Number.MAX_SAFE_INTEGER (2^53 - 1) where f64 loses precision.
fn validate_f64_amount(v: f64, name: &str) -> Result<u64, String> {
    if !v.is_finite() {
        return Err(format!("{name} is not finite"));
    }
    if v < 0.0 {
        return Err(format!("{name} is negative"));
    }
    if v != v.floor() {
        return Err(format!("{name} is not an integer"));
    }
    if v > 9_007_199_254_740_991.0 {
        return Err(format!("{name} exceeds safe integer range"));
    }
    Ok(v as u64)
}

use crate::{
    circuit,
    dual_fee_runtime::{
        quote_payment, submit_wallet_payment, WalletQuoteRequest, WalletSubmissionRequest,
    },
    payment_tx::compute_mode_a_tx_binding_hash,
    provenance_attestation,
};

#[wasm_bindgen]
pub struct ProofOutput {
    success: bool,
    message: String,
    prove_time_ms: f64,
    verify_time_ms: f64,
    // Public outputs (populated on success) - HashOut as [u32; 4]
    null_0: [u32; 4],
    null_1: [u32; 4],
    out_cm_0: [u32; 4],
    out_cm_1: [u32; 4],
    // Serialized proof for independent verification (base64-encoded JSON)
    proof_bytes: String,
    // Public state used in this proof
    note_root: [u32; 4],
    accumulator_root: [u32; 4],
    epoch: u32,
    // Trace shape needed for independent verification
    log_num_rows: u32,
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
    pub fn null_0(&self) -> String {
        format!(
            "{:08x}{:08x}{:08x}{:08x}",
            self.null_0[0], self.null_0[1], self.null_0[2], self.null_0[3]
        )
    }

    #[wasm_bindgen(getter)]
    pub fn null_1(&self) -> String {
        format!(
            "{:08x}{:08x}{:08x}{:08x}",
            self.null_1[0], self.null_1[1], self.null_1[2], self.null_1[3]
        )
    }

    #[wasm_bindgen(getter)]
    pub fn out_cm_0(&self) -> String {
        format!(
            "{:08x}{:08x}{:08x}{:08x}",
            self.out_cm_0[0], self.out_cm_0[1], self.out_cm_0[2], self.out_cm_0[3]
        )
    }

    #[wasm_bindgen(getter)]
    pub fn out_cm_1(&self) -> String {
        format!(
            "{:08x}{:08x}{:08x}{:08x}",
            self.out_cm_1[0], self.out_cm_1[1], self.out_cm_1[2], self.out_cm_1[3]
        )
    }

    #[wasm_bindgen(getter)]
    pub fn proof_bytes(&self) -> String {
        self.proof_bytes.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn note_root(&self) -> String {
        format!(
            "{:08x}{:08x}{:08x}{:08x}",
            self.note_root[0], self.note_root[1], self.note_root[2], self.note_root[3]
        )
    }

    #[wasm_bindgen(getter)]
    pub fn accumulator_root(&self) -> String {
        format!(
            "{:08x}{:08x}{:08x}{:08x}",
            self.accumulator_root[0],
            self.accumulator_root[1],
            self.accumulator_root[2],
            self.accumulator_root[3]
        )
    }

    #[wasm_bindgen(getter)]
    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    #[wasm_bindgen(getter)]
    pub fn log_num_rows(&self) -> u32 {
        self.log_num_rows
    }
}

fn json_result<T: Serialize>(result: Result<T, String>) -> String {
    match result {
        Ok(data) => serde_json::json!({ "ok": true, "data": data }).to_string(),
        Err(error) => serde_json::json!({ "ok": false, "error": error }).to_string(),
    }
}

fn json_error(msg: &str) -> String {
    serde_json::json!({ "ok": false, "error": msg }).to_string()
}

/// Binding preimage for tx_binding_hash recomputation.
/// Amounts are u64 (parsed from JSON numbers by serde).
#[derive(Deserialize)]
struct BindingPreimage {
    replay_domain: u32,
    payment_asset: u32,
    fee_asset: u32,
    fee_class: u32,
    fee_amount: u64,
    fee_schedule_version: u32,
    recipient_amount: u64,
    recipient_owner: [u32; 4],
    recipient_randomness: u32,
    sender_change_amount: u64,
    sender_change_randomness: u32,
}

/// Recompute tx_binding_hash from a JSON-encoded binding preimage.
/// Returns `{"hash": <u32>}` on success or `{"error": "..."}` on failure.
/// Uses a JSON interface instead of individual f64 parameters to avoid
/// deepening the fragile JS-to-WASM numeric boundary.
#[wasm_bindgen]
pub fn recompute_tx_binding_hash_json(binding_json: &str) -> String {
    let b: BindingPreimage = match serde_json::from_str(binding_json) {
        Ok(v) => v,
        Err(e) => {
            return serde_json::json!({ "error": format!("invalid binding JSON: {e}") }).to_string()
        }
    };
    let hash = compute_mode_a_tx_binding_hash(
        b.replay_domain,
        b.payment_asset,
        b.fee_asset,
        b.fee_class,
        b.fee_amount,
        b.fee_schedule_version,
        b.recipient_amount,
        b.recipient_owner,
        b.recipient_randomness,
        b.sender_change_amount,
        b.sender_change_randomness,
    );
    serde_json::json!({ "hash": hash }).to_string()
}

#[wasm_bindgen]
pub fn dual_fee_quote_payment_with_schedule_json(
    payment_asset: u32,
    fee_asset: u32,
    amount: f64,
    fee_schedule_version: u32,
) -> String {
    let amount = match validate_f64_amount(amount, "amount") {
        Ok(v) => v,
        Err(e) => return json_error(&e),
    };
    json_result(quote_payment(&WalletQuoteRequest {
        payment_asset,
        fee_asset,
        amount,
        fee_schedule_version,
    }))
}

#[wasm_bindgen]
pub fn dual_fee_submit_demo_payment_json(
    payment_asset: u32,
    fee_asset: u32,
    amount: f64,
    fee_schedule_version: u32,
    recipient_owner: u32,
    payment_balance: f64,
    hush_balance: f64,
    credential_expiry: u32,
) -> String {
    let amount = match validate_f64_amount(amount, "amount") {
        Ok(v) => v,
        Err(e) => return json_error(&e),
    };
    let payment_balance = match validate_f64_amount(payment_balance, "payment_balance") {
        Ok(v) => v,
        Err(e) => return json_error(&e),
    };
    let hush_balance = match validate_f64_amount(hush_balance, "hush_balance") {
        Ok(v) => v,
        Err(e) => return json_error(&e),
    };
    json_result(submit_wallet_payment(&WalletSubmissionRequest {
        payment_asset,
        fee_asset,
        amount,
        fee_schedule_version,
        recipient_owner,
        payment_balance,
        hush_balance,
        credential_expiry: (credential_expiry != 0).then_some(credential_expiry),
    }))
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

/// Result type for audit proofs, with serialized proof bytes for independent verification.
#[wasm_bindgen]
pub struct AuditOutput {
    success: bool,
    message: String,
    prove_time_ms: f64,
    verify_time_ms: f64,
    proof_bytes: String,
    // Public data for independent verification
    window_start: u32,
    window_end: u32,
    claimed_total: f64,
    cred_root: [u32; 4],
    cred_null: [u32; 4],
    epoch: u32,
    log_num_rows: u32,
}

#[wasm_bindgen]
impl AuditOutput {
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
    pub fn proof_bytes(&self) -> String {
        self.proof_bytes.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn window_start(&self) -> u32 {
        self.window_start
    }
    #[wasm_bindgen(getter)]
    pub fn window_end(&self) -> u32 {
        self.window_end
    }
    #[wasm_bindgen(getter)]
    pub fn claimed_total(&self) -> f64 {
        self.claimed_total
    }
    #[wasm_bindgen(getter)]
    pub fn cred_null(&self) -> String {
        format!(
            "{:08x}{:08x}{:08x}{:08x}",
            self.cred_null[0], self.cred_null[1], self.cred_null[2], self.cred_null[3]
        )
    }
    #[wasm_bindgen(getter)]
    pub fn cred_root(&self) -> String {
        format!(
            "{:08x}{:08x}{:08x}{:08x}",
            self.cred_root[0], self.cred_root[1], self.cred_root[2], self.cred_root[3]
        )
    }
    #[wasm_bindgen(getter)]
    pub fn epoch(&self) -> u32 {
        self.epoch
    }
    #[wasm_bindgen(getter)]
    pub fn log_num_rows(&self) -> u32 {
        self.log_num_rows
    }
}

#[wasm_bindgen]
pub struct CredentialIssuanceOutput {
    success: bool,
    message: String,
    prove_time_ms: f64,
}

#[wasm_bindgen]
impl CredentialIssuanceOutput {
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
}

#[wasm_bindgen]
pub fn prove_demo_provenance_attestation(
    sk: u32,
    issuer_key: u32,
    expiry: u32,
    secret: u32,
) -> CredentialIssuanceOutput {
    use stwo::core::fields::m31::M31;

    use crate::{poseidon2, types::MERKLE_DEPTH};

    let prove_start = js_sys::Date::now();

    let subject = poseidon2::hashout_to_u32_array(poseidon2::derive_owner(M31::from(sk)));
    let issuer_id = poseidon2::derive_issuer_id(M31::from(issuer_key));
    let credential_commitment = poseidon2::hashout_to_u32_array(poseidon2::credential_commitment(
        issuer_id,
        poseidon2::derive_owner(M31::from(sk)),
        M31::from(expiry),
        M31::from(secret),
    ));

    let mut issuer_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
    issuer_tree.set_leaf(0, issuer_id);
    let issuer_root = poseidon2::hashout_to_u32_array(issuer_tree.root());
    let issuer_path_pairs = issuer_tree.path(0);
    let mut issuer_path = [([0u32; 4], 0u32); MERKLE_DEPTH];
    for (i, (sib, dir)) in issuer_path_pairs.iter().enumerate() {
        issuer_path[i] = (poseidon2::hashout_to_u32_array(*sib), *dir);
    }

    let witness = provenance_attestation::AttestationWitness {
        issuer_root,
        credential_commitment,
        issuer_key,
        subject,
        expiry,
        secret,
        issuer_path,
    };

    match provenance_attestation::prove_provenance_attestation(&witness) {
        Ok(()) => CredentialIssuanceOutput {
            success: true,
            message: "Credential issuance proof verified".to_string(),
            prove_time_ms: js_sys::Date::now() - prove_start,
        },
        Err(error) => CredentialIssuanceOutput {
            success: false,
            message: error,
            prove_time_ms: js_sys::Date::now() - prove_start,
        },
    }
}

fn audit_error(message: String) -> AuditOutput {
    AuditOutput {
        success: false,
        message,
        prove_time_ms: 0.0,
        verify_time_ms: 0.0,
        proof_bytes: String::new(),
        window_start: 0,
        window_end: 0,
        claimed_total: 0.0,
        cred_root: [0; 4],
        cred_null: [0; 4],
        epoch: 0,
        log_num_rows: 0,
    }
}

/// Proves a time-window audit for the browser demo.
/// Amounts are passed as f64 (protocol units, same transport as payment circuit).
#[wasm_bindgen]
pub fn prove_time_window_audit(
    window_start: u32,
    window_end: u32,
    amounts: &[f64],
    timestamps: &[u32],
    sk: u32,
    cred_issuer: u32,
    cred_expiry: u32,
    cred_secret: u32,
) -> AuditOutput {
    use stwo::core::fields::m31::M31;

    use crate::{poseidon2, time_window, types::MERKLE_DEPTH};

    const MAX_TX: usize = 16;

    // Build credential tree
    let owner = poseidon2::derive_owner(M31::from(sk));
    let issuer_id = poseidon2::derive_issuer_id(M31::from(cred_issuer));
    let cred_cm = poseidon2::credential_commitment(
        issuer_id,
        owner,
        M31::from(cred_expiry),
        M31::from(cred_secret),
    );
    let mut cred_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
    cred_tree.set_leaf(0, cred_cm);
    let cred_root = poseidon2::hashout_to_u32_array(cred_tree.root());

    let cred_path_pairs = cred_tree.path(0);
    let mut cred_path = [([0u32; 4], 0u32); MERKLE_DEPTH];
    for (i, (sib, dir)) in cred_path_pairs.iter().enumerate() {
        cred_path[i] = (poseidon2::hashout_to_u32_array(*sib), *dir);
    }

    // Validate and convert f64 amounts to u64 (same validation as payment circuit)
    let tx_count = amounts.len().min(MAX_TX);
    let mut tx_amounts = [0u64; MAX_TX];
    let mut tx_timestamps = [0u32; MAX_TX];
    for i in 0..tx_count {
        match validate_f64_amount(amounts[i], &format!("amount[{i}]")) {
            Ok(v) => tx_amounts[i] = v,
            Err(e) => return audit_error(e),
        }
        tx_timestamps[i] = if i < timestamps.len() { timestamps[i] } else { window_start };
    }

    let claimed_total: u64 =
        match tx_amounts[..tx_count].iter().try_fold(0u64, |acc, &x| acc.checked_add(x)) {
            Some(total) => total,
            None => return audit_error("Amount total overflows u64".to_string()),
        };

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
    let proof_result = match time_window::prove_time_window(&witness) {
        Ok(r) => r,
        Err(e) => return audit_error(e),
    };
    let prove_time = js_sys::Date::now() - prove_start;

    // Serialize proof for independent verification
    let serialized = serde_json::to_string(&proof_result.proof).unwrap_or_else(|_| String::new());
    let proof_bytes = use_base64_encode(&serialized);

    let pd = &proof_result.public_data;
    let log_num_rows = proof_result.log_num_rows;

    // Verify
    let verify_start = js_sys::Date::now();
    match time_window::verify_time_window(&proof_result) {
        Ok(()) => AuditOutput {
            success: true,
            message: "Time-window audit proof verified".to_string(),
            prove_time_ms: prove_time,
            verify_time_ms: js_sys::Date::now() - verify_start,
            proof_bytes,
            window_start: pd.window_start,
            window_end: pd.window_end,
            claimed_total: pd.claimed_total as f64,
            cred_root: pd.cred_root,
            cred_null: pd.cred_null,
            epoch: pd.epoch,
            log_num_rows,
        },
        Err(e) => AuditOutput {
            success: false,
            message: format!("Proof generated but verification failed: {e}"),
            prove_time_ms: prove_time,
            verify_time_ms: js_sys::Date::now() - verify_start,
            proof_bytes: String::new(),
            window_start: 0,
            window_end: 0,
            claimed_total: 0.0,
            cred_root: [0; 4],
            cred_null: [0; 4],
            epoch: 0,
            log_num_rows: 0,
        },
    }
}

/// Independently verify a serialized time-window audit proof.
/// Returns "ok" on success, error message on failure.
#[wasm_bindgen]
pub fn verify_audit_proof(
    proof_b64: &str,
    window_start: u32,
    window_end: u32,
    claimed_total: f64,
    cred_root: &[u32],
    cred_null: &[u32],
    epoch: u32,
    log_num_rows: u32,
) -> String {
    use num_traits::Zero;
    use stwo::core::{
        air::Component, channel::Channel, fields::qm31::QM31, pcs::CommitmentSchemeVerifier,
        verifier::verify,
    };
    use stwo_constraint_framework::{FrameworkComponent, TraceLocationAllocator};

    use crate::{
        prover_common::{pcs_config, ProverChannel, ProverMerkleChannel, ProverMerkleHasher},
        time_window::{TimeWindowEval, TimeWindowPublicData},
    };

    let claimed_total_u64 = match validate_f64_amount(claimed_total, "claimed_total") {
        Ok(v) => v,
        Err(e) => return e,
    };

    fn to_arr(s: &[u32], name: &str) -> Result<[u32; 4], String> {
        if s.len() != 4 {
            return Err(format!("{name} must have 4 elements"));
        }
        Ok([s[0], s[1], s[2], s[3]])
    }
    let cred_root = match to_arr(cred_root, "cred_root") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let cred_null = match to_arr(cred_null, "cred_null") {
        Ok(v) => v,
        Err(e) => return e,
    };

    // Decode proof
    let json_bytes = match base64_decode(proof_b64) {
        Ok(b) => b,
        Err(e) => return format!("Base64 decode failed: {e}"),
    };
    let json_str = match std::str::from_utf8(&json_bytes) {
        Ok(s) => s,
        Err(e) => return format!("UTF-8 decode failed: {e}"),
    };
    let proof: stwo::core::proof::StarkProof<ProverMerkleHasher> =
        match serde_json::from_str(json_str) {
            Ok(p) => p,
            Err(e) => return format!("Proof deserialization failed: {e}"),
        };

    let public_data = TimeWindowPublicData {
        window_start,
        window_end,
        claimed_total: claimed_total_u64,
        cred_root,
        cred_null,
        epoch,
    };

    let config = pcs_config();
    let channel = &mut ProverChannel::default();
    let commitment_scheme = &mut CommitmentSchemeVerifier::<ProverMerkleChannel>::new(config);

    let component = FrameworkComponent::new(
        &mut TraceLocationAllocator::default(),
        TimeWindowEval { log_size: log_num_rows },
        QM31::zero(),
    );
    let sizes = component.trace_log_degree_bounds();

    commitment_scheme.commit(proof.commitments[0], &sizes[0], channel);
    channel.mix_u64(log_num_rows as u64);
    public_data.mix_into(channel);
    commitment_scheme.commit(proof.commitments[1], &sizes[1], channel);

    match verify(&[&component], channel, commitment_scheme, proof) {
        Ok(()) => "ok".to_string(),
        Err(e) => format!("Audit verification failed: {e:?}"),
    }
}

/// Verify a serialized STARK proof against its public outputs.
/// proof_b64: base64-encoded JSON of the serialized StarkProof.
/// log_num_rows: the trace height exponent used when the proof was generated.
///   This is required because different circuit shapes (single payment, batch)
///   use different trace sizes. The prover returns this value in ProofOutput.
/// Returns a JS string: "ok" on success, error message on failure.
#[wasm_bindgen]
pub fn verify_serialized_proof(
    proof_b64: &str,
    note_root: &[u32],
    accumulator_root: &[u32],
    epoch: u32,
    null_0: &[u32],
    null_1: &[u32],
    out_cm_0: &[u32],
    out_cm_1: &[u32],
    tx_binding_hash: &[u32],
    sender_binding_tag: &[u32],
    log_num_rows: u32,
) -> String {
    use num_traits::Zero;
    use stwo::core::fields::qm31::QM31;
    use stwo_constraint_framework::{FrameworkComponent, TraceLocationAllocator};

    use crate::{
        circuit::{HushPaymentEval, PaymentPublicData, ProofResult},
        prover_common::ProverMerkleHasher,
    };

    fn to_arr(s: &[u32], name: &str) -> Result<[u32; 4], String> {
        if s.len() != 4 {
            return Err(format!("{name} must have 4 elements"));
        }
        Ok([s[0], s[1], s[2], s[3]])
    }
    let note_root = match to_arr(note_root, "note_root") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let accumulator_root = match to_arr(accumulator_root, "accumulator_root") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let null_0 = match to_arr(null_0, "null_0") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let null_1 = match to_arr(null_1, "null_1") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let out_cm_0 = match to_arr(out_cm_0, "out_cm_0") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let out_cm_1 = match to_arr(out_cm_1, "out_cm_1") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let tx_binding_hash = match to_arr(tx_binding_hash, "tx_binding_hash") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let sender_binding_tag = match to_arr(sender_binding_tag, "sender_binding_tag") {
        Ok(v) => v,
        Err(e) => return e,
    };

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
        accumulator_root,
        tx_binding_hash,
        sender_binding_tag,
        null_0,
        null_1,
        out_cm_0,
        out_cm_1,
    };

    // log_num_rows is provided by the caller (stored in the receipt from the prover).
    // The minimum valid value is LOG_N_LANES (the SIMD lane width).
    use stwo::prover::backend::simd::m31::LOG_N_LANES;
    if log_num_rows < LOG_N_LANES {
        return format!("log_num_rows ({log_num_rows}) is below minimum ({LOG_N_LANES})");
    }
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
        while i < 26 {
            t[(b'A' + i) as usize] = i;
            t[(b'a' + i) as usize] = 26 + i;
            i += 1;
        }
        let mut i = 0u8;
        while i < 10 {
            t[(b'0' + i) as usize] = 52 + i;
            i += 1;
        }
        t[b'+' as usize] = 62;
        t[b'/' as usize] = 63;
        t[b'=' as usize] = 0;
        t
    };
    let mut i = 0;
    while i + 3 < s.len() {
        let (a, b, c, d) = (
            TABLE[s[i] as usize],
            TABLE[s[i + 1] as usize],
            TABLE[s[i + 2] as usize],
            TABLE[s[i + 3] as usize],
        );
        if a == 255 || b == 255 {
            return Err("invalid base64");
        }
        out.push((a << 2) | (b >> 4));
        if s[i + 2] != b'=' {
            if c == 255 {
                return Err("invalid base64");
            }
            out.push((b << 4) | (c >> 2));
        }
        if s[i + 3] != b'=' {
            if d == 255 {
                return Err("invalid base64");
            }
            out.push((c << 6) | d);
        }
        i += 4;
    }
    Ok(out)
}
