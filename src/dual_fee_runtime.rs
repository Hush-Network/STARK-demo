use std::{fmt::Write, time::Instant};

use serde::{Deserialize, Serialize};

use crate::{
    accounting::{
        AssetFeeBuckets, BlockAccountingBuilder, BlockAccountingRecord, ClaimablePayoutRecord,
        EpochAccumulator, EpochSettlement, ValidatorBlockParticipation, ValidatorStakeInfo,
    },
    fee_sidecar,
    measurement::duration_to_ms,
    payment_fixtures::{build_hush_fee_merkle_context, build_payment_merkle_context},
    payment_tx::{
        expected_fee_amount, payment_route, AssetId, NoteInput, PaymentRoute, PaymentTxV1,
        RecipientIntent, SenderChangeIntent,
    },
    payment_validation::{self, PaymentBundleProof},
};

const DEMO_SENDER_KEY: u32 = 12_345;
const DEMO_CRED_ISSUER: u32 = 1;
const DEMO_CRED_SECRET: u32 = 777;
const DEMO_ACTIVE_CREDENTIAL_EXPIRY: u32 = 50_000;
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
    pub fee_amount: u32,
    pub payment_debit: u32,
    pub hush_fee_debit: u32,
    pub receiver_amount: u32,
    pub backend_backed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletQuoteRequest {
    pub payment_asset: u32,
    pub fee_asset: u32,
    pub amount: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletSubmissionRequest {
    pub payment_asset: u32,
    pub fee_asset: u32,
    pub amount: u32,
    pub recipient_owner: u32,
    pub payment_balance: u32,
    pub hush_balance: u32,
    pub credential_expiry: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaymentProofView {
    pub null_0: u32,
    pub null_1: u32,
    pub out_cm_0: u32,
    pub out_cm_1: u32,
    pub cred_null: u32,
    pub note_root: u32,
    pub cred_root: u32,
    pub epoch: u32,
    pub prove_time_ms: f64,
    pub verify_time_ms: f64,
    pub proof_bytes: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HushSidecarProofView {
    pub note_root: u32,
    pub tx_binding_hash: u32,
    pub sender_binding_tag: u32,
    pub fee_amount: u32,
    pub null_0: u32,
    pub null_1: u32,
    pub change_cm: u32,
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
                label: "Wallet onboarding and credential activation".to_string(),
                level: ImplementationLevel::RepresentedOnly,
                detail: "The demo still simulates setup and credential issuance so the payment path stays focused on dual fee model execution support.".to_string(),
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
                detail: "The demo executes locally and does not yet submit dual fee transactions into a live validator network or finality path.".to_string(),
            },
        ],
    }
}

pub fn quote_payment(request: &WalletQuoteRequest) -> Result<PaymentQuote, String> {
    if request.amount == 0 {
        return Err("payment amount must be non-zero".to_string());
    }

    let route = payment_route(request.payment_asset, request.fee_asset)?;
    let fee_amount = expected_fee_amount(request.payment_asset, request.fee_asset)?;
    let payment_debit = request
        .amount
        .checked_add(route.payment_fee_deduction(fee_amount))
        .ok_or_else(|| "payment debit overflow".to_string())?;
    let hush_fee_debit = if route == PaymentRoute::HushSidecar {
        fee_amount
    } else {
        0
    };

    Ok(PaymentQuote {
        payment_asset: request.payment_asset,
        fee_asset: request.fee_asset,
        route: route_label(route),
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

pub fn submit_wallet_payment(request: &WalletSubmissionRequest) -> Result<WalletSubmissionResult, String> {
    let prepared = prepare_wallet_submission(request)?;

    let prove_start = Instant::now();
    let bundle = payment_validation::prove_payment_bundle(
        &prepared.tx,
        &prepared.payment_witness,
        prepared.fee_sidecar_witness.as_ref(),
    )?;
    let prove_time_ms = duration_to_ms(prove_start.elapsed());

    let verify_start = Instant::now();
    payment_validation::validate_payment_bundle(&prepared.tx, &bundle)?;
    let verify_time_ms = duration_to_ms(verify_start.elapsed());

    let accounting_start = Instant::now();
    let mut block = BlockAccountingBuilder::new(DEMO_BLOCK_HEIGHT, DEMO_PROPOSER_ID);
    block.record_payment_bundle(&prepared.tx, &bundle)?;
    let block_accounting = block.finalize();
    block_accounting.validate()?;
    let accounting_time_ms = duration_to_ms(accounting_start.elapsed());

    let settlement_start = Instant::now();
    let validators = demo_validator_set();
    let participation = demo_participation();
    let mut epoch = EpochAccumulator::new(1);
    epoch.apply_block(&block_accounting, &validators, &participation)?;
    let settlement = epoch.close()?;
    let payout_inspection = inspect_claimable_payouts(&settlement)?;
    let settlement_time_ms = duration_to_ms(settlement_start.elapsed());

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
            TimedStage {
                label: "prove".to_string(),
                duration_ms: prove_time_ms,
            },
            TimedStage {
                label: "verify".to_string(),
                duration_ms: verify_time_ms,
            },
            TimedStage {
                label: "accounting".to_string(),
                duration_ms: accounting_time_ms,
            },
            TimedStage {
                label: "epoch_close".to_string(),
                duration_ms: settlement_time_ms,
            },
        ],
    })
}

fn prepare_wallet_submission(request: &WalletSubmissionRequest) -> Result<PreparedWalletSubmission, String> {
    let quote = quote_payment(&WalletQuoteRequest {
        payment_asset: request.payment_asset,
        fee_asset: request.fee_asset,
        amount: request.amount,
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
    let recipient_randomness = request.recipient_owner.wrapping_mul(31).wrapping_add(request.amount);
    let sender_change_randomness = request.payment_balance.wrapping_mul(17).wrapping_add(444);
    let tx = match route {
        PaymentRoute::SameAsset => PaymentTxV1::build_same_asset(
            payment_asset,
            payment_inputs.clone(),
            RecipientIntent {
                amount: request.amount,
                owner: request.recipient_owner,
                randomness: recipient_randomness,
            },
            sender_change_randomness,
            DEMO_SENDER_KEY,
        )?,
        PaymentRoute::HushSidecar => PaymentTxV1::build_with_hush_fee(
            payment_asset,
            payment_inputs.clone(),
            RecipientIntent {
                amount: request.amount,
                owner: request.recipient_owner,
                randomness: recipient_randomness,
            },
            sender_change_randomness,
            DEMO_SENDER_KEY,
        )?,
    };

    let cred_expiry = request
        .credential_expiry
        .unwrap_or(DEMO_ACTIVE_CREDENTIAL_EXPIRY);
    let payment_context = build_payment_merkle_context(
        DEMO_SENDER_KEY,
        payment_inputs,
        payment_asset,
        DEMO_CRED_ISSUER,
        cred_expiry,
        DEMO_CRED_SECRET,
        DEMO_EPOCH,
    );
    let payment_witness = tx.build_witness(DEMO_SENDER_KEY, &payment_context)?;

    let fee_sidecar_witness = if route == PaymentRoute::HushSidecar {
        let hush_inputs = split_demo_notes(request.hush_balance, 515, 616)?;
        let hush_context = build_hush_fee_merkle_context(DEMO_SENDER_KEY, hush_inputs.clone());
        let hush_change = SenderChangeIntent {
            amount: request
                .hush_balance
                .checked_sub(quote.hush_fee_debit)
                .ok_or_else(|| "invalid HUSH change after fee deduction".to_string())?,
            randomness: request.hush_balance.wrapping_mul(19).wrapping_add(717),
        };
        Some(tx.build_hush_fee_witness(
            DEMO_SENDER_KEY,
            hush_inputs,
            hush_change,
            &hush_context,
        )?)
    } else {
        None
    };

    Ok(PreparedWalletSubmission {
        quote,
        tx,
        payment_witness,
        fee_sidecar_witness,
    })
}

fn split_demo_notes(total: u32, rand_0: u32, rand_1: u32) -> Result<[NoteInput; 2], String> {
    if total < 2 {
        return Err("demo balance must be at least two base units".to_string());
    }
    let mut left = ((total as u64)
        .checked_mul(DEMO_NOTE_RATIO_BPS as u64)
        .ok_or_else(|| "demo note split overflow".to_string())?
        / 10_000) as u32;
    left = left.clamp(1, total - 1);
    let right = total - left;
    Ok([
        NoteInput {
            amount: left,
            randomness: rand_0,
        },
        NoteInput {
            amount: right,
            randomness: rand_1,
        },
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
        cred_null: bundle.payment.public_data.cred_null,
        note_root: bundle.payment.public_data.note_root,
        cred_root: bundle.payment.public_data.cred_root,
        epoch: bundle.payment.public_data.epoch,
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
        ValidatorStakeInfo {
            validator_id: 1,
            payout_key: 101,
            effective_stake: 120,
        },
        ValidatorStakeInfo {
            validator_id: 2,
            payout_key: 202,
            effective_stake: 80,
        },
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
    const TABLE: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
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
    use crate::payment_tx::{AssetId, PaymentTxV1};

    fn sample_mode_a_request() -> WalletSubmissionRequest {
        WalletSubmissionRequest {
            payment_asset: AssetId::Usdc as u32,
            fee_asset: AssetId::Usdc as u32,
            amount: 8_000,
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
            recipient_owner: 66_666,
            payment_balance: 11_000,
            hush_balance: 15,
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
        })
        .expect("same-asset quote should succeed");
        assert_eq!(quote.route, "mode_a_same_asset");
        assert_eq!(quote.payment_debit, 8_005);
        assert_eq!(quote.hush_fee_debit, 0);

        let prepared = prepare_wallet_submission(&request).expect("submission should prepare");
        let expected = PaymentTxV1::build_same_asset(
            AssetId::Usdc,
            split_demo_notes(request.payment_balance, 111, 222).expect("split"),
            RecipientIntent {
                amount: request.amount,
                owner: request.recipient_owner,
                randomness: request.recipient_owner.wrapping_mul(31).wrapping_add(request.amount),
            },
            request.payment_balance.wrapping_mul(17).wrapping_add(444),
            DEMO_SENDER_KEY,
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
        })
        .expect("HUSH sidecar quote should succeed");
        assert_eq!(quote.route, "mode_b_hush_sidecar");
        assert_eq!(quote.payment_debit, 10_000);
        assert_eq!(quote.hush_fee_debit, 5);

        let prepared = prepare_wallet_submission(&request).expect("submission should prepare");
        let expected = PaymentTxV1::build_with_hush_fee(
            AssetId::Usdt,
            split_demo_notes(request.payment_balance, 111, 222).expect("split"),
            RecipientIntent {
                amount: request.amount,
                owner: request.recipient_owner,
                randomness: request.recipient_owner.wrapping_mul(31).wrapping_add(request.amount),
            },
            request.payment_balance.wrapping_mul(17).wrapping_add(444),
            DEMO_SENDER_KEY,
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
        })
        .expect_err("cross-stable route should be rejected");
        assert!(err.contains("cross-stablecoin"));
    }

    #[test]
    fn test_malformed_submission_rejected_for_insufficient_hush_balance() {
        let mut request = sample_mode_b_request();
        request.hush_balance = 4;
        let err = submit_wallet_payment(&request)
            .expect_err("insufficient HUSH fee reserve should be rejected");
        assert!(err.contains("HUSH balance"));
    }

    #[test]
    fn test_claimable_payout_records_are_inspectable() {
        let settlement = submit_wallet_payment(&sample_mode_b_request())
            .expect("Mode B submission should succeed")
            .settlement;
        let record = settlement
            .payout_record_for_validator(1)
            .expect("validator 1 payout should exist");
        assert!(record.entitlement.hush > 0);
    }

    #[test]
    fn test_mixed_basket_payout_totals_reconcile_at_consumer_interface() {
        let result = submit_wallet_payment(&sample_mode_b_request())
            .expect("Mode B submission should succeed");
        assert_eq!(
            result.payout_inspection.fee_pools,
            result.payout_inspection.payout_totals
        );
        assert_eq!(
            result.payout_inspection.payout_totals.hush,
            u64::from(result.quote.hush_fee_debit)
        );
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
}
