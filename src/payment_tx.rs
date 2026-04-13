use serde::{Deserialize, Serialize};
use stwo::core::fields::m31::M31;

use crate::{
    poseidon2,
    types::{HushFeeWitness, PaymentWitness, MERKLE_DEPTH},
};

pub const PAYMENT_TX_V1_VERSION: u32 = 1;
pub const PAYMENT_TX_V1_REPLAY_DOMAIN: u32 = 1;
pub const PAYMENT_FEE_SCHEDULE_STANDARD: u32 = 1;
pub const PAYMENT_FEE_SCHEDULE_BUSY: u32 = 2;
pub const PAYMENT_FEE_SCHEDULE_PEAK: u32 = 3;
pub const PAYMENT_STANDARD_FEE_SCHEDULE_VERSION: u32 = PAYMENT_FEE_SCHEDULE_STANDARD;
pub const FEE_CLASS_PAYMENT_STANDARD: u32 = 1;
pub const FEE_AUX_ROUTE_HUSH_SIDECAR: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u32)]
pub enum TxKind {
    Payment = 1,
    ValidatorAction = 2,
    IssuerAction = 3,
    CredentialStateAction = 4,
    ProtocolAdminAction = 5,
}

impl TxKind {
    pub fn as_u32(self) -> u32 {
        self as u32
    }

    pub fn try_from_u32(value: u32) -> Result<Self, String> {
        match value {
            1 => Ok(Self::Payment),
            2 => Ok(Self::ValidatorAction),
            3 => Ok(Self::IssuerAction),
            4 => Ok(Self::CredentialStateAction),
            5 => Ok(Self::ProtocolAdminAction),
            _ => Err(format!("unsupported tx_kind {value}")),
        }
    }
}

pub const TX_KIND_PAYMENT: u32 = TxKind::Payment as u32;
pub const TX_KIND_VALIDATOR_ACTION: u32 = TxKind::ValidatorAction as u32;
pub const TX_KIND_ISSUER_ACTION: u32 = TxKind::IssuerAction as u32;
pub const TX_KIND_CREDENTIAL_STATE_ACTION: u32 = TxKind::CredentialStateAction as u32;
pub const TX_KIND_PROTOCOL_ADMIN_ACTION: u32 = TxKind::ProtocolAdminAction as u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum AssetId {
    Usdc = 1,
    Usdt = 2,
    Hush = 3,
}

impl AssetId {
    pub fn as_u32(self) -> u32 {
        self as u32
    }

    pub fn try_from_u32(value: u32) -> Result<Self, String> {
        match value {
            1 => Ok(Self::Usdc),
            2 => Ok(Self::Usdt),
            3 => Ok(Self::Hush),
            _ => Err(format!("unsupported asset id {value}")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaymentRoute {
    SameAsset,
    HushSidecar,
}

impl PaymentRoute {
    pub fn payment_fee_deduction(self, fee_amount: u64) -> u64 {
        match self {
            Self::SameAsset => fee_amount,
            Self::HushSidecar => 0,
        }
    }

    pub fn requires_hush_sidecar(self) -> bool {
        matches!(self, Self::HushSidecar)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeDescriptor {
    pub tx_kind: u32,
    pub payment_asset: u32,
    pub fee_asset: u32,
    pub fee_class: u32,
    pub fee_amount: u64,
    pub fee_schedule_version: u32,
    pub replay_domain: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteInput {
    pub amount: u64,
    pub randomness: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipientIntent {
    pub amount: u64,
    pub owner: [u32; 4],
    pub randomness: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SenderChangeIntent {
    pub amount: u64,
    pub randomness: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeAuxProofDescriptor {
    pub route: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentExecutionAttachment {
    pub sender_binding_tag: [u32; 4],
    pub fee_aux: Option<FeeAuxProofDescriptor>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentTxV1 {
    pub version: u32,
    pub descriptor: FeeDescriptor,
    pub inputs: [NoteInput; 2],
    pub recipient: RecipientIntent,
    pub sender_change: SenderChangeIntent,
    pub tx_binding_hash: [u32; 4],
    pub attachment: PaymentExecutionAttachment,
}

#[derive(Clone, Debug)]
pub struct PaymentMerkleContext {
    pub epoch: u32,
    pub note_root: [u32; 4],
    pub cred_root: [u32; 4],
    pub cred_issuer: u32,
    pub cred_expiry: u32,
    pub cred_secret: u32,
    pub note_path_0: [([u32; 4], u32); MERKLE_DEPTH],
    pub note_path_1: [([u32; 4], u32); MERKLE_DEPTH],
    pub cred_path: [([u32; 4], u32); MERKLE_DEPTH],
}

#[derive(Clone, Debug)]
pub struct HushFeeMerkleContext {
    pub note_root: [u32; 4],
    pub note_path_0: [([u32; 4], u32); MERKLE_DEPTH],
    pub note_path_1: [([u32; 4], u32); MERKLE_DEPTH],
}

impl PaymentTxV1 {
    pub fn build_same_asset(
        payment_asset: AssetId,
        inputs: [NoteInput; 2],
        recipient: RecipientIntent,
        sender_change_randomness: u32,
        sk: u32,
    ) -> Result<Self, String> {
        Self::build_same_asset_with_schedule(
            payment_asset,
            inputs,
            recipient,
            sender_change_randomness,
            sk,
            PAYMENT_FEE_SCHEDULE_STANDARD,
        )
    }

    pub fn build_same_asset_with_schedule(
        payment_asset: AssetId,
        inputs: [NoteInput; 2],
        recipient: RecipientIntent,
        sender_change_randomness: u32,
        sk: u32,
        fee_schedule_version: u32,
    ) -> Result<Self, String> {
        Self::build_for_route(
            PaymentRoute::SameAsset,
            payment_asset,
            inputs,
            recipient,
            sender_change_randomness,
            sk,
            fee_schedule_version,
        )
    }

    pub fn build_with_hush_fee(
        payment_asset: AssetId,
        inputs: [NoteInput; 2],
        recipient: RecipientIntent,
        sender_change_randomness: u32,
        sk: u32,
    ) -> Result<Self, String> {
        Self::build_with_hush_fee_with_schedule(
            payment_asset,
            inputs,
            recipient,
            sender_change_randomness,
            sk,
            PAYMENT_FEE_SCHEDULE_STANDARD,
        )
    }

    pub fn build_with_hush_fee_with_schedule(
        payment_asset: AssetId,
        inputs: [NoteInput; 2],
        recipient: RecipientIntent,
        sender_change_randomness: u32,
        sk: u32,
        fee_schedule_version: u32,
    ) -> Result<Self, String> {
        Self::build_for_route(
            PaymentRoute::HushSidecar,
            payment_asset,
            inputs,
            recipient,
            sender_change_randomness,
            sk,
            fee_schedule_version,
        )
    }

    fn build_for_route(
        route: PaymentRoute,
        payment_asset: AssetId,
        inputs: [NoteInput; 2],
        recipient: RecipientIntent,
        sender_change_randomness: u32,
        sk: u32,
        fee_schedule_version: u32,
    ) -> Result<Self, String> {
        let fee_asset = match route {
            PaymentRoute::SameAsset => payment_asset,
            PaymentRoute::HushSidecar => AssetId::Hush,
        };
        let fee_amount =
            expected_fee_amount(payment_asset.as_u32(), fee_asset.as_u32(), fee_schedule_version)?;
        let total_in = inputs[0]
            .amount
            .checked_add(inputs[1].amount)
            .ok_or_else(|| "input amount overflow".to_string())?;
        let payment_fee_deduction = route.payment_fee_deduction(fee_amount);
        let total_out = recipient
            .amount
            .checked_add(payment_fee_deduction)
            .ok_or_else(|| "output amount overflow".to_string())?;
        if total_in < total_out {
            return Err(format!(
                "insufficient input value: total inputs {total_in} < recipient {} + payment fee {payment_fee_deduction}",
                recipient.amount
            ));
        }

        let descriptor = FeeDescriptor {
            tx_kind: TX_KIND_PAYMENT,
            payment_asset: payment_asset.as_u32(),
            fee_asset: fee_asset.as_u32(),
            fee_class: FEE_CLASS_PAYMENT_STANDARD,
            fee_amount,
            fee_schedule_version,
            replay_domain: PAYMENT_TX_V1_REPLAY_DOMAIN,
        };
        let sender_change = SenderChangeIntent {
            amount: total_in - recipient.amount - payment_fee_deduction,
            randomness: sender_change_randomness,
        };

        let mut tx = Self {
            version: PAYMENT_TX_V1_VERSION,
            descriptor,
            inputs,
            recipient,
            sender_change,
            tx_binding_hash: [0; 4],
            attachment: PaymentExecutionAttachment {
                sender_binding_tag: [0; 4],
                fee_aux: route
                    .requires_hush_sidecar()
                    .then_some(FeeAuxProofDescriptor { route: FEE_AUX_ROUTE_HUSH_SIDECAR }),
            },
        };
        tx.tx_binding_hash = compute_tx_binding_hash(&tx);
        tx.attachment.sender_binding_tag = derive_sender_binding_tag(sk, tx.tx_binding_hash);
        Ok(tx)
    }

    pub fn total_input_amount(&self) -> Result<u64, String> {
        self.inputs[0]
            .amount
            .checked_add(self.inputs[1].amount)
            .ok_or_else(|| "input amount overflow".to_string())
    }

    pub fn route(&self) -> Result<PaymentRoute, String> {
        payment_route(self.descriptor.payment_asset, self.descriptor.fee_asset)
    }

    pub fn build_witness(
        &self,
        sk: u32,
        context: &PaymentMerkleContext,
    ) -> Result<PaymentWitness, String> {
        let route = validate_payment_tx(self)?;
        let expected_sender_binding_tag = derive_sender_binding_tag(sk, self.tx_binding_hash);
        if self.attachment.sender_binding_tag != expected_sender_binding_tag {
            return Err(format!(
                "sender_binding_tag mismatch: tx {:?}, expected {:?}",
                self.attachment.sender_binding_tag, expected_sender_binding_tag
            ));
        }

        Ok(PaymentWitness {
            epoch: context.epoch,
            note_root: context.note_root,
            cred_root: context.cred_root,
            sk,
            in_asset: self.descriptor.payment_asset,
            in_amt_0: self.inputs[0].amount,
            in_rand_0: self.inputs[0].randomness,
            in_amt_1: self.inputs[1].amount,
            in_rand_1: self.inputs[1].randomness,
            out_amt_0: self.recipient.amount,
            out_owner_0: self.recipient.owner,
            out_rand_0: self.recipient.randomness,
            out_amt_1: self.sender_change.amount,
            out_rand_1: self.sender_change.randomness,
            payment_fee_amount: route.payment_fee_deduction(self.descriptor.fee_amount),
            binding_fee_asset: self.descriptor.fee_asset,
            fee_amount: self.descriptor.fee_amount,
            fee_class: self.descriptor.fee_class,
            fee_schedule_version: self.descriptor.fee_schedule_version,
            replay_domain: self.descriptor.replay_domain,
            tx_binding_hash: self.tx_binding_hash,
            sender_binding_tag: self.attachment.sender_binding_tag,
            cred_issuer: context.cred_issuer,
            cred_expiry: context.cred_expiry,
            cred_secret: context.cred_secret,
            note_path_0: context.note_path_0,
            note_path_1: context.note_path_1,
            cred_path: context.cred_path,
        })
    }

    pub fn build_hush_fee_witness(
        &self,
        sk: u32,
        inputs: [NoteInput; 2],
        change: SenderChangeIntent,
        context: &HushFeeMerkleContext,
    ) -> Result<HushFeeWitness, String> {
        let route = validate_payment_tx(self)?;
        if route != PaymentRoute::HushSidecar {
            return Err(
                "HUSH fee sidecar witness is only valid for Mode B transactions".to_string()
            );
        }

        let expected_sender_binding_tag = derive_sender_binding_tag(sk, self.tx_binding_hash);
        if self.attachment.sender_binding_tag != expected_sender_binding_tag {
            return Err(format!(
                "sender_binding_tag mismatch: tx {:?}, expected {:?}",
                self.attachment.sender_binding_tag, expected_sender_binding_tag
            ));
        }

        let total_in = inputs[0]
            .amount
            .checked_add(inputs[1].amount)
            .ok_or_else(|| "HUSH sidecar input amount overflow".to_string())?;
        let expected_change = total_in
            .checked_sub(self.descriptor.fee_amount)
            .ok_or_else(|| "insufficient HUSH fee coverage".to_string())?;
        if change.amount != expected_change {
            return Err(format!(
                "invalid HUSH change: got {}, expected {}",
                change.amount, expected_change
            ));
        }

        Ok(HushFeeWitness {
            note_root: context.note_root,
            sk,
            in_amt_0: inputs[0].amount,
            in_rand_0: inputs[0].randomness,
            in_amt_1: inputs[1].amount,
            in_rand_1: inputs[1].randomness,
            change_amt: change.amount,
            change_rand: change.randomness,
            fee_amount: self.descriptor.fee_amount,
            tx_binding_hash: self.tx_binding_hash,
            sender_binding_tag: self.attachment.sender_binding_tag,
            note_path_0: context.note_path_0,
            note_path_1: context.note_path_1,
        })
    }
}

pub fn payment_route(payment_asset: u32, fee_asset: u32) -> Result<PaymentRoute, String> {
    let payment_asset = AssetId::try_from_u32(payment_asset)?;
    let fee_asset = AssetId::try_from_u32(fee_asset)?;
    match (payment_asset, fee_asset) {
        (AssetId::Usdc, AssetId::Usdc) | (AssetId::Usdt, AssetId::Usdt) => {
            Ok(PaymentRoute::SameAsset)
        }
        (AssetId::Usdc, AssetId::Hush) | (AssetId::Usdt, AssetId::Hush) => {
            Ok(PaymentRoute::HushSidecar)
        }
        (AssetId::Hush, _) => Err("HUSH is not a valid payment asset for PaymentTxV1".to_string()),
        (AssetId::Usdc, AssetId::Usdt) | (AssetId::Usdt, AssetId::Usdc) => Err(format!(
            "cross-stablecoin fee mismatch is invalid: payment asset {} with fee asset {}",
            payment_asset.as_u32(),
            fee_asset.as_u32()
        )),
    }
}

pub fn validate_tx_kind_fee_asset_policy(
    tx_kind: u32,
    payment_asset: Option<u32>,
    fee_asset: u32,
) -> Result<(), String> {
    let tx_kind = TxKind::try_from_u32(tx_kind)?;
    let fee_asset = AssetId::try_from_u32(fee_asset)?;
    match tx_kind {
        TxKind::Payment => {
            let payment_asset = payment_asset
                .ok_or_else(|| "payment tx_kind requires payment_asset".to_string())?;
            payment_route(payment_asset, fee_asset.as_u32()).map(|_| ())
        }
        TxKind::ValidatorAction
        | TxKind::IssuerAction
        | TxKind::CredentialStateAction
        | TxKind::ProtocolAdminAction => {
            if fee_asset != AssetId::Hush {
                return Err(format!(
                    "tx_kind {} is HUSH-only and rejects fee asset {}",
                    tx_kind.as_u32(),
                    fee_asset.as_u32()
                ));
            }
            Ok(())
        }
    }
}

pub fn is_hush_only_action(tx_kind: u32) -> Result<bool, String> {
    Ok(matches!(
        TxKind::try_from_u32(tx_kind)?,
        TxKind::ValidatorAction
            | TxKind::IssuerAction
            | TxKind::CredentialStateAction
            | TxKind::ProtocolAdminAction
    ))
}

pub fn expected_fee_amount(
    payment_asset: u32,
    fee_asset: u32,
    fee_schedule_version: u32,
) -> Result<u64, String> {
    validate_tx_kind_fee_asset_policy(TX_KIND_PAYMENT, Some(payment_asset), fee_asset)?;
    let route = payment_route(payment_asset, fee_asset)?;
    match (route, fee_schedule_version) {
        (PaymentRoute::SameAsset, PAYMENT_FEE_SCHEDULE_STANDARD) => Ok(50),
        (PaymentRoute::SameAsset, PAYMENT_FEE_SCHEDULE_BUSY) => Ok(125),
        (PaymentRoute::SameAsset, PAYMENT_FEE_SCHEDULE_PEAK) => Ok(300),
        (PaymentRoute::HushSidecar, PAYMENT_FEE_SCHEDULE_STANDARD) => Ok(50),
        (PaymentRoute::HushSidecar, PAYMENT_FEE_SCHEDULE_BUSY) => Ok(125),
        (PaymentRoute::HushSidecar, PAYMENT_FEE_SCHEDULE_PEAK) => Ok(300),
        _ => Err(format!("unsupported payment fee schedule version {fee_schedule_version}")),
    }
}

pub fn compute_tx_binding_hash(tx: &PaymentTxV1) -> [u32; 4] {
    compute_mode_a_tx_binding_hash(
        tx.descriptor.replay_domain,
        tx.descriptor.payment_asset,
        tx.descriptor.fee_asset,
        tx.descriptor.fee_class,
        tx.descriptor.fee_amount,
        tx.descriptor.fee_schedule_version,
        tx.recipient.amount,
        tx.recipient.owner,
        tx.recipient.randomness,
        tx.sender_change.amount,
        tx.sender_change.randomness,
    )
}

/// Split a u64 amount into two M31 elements (lo = lower 31 bits, hi = upper bits).
/// Both halves are guaranteed to fit in M31 for amounts up to 2^60 - 1.
fn amount_to_m31_pair(amount: u64) -> (M31, M31) {
    let lo = (amount & 0x7FFF_FFFF) as u32;
    let hi = (amount >> 31) as u32;
    (M31::from(lo), M31::from(hi))
}

pub fn compute_mode_a_tx_binding_hash(
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
) -> [u32; 4] {
    let (fee_lo, fee_hi) = amount_to_m31_pair(fee_amount);
    let (recip_lo, recip_hi) = amount_to_m31_pair(recipient_amount);
    let (change_lo, change_hi) = amount_to_m31_pair(sender_change_amount);
    let owner = poseidon2::u32_array_to_hashout(recipient_owner);

    let chunk_0 = poseidon2::domain_hash4(
        M31::from(replay_domain),
        M31::from(TX_KIND_PAYMENT),
        M31::from(payment_asset),
        M31::from(fee_asset),
        poseidon2::DOMAIN_TX_BINDING,
    );
    let chunk_1 = poseidon2::domain_hash4(
        M31::from(fee_class),
        fee_lo,
        fee_hi,
        M31::from(fee_schedule_version),
        poseidon2::DOMAIN_TX_BINDING,
    );
    let chunk_2 = poseidon2::domain_hash7(
        recip_lo,
        recip_hi,
        owner[0],
        owner[1],
        owner[2],
        owner[3],
        M31::from(recipient_randomness),
        poseidon2::DOMAIN_TX_BINDING,
    );
    let chunk_3 = poseidon2::domain_hash4(
        change_lo,
        change_hi,
        M31::from(sender_change_randomness),
        M31::from(0u32),
        poseidon2::DOMAIN_TX_BINDING,
    );
    let left = poseidon2::hash_pair(chunk_0, chunk_1, poseidon2::DOMAIN_TX_BINDING);
    let mid = poseidon2::hash_pair(left, chunk_2, poseidon2::DOMAIN_TX_BINDING);
    let result = poseidon2::hash_pair(mid, chunk_3, poseidon2::DOMAIN_TX_BINDING);
    poseidon2::hashout_to_u32_array(result)
}

pub fn derive_sender_binding_tag(sk: u32, tx_binding_hash: [u32; 4]) -> [u32; 4] {
    let h = poseidon2::u32_array_to_hashout(tx_binding_hash);
    let result = poseidon2::sponge_hash(
        &[M31::from(sk), h[0], h[1], h[2], h[3]],
        poseidon2::DOMAIN_SENDER_BINDING,
    );
    poseidon2::hashout_to_u32_array(result)
}

pub fn validate_payment_tx(tx: &PaymentTxV1) -> Result<PaymentRoute, String> {
    if tx.version != PAYMENT_TX_V1_VERSION {
        return Err(format!("unsupported payment tx version {}", tx.version));
    }
    if tx.descriptor.tx_kind != TX_KIND_PAYMENT {
        return Err(format!("unsupported tx_kind {}", tx.descriptor.tx_kind));
    }
    validate_tx_kind_fee_asset_policy(
        tx.descriptor.tx_kind,
        Some(tx.descriptor.payment_asset),
        tx.descriptor.fee_asset,
    )?;
    if tx.descriptor.fee_class != FEE_CLASS_PAYMENT_STANDARD {
        return Err(format!("unsupported fee_class {}", tx.descriptor.fee_class));
    }
    if !matches!(
        tx.descriptor.fee_schedule_version,
        PAYMENT_FEE_SCHEDULE_STANDARD | PAYMENT_FEE_SCHEDULE_BUSY | PAYMENT_FEE_SCHEDULE_PEAK
    ) {
        return Err(format!(
            "unsupported fee_schedule_version {}",
            tx.descriptor.fee_schedule_version
        ));
    }
    if tx.descriptor.replay_domain != PAYMENT_TX_V1_REPLAY_DOMAIN {
        return Err(format!("invalid replay_domain {}", tx.descriptor.replay_domain));
    }

    let route = payment_route(tx.descriptor.payment_asset, tx.descriptor.fee_asset)?;
    let expected_fee = expected_fee_amount(
        tx.descriptor.payment_asset,
        tx.descriptor.fee_asset,
        tx.descriptor.fee_schedule_version,
    )?;
    if tx.descriptor.fee_amount != expected_fee {
        return Err(format!(
            "fee amount mismatch: got {}, expected {}",
            tx.descriptor.fee_amount, expected_fee
        ));
    }

    match (&tx.attachment.fee_aux, route) {
        (None, PaymentRoute::SameAsset) => {}
        (Some(fee_aux), PaymentRoute::HushSidecar) => {
            if fee_aux.route != FEE_AUX_ROUTE_HUSH_SIDECAR {
                return Err(format!("unsupported fee aux proof route {}", fee_aux.route));
            }
        }
        (None, PaymentRoute::HushSidecar) => {
            return Err("missing HUSH sidecar attachment for fee_asset = HUSH".to_string());
        }
        (Some(_), PaymentRoute::SameAsset) => {
            return Err("fee sidecar attachment is disallowed for same-asset Mode A".to_string());
        }
    }

    let total_in = tx.total_input_amount()?;
    let expected_change = total_in
        .checked_sub(tx.recipient.amount)
        .and_then(|value| value.checked_sub(route.payment_fee_deduction(tx.descriptor.fee_amount)))
        .ok_or_else(|| "insufficient input value after fee deduction".to_string())?;
    if tx.sender_change.amount != expected_change {
        return Err(format!(
            "sender change mismatch: got {}, expected {}",
            tx.sender_change.amount, expected_change
        ));
    }

    let expected_binding_hash = compute_tx_binding_hash(tx);
    if tx.tx_binding_hash != expected_binding_hash {
        return Err(format!(
            "tx_binding_hash mismatch: got {:?}, expected {:?}",
            tx.tx_binding_hash, expected_binding_hash
        ));
    }

    Ok(route)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_same_asset_tx(asset: AssetId) -> PaymentTxV1 {
        PaymentTxV1::build_same_asset(
            asset,
            [
                NoteInput { amount: 7_000, randomness: 111 },
                NoteInput { amount: 3_000, randomness: 222 },
            ],
            RecipientIntent { amount: 8_000, owner: [99_999, 0, 0, 0], randomness: 333 },
            444,
            12_345,
        )
        .expect("sample tx should build")
    }

    fn sample_hush_fee_tx(asset: AssetId) -> PaymentTxV1 {
        PaymentTxV1::build_with_hush_fee(
            asset,
            [
                NoteInput { amount: 7_000, randomness: 111 },
                NoteInput { amount: 3_000, randomness: 222 },
            ],
            RecipientIntent { amount: 8_000, owner: [99_999, 0, 0, 0], randomness: 333 },
            444,
            12_345,
        )
        .expect("sample tx should build")
    }

    #[test]
    fn test_builder_output_is_canonical_same_asset() {
        let tx = sample_same_asset_tx(AssetId::Usdc);
        assert_eq!(tx.descriptor.payment_asset, AssetId::Usdc as u32);
        assert_eq!(tx.descriptor.fee_asset, AssetId::Usdc as u32);
        assert_eq!(tx.descriptor.fee_amount, 50);
        assert_eq!(tx.descriptor.fee_schedule_version, PAYMENT_FEE_SCHEDULE_STANDARD);
        assert_eq!(tx.sender_change.amount, 1_950);
        assert!(tx.attachment.fee_aux.is_none());
        validate_payment_tx(&tx).expect("builder output should validate");
    }

    #[test]
    fn test_builder_output_is_canonical_hush_sidecar() {
        let tx = sample_hush_fee_tx(AssetId::Usdc);
        assert_eq!(tx.descriptor.payment_asset, AssetId::Usdc as u32);
        assert_eq!(tx.descriptor.fee_asset, AssetId::Hush as u32);
        assert_eq!(tx.sender_change.amount, 2_000);
        assert_eq!(
            tx.attachment.fee_aux.as_ref().map(|fee_aux| fee_aux.route),
            Some(FEE_AUX_ROUTE_HUSH_SIDECAR)
        );
        validate_payment_tx(&tx).expect("Mode B builder output should validate");
    }

    #[test]
    fn test_tx_binding_hash_is_deterministic() {
        let tx_a = sample_same_asset_tx(AssetId::Usdc);
        let tx_b = sample_same_asset_tx(AssetId::Usdc);
        assert_eq!(tx_a.tx_binding_hash, tx_b.tx_binding_hash);
    }

    #[test]
    fn test_sender_binding_tag_is_deterministic() {
        let tx = sample_same_asset_tx(AssetId::Usdc);
        assert_eq!(
            derive_sender_binding_tag(12_345, tx.tx_binding_hash),
            derive_sender_binding_tag(12_345, tx.tx_binding_hash)
        );
    }

    #[test]
    fn test_fixture_roundtrip_parsing() {
        let tx = sample_hush_fee_tx(AssetId::Usdt);
        let json = serde_json::to_string(&tx).expect("serialize tx");
        let parsed: PaymentTxV1 = serde_json::from_str(&json).expect("parse tx");
        assert_eq!(parsed, tx);
        validate_payment_tx(&parsed).expect("parsed tx should validate");
    }

    #[test]
    fn test_cross_stable_fee_mismatch_rejected() {
        let mut tx = sample_same_asset_tx(AssetId::Usdc);
        tx.descriptor.fee_asset = AssetId::Usdt as u32;
        assert!(validate_payment_tx(&tx).is_err());
    }

    #[test]
    fn test_wrong_binding_hash_rejected() {
        let mut tx = sample_hush_fee_tx(AssetId::Usdt);
        tx.tx_binding_hash[0] = tx.tx_binding_hash[0].wrapping_add(1);
        assert!(validate_payment_tx(&tx).is_err());
    }

    #[test]
    fn test_wrong_replay_domain_rejected() {
        let mut tx = sample_same_asset_tx(AssetId::Usdc);
        tx.descriptor.replay_domain = PAYMENT_TX_V1_REPLAY_DOMAIN + 1;
        assert!(validate_payment_tx(&tx).is_err());
    }

    #[test]
    fn test_malformed_fee_descriptor_rejected() {
        let mut tx = sample_same_asset_tx(AssetId::Usdc);
        tx.descriptor.fee_class = 99;
        assert!(validate_payment_tx(&tx).is_err());
    }

    #[test]
    fn test_missing_sidecar_when_required_rejected() {
        let mut tx = sample_hush_fee_tx(AssetId::Usdc);
        tx.attachment.fee_aux = None;
        assert!(validate_payment_tx(&tx).is_err());
    }

    #[test]
    fn test_sidecar_rejected_when_disallowed() {
        let mut tx = sample_same_asset_tx(AssetId::Usdt);
        tx.attachment.fee_aux = Some(FeeAuxProofDescriptor { route: FEE_AUX_ROUTE_HUSH_SIDECAR });
        assert!(validate_payment_tx(&tx).is_err());
    }

    #[test]
    fn test_fee_amount_mismatch_rejected() {
        let mut tx = sample_hush_fee_tx(AssetId::Usdt);
        tx.descriptor.fee_amount += 1;
        assert!(validate_payment_tx(&tx).is_err());
    }

    #[test]
    fn test_hush_only_action_fee_policy() {
        validate_tx_kind_fee_asset_policy(TX_KIND_VALIDATOR_ACTION, None, AssetId::Hush as u32)
            .expect("validator action should accept HUSH");
        assert!(validate_tx_kind_fee_asset_policy(
            TX_KIND_VALIDATOR_ACTION,
            None,
            AssetId::Usdc as u32,
        )
        .is_err());
    }

    #[test]
    fn test_payment_fee_policy_mapping() {
        validate_tx_kind_fee_asset_policy(
            TX_KIND_PAYMENT,
            Some(AssetId::Usdc as u32),
            AssetId::Usdc as u32,
        )
        .expect("USDC same-asset path should be allowed");
        validate_tx_kind_fee_asset_policy(
            TX_KIND_PAYMENT,
            Some(AssetId::Usdt as u32),
            AssetId::Hush as u32,
        )
        .expect("USDT HUSH sidecar path should be allowed");
        assert!(validate_tx_kind_fee_asset_policy(
            TX_KIND_PAYMENT,
            Some(AssetId::Usdt as u32),
            AssetId::Usdc as u32,
        )
        .is_err());
    }
}
