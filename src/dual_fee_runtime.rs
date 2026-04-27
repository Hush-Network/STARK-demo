use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::{
    accounting::{
        AssetFeeBuckets, BlockAccountingBuilder, BlockAccountingRecord, ClaimablePayoutRecord,
        EpochAccumulator, EpochSettlement, ValidatorBlockParticipation, ValidatorStakeInfo,
    },
    fee_sidecar,
    measurement::Timer,
    payment_fixtures::{build_hush_fee_merkle_context, build_payment_merkle_context},
    payment_tx::{
        expected_fee_amount, payment_route, AssetId, NoteInput, PaymentRoute, PaymentTxV1,
        RecipientIntent, SenderChangeIntent,
    },
    payment_validation::{self, PaymentBundleProof},
};

const DEMO_SENDER_KEY: u32 = 12_345;
const DEMO_EPOCH: u32 = 1_000;
const DEMO_BLOCK_HEIGHT: u64 = 4_200;
const DEMO_PROPOSER_ID: u32 = 1;
const DEMO_NOTE_RATIO_BPS: u32 = 6_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationLevel {
    Supported,
    RepresentedOnly,
    NotImplemented,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewItem {
    pub group: String,
    pub key: String,
    pub label: String,
    pub level: ImplementationLevel,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DualFeeReviewSnapshot {
    pub items: Vec<ReviewItem>,
}

impl DualFeeReviewSnapshot {
    pub fn item(&self, key: &str) -> Option<&ReviewItem> {
        self.items.iter().find(|item| item.key == key)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentQuote {
    pub payment_asset: u32,
    pub fee_asset: u32,
    pub route: String,
    pub fee_schedule_version: u32,
    pub fee_amount: u64,
    pub payment_debit: u64,
    pub hush_fee_debit: u64,
    pub receiver_amount: u64,
    pub backend_backed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletQuoteRequest {
    pub payment_asset: u32,
    pub fee_asset: u32,
    pub amount: u64,
    pub fee_schedule_version: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletSubmissionRequest {
    pub payment_asset: u32,
    pub fee_asset: u32,
    pub amount: u64,
    pub fee_schedule_version: u32,
    pub recipient_owner: u32,
    pub payment_balance: u64,
    pub hush_balance: u64,
    pub credential_expiry: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaymentProofView {
    pub null_0: [u32; 4],
    pub null_1: [u32; 4],
    pub out_cm_0: [u32; 4],
    pub out_cm_1: [u32; 4],
    pub accumulator_root: [u32; 4],
    pub note_root: [u32; 4],
    pub epoch: u32,
    pub tx_binding_hash: [u32; 4],
    pub sender_binding_tag: [u32; 4],
    pub prove_time_ms: f64,
    pub verify_time_ms: f64,
    pub proof_bytes: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HushSidecarProofView {
    pub note_root: [u32; 4],
    pub tx_binding_hash: [u32; 4],
    pub sender_binding_tag: [u32; 4],
    pub fee_amount: u64,
    pub null_0: [u32; 4],
    pub null_1: [u32; 4],
    pub change_cm: [u32; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimedStage {
    pub label: String,
    pub duration_ms: f64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayoutInspection {
    pub fee_pools: AssetFeeBuckets,
    pub payout_totals: AssetFeeBuckets,
    pub payout_records: Vec<ClaimablePayoutRecord>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WalletSubmissionResult {
    pub accepted: bool,
    pub route: String,
    pub quote: PaymentQuote,
    pub tx: PaymentTxV1,
    pub payment_proof: PaymentProofView,
    pub hush_sidecar: Option<HushSidecarProofView>,
    pub block_accounting: BlockAccountingRecord,
    pub settlement: EpochSettlement,
    pub payout_inspection: PayoutInspection,
    pub stage_timings: Vec<TimedStage>,
}

struct PreparedWalletSubmission {
    quote: PaymentQuote,
    tx: PaymentTxV1,
    payment_witness: crate::types::PaymentWitness,
    fee_sidecar_witness: Option<crate::types::HushFeeWitness>,
}

pub fn dual_fee_review_snapshot() -> DualFeeReviewSnapshot {
    DualFeeReviewSnapshot {
        items: vec![
            ReviewItem {
                group: "supported".to_string(),
                key: "payment_route_same_asset".to_string(),
                label: "Same-asset payment fee route".to_string(),
                level: ImplementationLevel::Supported,
                detail: "USDC->USDC and USDT->USDT use canonical PaymentTxV1 building, proof generation, validation, and accounting.".to_string(),
            },
            ReviewItem {
                group: "supported".to_string(),
                key: "payment_route_hush_sidecar".to_string(),
                label: "HUSH sidecar fee route".to_string(),
                level: ImplementationLevel::Supported,
                detail: "USDC->HUSH and USDT->HUSH use the narrow sidecar proof path with tx_binding_hash and sender_binding_tag enforcement.".to_string(),
            },
            ReviewItem {
                group: "supported".to_string(),
                key: "fee_quote_and_builder".to_string(),
                label: "Wallet quote and builder path".to_string(),
                level: ImplementationLevel::Supported,
                detail: "The aligned runtime quotes only the four valid combinations and always builds the canonical unsigned PaymentTxV1.".to_string(),
            },
            ReviewItem {
                group: "supported".to_string(),
                key: "submission_validation_accounting".to_string(),
                label: "Submission, validation, and accounting".to_string(),
                level: ImplementationLevel::Supported,
                detail: "Accepted demo submissions run through bundle validation, block fee buckets, epoch accrual, and claimable payout record generation.".to_string(),
            },
            ReviewItem {
                group: "supported".to_string(),
                key: "claimable_payout_records".to_string(),
                label: "Claimable mixed-basket payout records".to_string(),
                level: ImplementationLevel::Supported,
                detail: "Validator entitlements and claimable payout records are now inspectable through the runtime and test interfaces.".to_string(),
            },
            ReviewItem {
                group: "represented_only".to_string(),
                key: "wallet_onboarding".to_string(),
                label: "Wallet setup and credential issuance".to_string(),
                level: ImplementationLevel::RepresentedOnly,
                detail: "Wallet setup and issuer approval remain represented in the demo, even though the browser can run the credential issuance circuit locally.".to_string(),
            },
            ReviewItem {
                group: "represented_only".to_string(),
                key: "wallet_balances_and_funding".to_string(),
                label: "Wallet balances and funding".to_string(),
                level: ImplementationLevel::RepresentedOnly,
                detail: "Displayed balances and the demo HUSH sidecar reserve remain local browser state rather than protocol-connected wallet sync.".to_string(),
            },
            ReviewItem {
                group: "represented_only".to_string(),
                key: "wallet_reward_consumption_ui".to_string(),
                label: "Validator reward wallet UX".to_string(),
                level: ImplementationLevel::RepresentedOnly,
                detail: "Claimable payout records exist, but a production wallet reward claiming flow has not been built yet.".to_string(),
            },
            ReviewItem {
                group: "not_implemented".to_string(),
                key: "live_network_submission".to_string(),
                label: "Live validator network path".to_string(),
                level: ImplementationLevel::NotImplemented,
                detail: "The demo executes locally and does not yet submit dual-fee transactions into a live validator network or finality path.".to_string(),
            },
        ],
    }
}

pub fn quote_payment(request: &WalletQuoteRequest) -> Result<PaymentQuote, String> {
    if request.amount == 0 {
        return Err("payment amount must be non-zero".to_string());
    }

    let route = payment_route(request.payment_asset, request.fee_asset)?;
    let fee_amount = expected_fee_amount(
        request.payment_asset,
        request.fee_asset,
        request.fee_schedule_version,
    )?;
    let payment_debit = request
        .amount
        .checked_add(route.payment_fee_deduction(fee_amount))
        .ok_or_else(|| "payment debit overflow".to_string())?;
    let hush_fee_debit = if route == PaymentRoute::HushSidecar { fee_amount } else { 0 };

    Ok(PaymentQuote {
        payment_asset: request.payment_asset,
        fee_asset: request.fee_asset,
        route: route_label(route),
        fee_schedule_version: request.fee_schedule_version,
        fee_amount,
        payment_debit,
        hush_fee_debit,
        receiver_amount: request.amount,
        backend_backed: true,
    })
}

pub fn inspect_claimable_payouts(settlement: &EpochSettlement) -> Result<PayoutInspection, String> {
    Ok(PayoutInspection {
        fee_pools: settlement.fee_pools,
        payout_totals: settlement.total_payouts()?,
        payout_records: settlement.payout_records.clone(),
    })
}

pub fn submit_wallet_payment(
    request: &WalletSubmissionRequest,
) -> Result<WalletSubmissionResult, String> {
    let prepared = prepare_wallet_submission(request)?;

    let prove_timer = Timer::start();
    let bundle = payment_validation::prove_payment_bundle(
        &prepared.tx,
        &prepared.payment_witness,
        prepared.fee_sidecar_witness.as_ref(),
    )?;
    let prove_time_ms = prove_timer.elapsed_ms();

    let verify_timer = Timer::start();
    payment_validation::validate_payment_bundle(&prepared.tx, &bundle)?;
    let verify_time_ms = verify_timer.elapsed_ms();

    let accounting_timer = Timer::start();
    let mut block = BlockAccountingBuilder::new(DEMO_BLOCK_HEIGHT, DEMO_PROPOSER_ID);
    block.record_payment_bundle(&prepared.tx, &bundle)?;
    let block_accounting = block.finalize();
    block_accounting.validate()?;
    let accounting_time_ms = accounting_timer.elapsed_ms();

    let settlement_timer = Timer::start();
    let validators = demo_validator_set();
    let participation = demo_participation();
    let mut epoch = EpochAccumulator::new(1);
    epoch.apply_block(&block_accounting, &validators, &participation)?;
    let settlement = epoch.close()?;
    let payout_inspection = inspect_claimable_payouts(&settlement)?;
    let settlement_time_ms = settlement_timer.elapsed_ms();

    Ok(WalletSubmissionResult {
        accepted: true,
        route: prepared.quote.route.clone(),
        quote: prepared.quote,
        tx: prepared.tx,
        payment_proof: payment_proof_view(&bundle, prove_time_ms, verify_time_ms)?,
        hush_sidecar: bundle.fee_sidecar.as_ref().map(hush_sidecar_view),
        block_accounting,
        settlement,
        payout_inspection,
        stage_timings: vec![
            TimedStage { label: "prove".to_string(), duration_ms: prove_time_ms },
            TimedStage { label: "verify".to_string(), duration_ms: verify_time_ms },
            TimedStage { label: "accounting".to_string(), duration_ms: accounting_time_ms },
            TimedStage { label: "epoch_close".to_string(), duration_ms: settlement_time_ms },
        ],
    })
}

fn prepare_wallet_submission(
    request: &WalletSubmissionRequest,
) -> Result<PreparedWalletSubmission, String> {
    let quote = quote_payment(&WalletQuoteRequest {
        payment_asset: request.payment_asset,
        fee_asset: request.fee_asset,
        amount: request.amount,
        fee_schedule_version: request.fee_schedule_version,
    })?;
    let payment_asset = AssetId::try_from_u32(request.payment_asset)?;
    if request.payment_balance < quote.payment_debit {
        return Err(format!(
            "payment balance {} does not cover required debit {}",
            request.payment_balance, quote.payment_debit
        ));
    }
    if quote.hush_fee_debit > 0 && request.hush_balance < quote.hush_fee_debit {
        return Err(format!(
            "HUSH balance {} does not cover required fee {}",
            request.hush_balance, quote.hush_fee_debit
        ));
    }

    let route = payment_route(request.payment_asset, request.fee_asset)?;
    let payment_inputs = split_demo_notes(request.payment_balance, 111, 222)?;
    let recipient_randomness =
        request.recipient_owner.wrapping_mul(31).wrapping_add(request.amount as u32);
    let sender_change_randomness =
        (request.payment_balance as u32).wrapping_mul(17).wrapping_add(444);
    let recipient_owner_hash = crate::poseidon2::hashout_to_u32_array(
        crate::poseidon2::derive_owner(stwo::core::fields::m31::M31::from(request.recipient_owner)),
    );
    let tx = match route {
        PaymentRoute::SameAsset => PaymentTxV1::build_same_asset_with_schedule(
            payment_asset,
            payment_inputs.clone(),
            RecipientIntent {
                amount: request.amount,
                owner: recipient_owner_hash,
                randomness: recipient_randomness,
            },
            sender_change_randomness,
            DEMO_SENDER_KEY,
            request.fee_schedule_version,
        )?,
        PaymentRoute::HushSidecar => PaymentTxV1::build_with_hush_fee_with_schedule(
            payment_asset,
            payment_inputs.clone(),
            RecipientIntent {
                amount: request.amount,
                owner: recipient_owner_hash,
                randomness: recipient_randomness,
            },
            sender_change_randomness,
            DEMO_SENDER_KEY,
            request.fee_schedule_version,
        )?,
    };

    let payment_context =
        build_payment_merkle_context(DEMO_SENDER_KEY, payment_inputs, payment_asset, DEMO_EPOCH);
    let payment_witness = tx.build_witness(DEMO_SENDER_KEY, &payment_context)?;

    let fee_sidecar_witness = if route == PaymentRoute::HushSidecar {
        let hush_inputs = split_demo_notes(request.hush_balance, 515, 616)?;
        let hush_context = build_hush_fee_merkle_context(DEMO_SENDER_KEY, hush_inputs.clone());
        let hush_change = SenderChangeIntent {
            amount: request
                .hush_balance
                .checked_sub(quote.hush_fee_debit)
                .ok_or_else(|| "invalid HUSH change after fee deduction".to_string())?,
            randomness: (request.hush_balance as u32).wrapping_mul(19).wrapping_add(717),
        };
        Some(tx.build_hush_fee_witness(DEMO_SENDER_KEY, hush_inputs, hush_change, &hush_context)?)
    } else {
        None
    };

    Ok(PreparedWalletSubmission { quote, tx, payment_witness, fee_sidecar_witness })
}

fn split_demo_notes(total: u64, rand_0: u32, rand_1: u32) -> Result<[NoteInput; 2], String> {
    if total < 2 {
        return Err("demo balance must be at least two base units".to_string());
    }
    let mut left = total
        .checked_mul(u64::from(DEMO_NOTE_RATIO_BPS))
        .ok_or_else(|| "demo note split overflow".to_string())?
        / 10_000;
    left = left.clamp(1, total - 1);
    let right = total - left;
    Ok([
        NoteInput { amount: left, randomness: rand_0 },
        NoteInput { amount: right, randomness: rand_1 },
    ])
}

fn route_label(route: PaymentRoute) -> String {
    match route {
        PaymentRoute::SameAsset => "mode_a_same_asset".to_string(),
        PaymentRoute::HushSidecar => "mode_b_hush_sidecar".to_string(),
    }
}

fn payment_proof_view(
    bundle: &PaymentBundleProof,
    prove_time_ms: f64,
    verify_time_ms: f64,
) -> Result<PaymentProofView, String> {
    let serialized = serde_json::to_string(&bundle.payment.proof)
        .map_err(|err| format!("failed to serialize payment proof: {err}"))?;
    Ok(PaymentProofView {
        null_0: bundle.payment.public_data.null_0,
        null_1: bundle.payment.public_data.null_1,
        out_cm_0: bundle.payment.public_data.out_cm_0,
        out_cm_1: bundle.payment.public_data.out_cm_1,
        accumulator_root: bundle.payment.public_data.accumulator_root,
        note_root: bundle.payment.public_data.note_root,
        epoch: bundle.payment.public_data.epoch,
        tx_binding_hash: bundle.payment.public_data.tx_binding_hash,
        sender_binding_tag: bundle.payment.public_data.sender_binding_tag,
        prove_time_ms,
        verify_time_ms,
        proof_bytes: base64_encode(&serialized),
    })
}

fn hush_sidecar_view(result: &fee_sidecar::ProofResult) -> HushSidecarProofView {
    HushSidecarProofView {
        note_root: result.public_data.note_root,
        tx_binding_hash: result.public_data.tx_binding_hash,
        sender_binding_tag: result.public_data.sender_binding_tag,
        fee_amount: result.public_data.fee_amount,
        null_0: result.public_data.null_0,
        null_1: result.public_data.null_1,
        change_cm: result.public_data.change_cm,
    }
}

fn demo_validator_set() -> Vec<ValidatorStakeInfo> {
    vec![
        ValidatorStakeInfo { validator_id: 1, payout_key: 101, effective_stake: 120 },
        ValidatorStakeInfo { validator_id: 2, payout_key: 202, effective_stake: 80 },
    ]
}

fn demo_participation() -> Vec<ValidatorBlockParticipation> {
    vec![
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
    ]
}

fn base64_encode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut index = 0;
    while index + 2 < bytes.len() {
        let b0 = bytes[index] as usize;
        let b1 = bytes[index + 1] as usize;
        let b2 = bytes[index + 2] as usize;
        let _ = out.write_char(TABLE[b0 >> 2] as char);
        let _ = out.write_char(TABLE[((b0 & 3) << 4) | (b1 >> 4)] as char);
        let _ = out.write_char(TABLE[((b1 & 0xf) << 2) | (b2 >> 6)] as char);
        let _ = out.write_char(TABLE[b2 & 0x3f] as char);
        index += 3;
    }

    match bytes.len() - index {
        1 => {
            let b0 = bytes[index] as usize;
            let _ = out.write_char(TABLE[b0 >> 2] as char);
            let _ = out.write_char(TABLE[(b0 & 3) << 4] as char);
            out.push_str("==");
        }
        2 => {
            let b0 = bytes[index] as usize;
            let b1 = bytes[index + 1] as usize;
            let _ = out.write_char(TABLE[b0 >> 2] as char);
            let _ = out.write_char(TABLE[((b0 & 3) << 4) | (b1 >> 4)] as char);
            let _ = out.write_char(TABLE[(b1 & 0xf) << 2] as char);
            out.push('=');
        }
        _ => {}
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payment_tx::{
        AssetId, PaymentTxV1, PAYMENT_FEE_SCHEDULE_BUSY, PAYMENT_FEE_SCHEDULE_STANDARD,
    };

    fn sample_mode_a_request() -> WalletSubmissionRequest {
        WalletSubmissionRequest {
            payment_asset: AssetId::Usdc as u32,
            fee_asset: AssetId::Usdc as u32,
            amount: 8_000,
            fee_schedule_version: PAYMENT_FEE_SCHEDULE_STANDARD,
            recipient_owner: 77_777,
            payment_balance: 10_000,
            hush_balance: 12,
            credential_expiry: None,
        }
    }

    fn sample_mode_b_request() -> WalletSubmissionRequest {
        WalletSubmissionRequest {
            payment_asset: AssetId::Usdt as u32,
            fee_asset: AssetId::Hush as u32,
            amount: 10_000,
            fee_schedule_version: PAYMENT_FEE_SCHEDULE_STANDARD,
            recipient_owner: 66_666,
            payment_balance: 11_000,
            hush_balance: 100,
            credential_expiry: None,
        }
    }

    #[test]
    fn test_supported_route_quote_and_builder_alignment_same_asset() {
        let request = sample_mode_a_request();
        let quote = quote_payment(&WalletQuoteRequest {
            payment_asset: request.payment_asset,
            fee_asset: request.fee_asset,
            amount: request.amount,
            fee_schedule_version: request.fee_schedule_version,
        })
        .expect("same-asset quote should succeed");
        assert_eq!(quote.route, "mode_a_same_asset");
        assert_eq!(quote.payment_debit, 8_050);
        assert_eq!(quote.hush_fee_debit, 0);

        let prepared = prepare_wallet_submission(&request).expect("submission should prepare");
        let expected = PaymentTxV1::build_same_asset_with_schedule(
            AssetId::Usdc,
            split_demo_notes(request.payment_balance, 111, 222).expect("split"),
            RecipientIntent {
                amount: request.amount,
                owner: crate::poseidon2::hashout_to_u32_array(crate::poseidon2::derive_owner(
                    stwo::core::fields::m31::M31::from(request.recipient_owner),
                )),
                randomness: request
                    .recipient_owner
                    .wrapping_mul(31)
                    .wrapping_add(request.amount as u32),
            },
            (request.payment_balance as u32).wrapping_mul(17).wrapping_add(444),
            DEMO_SENDER_KEY,
            request.fee_schedule_version,
        )
        .expect("expected tx should build");
        assert_eq!(prepared.tx, expected);
    }

    #[test]
    fn test_supported_route_quote_and_builder_alignment_hush_sidecar() {
        let request = sample_mode_b_request();
        let quote = quote_payment(&WalletQuoteRequest {
            payment_asset: request.payment_asset,
            fee_asset: request.fee_asset,
            amount: request.amount,
            fee_schedule_version: request.fee_schedule_version,
        })
        .expect("HUSH sidecar quote should succeed");
        assert_eq!(quote.route, "mode_b_hush_sidecar");
        assert_eq!(quote.payment_debit, 10_000);
        assert_eq!(quote.hush_fee_debit, 50);

        let prepared = prepare_wallet_submission(&request).expect("submission should prepare");
        let expected = PaymentTxV1::build_with_hush_fee_with_schedule(
            AssetId::Usdt,
            split_demo_notes(request.payment_balance, 111, 222).expect("split"),
            RecipientIntent {
                amount: request.amount,
                owner: crate::poseidon2::hashout_to_u32_array(crate::poseidon2::derive_owner(
                    stwo::core::fields::m31::M31::from(request.recipient_owner),
                )),
                randomness: request
                    .recipient_owner
                    .wrapping_mul(31)
                    .wrapping_add(request.amount as u32),
            },
            (request.payment_balance as u32).wrapping_mul(17).wrapping_add(444),
            DEMO_SENDER_KEY,
            request.fee_schedule_version,
        )
        .expect("expected tx should build");
        assert_eq!(prepared.tx, expected);
    }

    #[test]
    fn test_unsupported_route_quote_rejected() {
        let err = quote_payment(&WalletQuoteRequest {
            payment_asset: AssetId::Usdc as u32,
            fee_asset: AssetId::Usdt as u32,
            amount: 8_000,
            fee_schedule_version: PAYMENT_FEE_SCHEDULE_STANDARD,
        })
        .expect_err("cross-asset route should be rejected");
        assert!(err.contains("cross-asset"));
    }

    #[test]
    fn test_malformed_submission_rejected_for_insufficient_hush_balance() {
        let mut request = sample_mode_b_request();
        request.hush_balance = 0;
        let err = submit_wallet_payment(&request)
            .expect_err("insufficient HUSH fee reserve should be rejected");
        assert!(err.contains("HUSH balance"));
    }

    #[test]
    fn test_claimable_payout_records_are_inspectable() {
        let settlement = submit_wallet_payment(&sample_mode_b_request())
            .expect("Mode B submission should succeed")
            .settlement;
        let record =
            settlement.payout_record_for_validator(1).expect("validator 1 payout should exist");
        assert!(record.entitlement.hush > 0);
    }

    #[test]
    fn test_mixed_basket_payout_totals_reconcile_at_consumer_interface() {
        let result = submit_wallet_payment(&sample_mode_b_request())
            .expect("Mode B submission should succeed");
        assert_eq!(result.payout_inspection.fee_pools, result.payout_inspection.payout_totals);
        assert_eq!(result.payout_inspection.payout_totals.hush, result.quote.hush_fee_debit);
    }

    #[test]
    fn test_supported_paths_marked_real_only_when_backend_backed() {
        let review = dual_fee_review_snapshot();
        assert_eq!(
            review.item("payment_route_same_asset").map(|item| item.level),
            Some(ImplementationLevel::Supported)
        );
        assert_eq!(
            review.item("payment_route_hush_sidecar").map(|item| item.level),
            Some(ImplementationLevel::Supported)
        );
        assert_eq!(
            review.item("wallet_onboarding").map(|item| item.level),
            Some(ImplementationLevel::RepresentedOnly)
        );
    }

    #[test]
    fn test_unsupported_steps_remain_explicitly_marked() {
        let review = dual_fee_review_snapshot();
        assert_eq!(
            review.item("live_network_submission").map(|item| item.level),
            Some(ImplementationLevel::NotImplemented)
        );
        assert_eq!(
            review.item("wallet_reward_consumption_ui").map(|item| item.level),
            Some(ImplementationLevel::RepresentedOnly)
        );
    }

    #[test]
    fn test_demo_large_balance_125k_payment() {
        // Reproduces the exact demo values: initial USDC balance 1.5M, payment 125k
        let request = WalletSubmissionRequest {
            payment_asset: 1, // USDC
            fee_asset: 1,     // same-asset
            amount: 125_000,
            fee_schedule_version: PAYMENT_FEE_SCHEDULE_STANDARD,
            recipient_owner: 50_000,
            payment_balance: 1_500_000,
            hush_balance: 250,
            credential_expiry: None,
        };
        let result = submit_wallet_payment(&request)
            .expect("1.5M balance / 125k payment should prove successfully");
        assert_eq!(result.quote.route, "mode_a_same_asset");
    }

    #[test]
    fn test_busy_schedule_quotes_higher_fee_without_amount_dependency() {
        let quote = quote_payment(&WalletQuoteRequest {
            payment_asset: AssetId::Usdc as u32,
            fee_asset: AssetId::Usdc as u32,
            amount: 50_000_000,
            fee_schedule_version: PAYMENT_FEE_SCHEDULE_BUSY,
        })
        .expect("busy quote should succeed");
        assert_eq!(quote.fee_amount, 125);
        assert_eq!(quote.payment_debit, 50_000_125);
    }

    #[test]
    fn test_busy_schedule_submission_proves_successfully() {
        let mut request = sample_mode_a_request();
        request.amount = 5_000_000;
        request.payment_balance = 75_000_000;
        request.fee_schedule_version = PAYMENT_FEE_SCHEDULE_BUSY;

        let result =
            submit_wallet_payment(&request).expect("busy schedule submission should succeed");

        assert_eq!(result.tx.descriptor.fee_schedule_version, PAYMENT_FEE_SCHEDULE_BUSY);
        assert_eq!(result.quote.fee_amount, 125);
        assert!(result.accepted);
    }
}
