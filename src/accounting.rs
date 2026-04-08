use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    payment_tx::{
        validate_tx_kind_fee_asset_policy, AssetId, PaymentTxV1, TxKind,
        PAYMENT_STANDARD_FEE_SCHEDULE_VERSION, PAYMENT_TX_V1_REPLAY_DOMAIN,
    },
    payment_validation::{validate_payment_bundle, PaymentBundleProof},
};

const BPS_DENOMINATOR: u64 = 10_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetFeeBuckets {
    pub hush: u64,
    pub usdc: u64,
    pub usdt: u64,
}

impl AssetFeeBuckets {
    pub fn increment(&mut self, asset: AssetId, amount: u64) -> Result<(), String> {
        match asset {
            AssetId::Hush => {
                self.hush = self
                    .hush
                    .checked_add(amount)
                    .ok_or_else(|| "HUSH fee bucket overflow".to_string())?;
            }
            AssetId::Usdc => {
                self.usdc = self
                    .usdc
                    .checked_add(amount)
                    .ok_or_else(|| "USDC fee bucket overflow".to_string())?;
            }
            AssetId::Usdt => {
                self.usdt = self
                    .usdt
                    .checked_add(amount)
                    .ok_or_else(|| "USDT fee bucket overflow".to_string())?;
            }
        }
        Ok(())
    }

    pub fn add_assign(&mut self, other: &Self) -> Result<(), String> {
        self.hush = self
            .hush
            .checked_add(other.hush)
            .ok_or_else(|| "HUSH epoch fee pool overflow".to_string())?;
        self.usdc = self
            .usdc
            .checked_add(other.usdc)
            .ok_or_else(|| "USDC epoch fee pool overflow".to_string())?;
        self.usdt = self
            .usdt
            .checked_add(other.usdt)
            .ok_or_else(|| "USDT epoch fee pool overflow".to_string())?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetTxCounts {
    pub hush: u32,
    pub usdc: u32,
    pub usdt: u32,
}

impl AssetTxCounts {
    pub fn increment(&mut self, asset: AssetId) -> Result<(), String> {
        match asset {
            AssetId::Hush => {
                self.hush =
                    self.hush.checked_add(1).ok_or_else(|| "HUSH tx count overflow".to_string())?;
            }
            AssetId::Usdc => {
                self.usdc =
                    self.usdc.checked_add(1).ok_or_else(|| "USDC tx count overflow".to_string())?;
            }
            AssetId::Usdt => {
                self.usdt =
                    self.usdt.checked_add(1).ok_or_else(|| "USDT tx count overflow".to_string())?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedTxRecord {
    pub tx_id: u64,
    pub tx_kind: u32,
    pub fee_asset: u32,
    pub fee_amount: u64,
    pub fee_schedule_version: u32,
}

pub fn accepted_payment_record(
    tx: &PaymentTxV1,
    bundle: &PaymentBundleProof,
) -> Result<AcceptedTxRecord, String> {
    validate_payment_bundle(tx, bundle)?;
    Ok(AcceptedTxRecord {
        tx_id: tx.tx_binding_hash[0] as u64 | ((tx.tx_binding_hash[1] as u64) << 31),
        tx_kind: tx.descriptor.tx_kind,
        fee_asset: tx.descriptor.fee_asset,
        fee_amount: tx.descriptor.fee_amount,
        fee_schedule_version: tx.descriptor.fee_schedule_version,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolActionTx {
    pub action_id: u64,
    pub tx_kind: u32,
    pub fee_asset: u32,
    pub fee_amount: u64,
    pub fee_schedule_version: u32,
    pub replay_domain: u32,
}

impl ProtocolActionTx {
    pub fn build(tx_kind: TxKind, action_id: u64, fee_amount: u64) -> Result<Self, String> {
        if tx_kind == TxKind::Payment {
            return Err("ProtocolActionTx cannot use payment tx_kind".to_string());
        }
        Ok(Self {
            action_id,
            tx_kind: tx_kind.as_u32(),
            fee_asset: AssetId::Hush as u32,
            fee_amount,
            fee_schedule_version: PAYMENT_STANDARD_FEE_SCHEDULE_VERSION,
            replay_domain: PAYMENT_TX_V1_REPLAY_DOMAIN,
        })
    }
}

pub fn validate_protocol_action_tx(tx: &ProtocolActionTx) -> Result<TxKind, String> {
    let tx_kind = TxKind::try_from_u32(tx.tx_kind)?;
    if tx_kind == TxKind::Payment {
        return Err("payment tx_kind must use PaymentTxV1".to_string());
    }
    validate_tx_kind_fee_asset_policy(tx.tx_kind, None, tx.fee_asset)?;
    if tx.fee_schedule_version != PAYMENT_STANDARD_FEE_SCHEDULE_VERSION {
        return Err(format!("unsupported fee_schedule_version {}", tx.fee_schedule_version));
    }
    if tx.replay_domain != PAYMENT_TX_V1_REPLAY_DOMAIN {
        return Err(format!("invalid replay_domain {}", tx.replay_domain));
    }
    if tx.fee_amount == 0 {
        return Err("protocol action fee amount must be non-zero".to_string());
    }
    Ok(tx_kind)
}

pub fn accepted_protocol_action_record(tx: &ProtocolActionTx) -> Result<AcceptedTxRecord, String> {
    validate_protocol_action_tx(tx)?;
    Ok(AcceptedTxRecord {
        tx_id: tx.action_id,
        tx_kind: tx.tx_kind,
        fee_asset: tx.fee_asset,
        fee_amount: tx.fee_amount,
        fee_schedule_version: tx.fee_schedule_version,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockAccountingRecord {
    pub block_height: u64,
    pub proposer_id: u32,
    pub fee_buckets: AssetFeeBuckets,
    pub tx_counts_by_asset: AssetTxCounts,
    pub tx_counts_by_kind: BTreeMap<u32, u32>,
    pub accepted_transactions: Vec<AcceptedTxRecord>,
}

impl BlockAccountingRecord {
    pub fn validate(&self) -> Result<(), String> {
        let mut recomputed_buckets = AssetFeeBuckets::default();
        let mut recomputed_counts_by_asset = AssetTxCounts::default();
        let mut recomputed_counts_by_kind = BTreeMap::new();
        let mut tx_ids = BTreeSet::new();

        for tx in &self.accepted_transactions {
            if !tx_ids.insert(tx.tx_id) {
                return Err(format!("duplicate tx_id {} in block accounting record", tx.tx_id));
            }
            let fee_asset = AssetId::try_from_u32(tx.fee_asset)?;
            recomputed_buckets.increment(fee_asset, tx.fee_amount)?;
            recomputed_counts_by_asset.increment(fee_asset)?;
            *recomputed_counts_by_kind.entry(tx.tx_kind).or_insert(0) += 1;
        }

        if self.fee_buckets != recomputed_buckets {
            return Err("block fee buckets do not reconcile with accepted transactions".to_string());
        }
        if self.tx_counts_by_asset != recomputed_counts_by_asset {
            return Err("block tx counts by asset do not reconcile".to_string());
        }
        if self.tx_counts_by_kind != recomputed_counts_by_kind {
            return Err("block tx counts by kind do not reconcile".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct BlockAccountingBuilder {
    block_height: u64,
    proposer_id: u32,
    fee_buckets: AssetFeeBuckets,
    tx_counts_by_asset: AssetTxCounts,
    tx_counts_by_kind: BTreeMap<u32, u32>,
    accepted_transactions: Vec<AcceptedTxRecord>,
    seen_tx_ids: BTreeSet<u64>,
}

impl BlockAccountingBuilder {
    pub fn new(block_height: u64, proposer_id: u32) -> Self {
        Self {
            block_height,
            proposer_id,
            fee_buckets: AssetFeeBuckets::default(),
            tx_counts_by_asset: AssetTxCounts::default(),
            tx_counts_by_kind: BTreeMap::new(),
            accepted_transactions: Vec::new(),
            seen_tx_ids: BTreeSet::new(),
        }
    }

    pub fn record_payment_bundle(
        &mut self,
        tx: &PaymentTxV1,
        bundle: &PaymentBundleProof,
    ) -> Result<(), String> {
        let record = accepted_payment_record(tx, bundle)?;
        self.record_accepted_tx_record(&record)
    }

    pub fn record_protocol_action(&mut self, tx: &ProtocolActionTx) -> Result<(), String> {
        let record = accepted_protocol_action_record(tx)?;
        self.record_accepted_tx_record(&record)
    }

    pub fn record_accepted_tx_record(&mut self, record: &AcceptedTxRecord) -> Result<(), String> {
        if !self.seen_tx_ids.insert(record.tx_id) {
            return Err(format!("tx_id {} already counted in this block", record.tx_id));
        }
        let fee_asset = AssetId::try_from_u32(record.fee_asset)?;
        self.fee_buckets.increment(fee_asset, record.fee_amount)?;
        self.tx_counts_by_asset.increment(fee_asset)?;
        *self.tx_counts_by_kind.entry(record.tx_kind).or_insert(0) += 1;
        self.accepted_transactions.push(record.clone());
        Ok(())
    }

    pub fn fee_buckets(&self) -> AssetFeeBuckets {
        self.fee_buckets
    }

    pub fn finalize(self) -> BlockAccountingRecord {
        BlockAccountingRecord {
            block_height: self.block_height,
            proposer_id: self.proposer_id,
            fee_buckets: self.fee_buckets,
            tx_counts_by_asset: self.tx_counts_by_asset,
            tx_counts_by_kind: self.tx_counts_by_kind,
            accepted_transactions: self.accepted_transactions,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorStakeInfo {
    pub validator_id: u32,
    pub payout_key: u32,
    pub effective_stake: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorBlockParticipation {
    pub validator_id: u32,
    pub signed_block: bool,
    pub liveness_penalty_bps: u16,
    pub slash_penalty_bps: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorEpochEntitlement {
    pub validator_id: u32,
    pub payout_key: u32,
    pub effective_stake: u64,
    pub present_blocks: u32,
    pub missed_blocks: u32,
    pub max_liveness_penalty_bps: u16,
    pub max_slash_penalty_bps: u16,
    pub entitlement: AssetFeeBuckets,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimablePayoutRecord {
    pub epoch_index: u64,
    pub validator_id: u32,
    pub payout_key: u32,
    pub entitlement: AssetFeeBuckets,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochSettlement {
    pub epoch_index: u64,
    pub fee_pools: AssetFeeBuckets,
    pub validator_entitlements: BTreeMap<u32, ValidatorEpochEntitlement>,
    pub payout_records: Vec<ClaimablePayoutRecord>,
    pub applied_block_count: u32,
}

impl EpochSettlement {
    pub fn total_payouts(&self) -> Result<AssetFeeBuckets, String> {
        let mut totals = AssetFeeBuckets::default();
        for payout in &self.payout_records {
            totals.add_assign(&payout.entitlement)?;
        }
        Ok(totals)
    }

    pub fn payout_record_for_validator(&self, validator_id: u32) -> Option<&ClaimablePayoutRecord> {
        self.payout_records.iter().find(|record| record.validator_id == validator_id)
    }
}

pub struct EpochAccumulator {
    epoch_index: u64,
    fee_pools: AssetFeeBuckets,
    validator_entitlements: BTreeMap<u32, ValidatorEpochEntitlement>,
    applied_blocks: BTreeSet<u64>,
    seen_tx_ids: BTreeSet<u64>,
}

impl EpochAccumulator {
    pub fn new(epoch_index: u64) -> Self {
        Self {
            epoch_index,
            fee_pools: AssetFeeBuckets::default(),
            validator_entitlements: BTreeMap::new(),
            applied_blocks: BTreeSet::new(),
            seen_tx_ids: BTreeSet::new(),
        }
    }

    pub fn fee_pools(&self) -> AssetFeeBuckets {
        self.fee_pools
    }

    pub fn apply_block(
        &mut self,
        block: &BlockAccountingRecord,
        validator_set: &[ValidatorStakeInfo],
        participation: &[ValidatorBlockParticipation],
    ) -> Result<(), String> {
        block.validate()?;
        if !self.applied_blocks.insert(block.block_height) {
            return Err(format!(
                "block height {} already accrued in epoch {}",
                block.block_height, self.epoch_index
            ));
        }
        for tx in &block.accepted_transactions {
            if !self.seen_tx_ids.insert(tx.tx_id) {
                return Err(format!("tx_id {} already accrued in this epoch", tx.tx_id));
            }
        }

        let validator_map = build_validator_map(validator_set)?;
        let participation_map = build_participation_map(participation, &validator_map)?;
        self.fee_pools.add_assign(&block.fee_buckets)?;

        let weights = validator_set
            .iter()
            .filter_map(|validator| {
                let participation = participation_map.get(&validator.validator_id);
                let signed_block = participation.map(|entry| entry.signed_block).unwrap_or(false);
                if !signed_block {
                    return None;
                }

                let penalties = participation
                    .map(|entry| {
                        u64::from(entry.liveness_penalty_bps) + u64::from(entry.slash_penalty_bps)
                    })
                    .unwrap_or(0)
                    .min(BPS_DENOMINATOR);
                let weight = validator
                    .effective_stake
                    .checked_mul(BPS_DENOMINATOR - penalties)
                    .expect("validator weight multiplication should fit u64 in tests");
                if weight == 0 {
                    None
                } else {
                    Some((validator.validator_id, weight))
                }
            })
            .collect::<Vec<_>>();

        if (block.fee_buckets.hush > 0 || block.fee_buckets.usdc > 0 || block.fee_buckets.usdt > 0)
            && weights.is_empty()
        {
            return Err("cannot accrue non-zero fee buckets without active validators".to_string());
        }

        let hush_allocations = allocate_asset_bucket(block.fee_buckets.hush, &weights)?;
        let usdc_allocations = allocate_asset_bucket(block.fee_buckets.usdc, &weights)?;
        let usdt_allocations = allocate_asset_bucket(block.fee_buckets.usdt, &weights)?;

        for validator in validator_set {
            let participation = participation_map.get(&validator.validator_id);
            let entry =
                self.validator_entitlements.entry(validator.validator_id).or_insert_with(|| {
                    ValidatorEpochEntitlement {
                        validator_id: validator.validator_id,
                        payout_key: validator.payout_key,
                        effective_stake: validator.effective_stake,
                        present_blocks: 0,
                        missed_blocks: 0,
                        max_liveness_penalty_bps: 0,
                        max_slash_penalty_bps: 0,
                        entitlement: AssetFeeBuckets::default(),
                    }
                });

            entry.effective_stake = validator.effective_stake;
            entry.payout_key = validator.payout_key;
            match participation {
                Some(participation) if participation.signed_block => {
                    entry.present_blocks += 1;
                    entry.max_liveness_penalty_bps =
                        entry.max_liveness_penalty_bps.max(participation.liveness_penalty_bps);
                    entry.max_slash_penalty_bps =
                        entry.max_slash_penalty_bps.max(participation.slash_penalty_bps);
                }
                _ => {
                    entry.missed_blocks += 1;
                }
            }

            entry.entitlement.hush = entry
                .entitlement
                .hush
                .checked_add(*hush_allocations.get(&validator.validator_id).unwrap_or(&0))
                .ok_or_else(|| "validator HUSH entitlement overflow".to_string())?;
            entry.entitlement.usdc = entry
                .entitlement
                .usdc
                .checked_add(*usdc_allocations.get(&validator.validator_id).unwrap_or(&0))
                .ok_or_else(|| "validator USDC entitlement overflow".to_string())?;
            entry.entitlement.usdt = entry
                .entitlement
                .usdt
                .checked_add(*usdt_allocations.get(&validator.validator_id).unwrap_or(&0))
                .ok_or_else(|| "validator USDT entitlement overflow".to_string())?;
        }

        Ok(())
    }

    pub fn close(self) -> Result<EpochSettlement, String> {
        let mut payout_records = Vec::new();
        for entitlement in self.validator_entitlements.values() {
            if entitlement.entitlement == AssetFeeBuckets::default() {
                continue;
            }
            payout_records.push(ClaimablePayoutRecord {
                epoch_index: self.epoch_index,
                validator_id: entitlement.validator_id,
                payout_key: entitlement.payout_key,
                entitlement: entitlement.entitlement,
            });
        }

        let settlement = EpochSettlement {
            epoch_index: self.epoch_index,
            fee_pools: self.fee_pools,
            validator_entitlements: self.validator_entitlements,
            payout_records,
            applied_block_count: self.applied_blocks.len() as u32,
        };
        let payout_totals = settlement.total_payouts()?;
        if payout_totals != settlement.fee_pools {
            return Err("payout totals do not reconcile with epoch fee pools".to_string());
        }
        Ok(settlement)
    }
}

fn build_validator_map(
    validator_set: &[ValidatorStakeInfo],
) -> Result<BTreeMap<u32, &ValidatorStakeInfo>, String> {
    let mut validator_map = BTreeMap::new();
    for validator in validator_set {
        if validator_map.insert(validator.validator_id, validator).is_some() {
            return Err(format!(
                "duplicate validator_id {} in validator set",
                validator.validator_id
            ));
        }
    }
    Ok(validator_map)
}

fn build_participation_map<'a>(
    participation: &'a [ValidatorBlockParticipation],
    validator_map: &BTreeMap<u32, &ValidatorStakeInfo>,
) -> Result<BTreeMap<u32, &'a ValidatorBlockParticipation>, String> {
    let mut participation_map = BTreeMap::new();
    for entry in participation {
        if !validator_map.contains_key(&entry.validator_id) {
            return Err(format!(
                "participation entry references unknown validator {}",
                entry.validator_id
            ));
        }
        if u64::from(entry.liveness_penalty_bps) > BPS_DENOMINATOR {
            return Err(format!(
                "validator {} liveness penalty exceeds 10000 bps",
                entry.validator_id
            ));
        }
        if u64::from(entry.slash_penalty_bps) > BPS_DENOMINATOR {
            return Err(format!(
                "validator {} slash penalty exceeds 10000 bps",
                entry.validator_id
            ));
        }
        if participation_map.insert(entry.validator_id, entry).is_some() {
            return Err(format!(
                "duplicate participation entry for validator {}",
                entry.validator_id
            ));
        }
    }
    Ok(participation_map)
}

fn allocate_asset_bucket(
    amount: u64,
    weights: &[(u32, u64)],
) -> Result<BTreeMap<u32, u64>, String> {
    let mut allocations = BTreeMap::new();
    if amount == 0 || weights.is_empty() {
        return Ok(allocations);
    }

    let total_weight = weights.iter().try_fold(0u128, |acc, (_, weight)| {
        acc.checked_add(u128::from(*weight)).ok_or_else(|| "validator weight overflow".to_string())
    })?;
    if total_weight == 0 {
        return Err(
            "cannot allocate non-zero asset bucket with zero total validator weight".to_string()
        );
    }

    let mut distributed = 0u64;
    let mut remainders = Vec::with_capacity(weights.len());
    for (validator_id, weight) in weights {
        let product = u128::from(amount) * u128::from(*weight);
        let quotient = (product / total_weight) as u64;
        let remainder = product % total_weight;
        allocations.insert(*validator_id, quotient);
        distributed = distributed
            .checked_add(quotient)
            .ok_or_else(|| "distributed payout overflow".to_string())?;
        remainders.push((remainder, *validator_id));
    }

    let leftover = amount
        .checked_sub(distributed)
        .ok_or_else(|| "distributed payout exceeded asset bucket".to_string())?;
    remainders.sort_by(|(left_remainder, left_id), (right_remainder, right_id)| {
        right_remainder.cmp(left_remainder).then(left_id.cmp(right_id))
    });
    for index in 0..leftover as usize {
        let validator_id = remainders[index].1;
        *allocations.entry(validator_id).or_insert(0) += 1;
    }

    Ok(allocations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        payment_fixtures::{valid_usdc_hush_fee_fixture, valid_usdc_same_asset_fixture},
        payment_tx::{AssetId, TxKind, TX_KIND_PAYMENT},
        payment_validation,
    };

    fn sample_validators() -> Vec<ValidatorStakeInfo> {
        vec![
            ValidatorStakeInfo { validator_id: 1, payout_key: 101, effective_stake: 100 },
            ValidatorStakeInfo { validator_id: 2, payout_key: 202, effective_stake: 100 },
        ]
    }

    fn sample_participation_all_present() -> Vec<ValidatorBlockParticipation> {
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

    fn build_mode_a_bundle_record() -> (PaymentTxV1, PaymentBundleProof, AcceptedTxRecord) {
        let fixture = valid_usdc_same_asset_fixture();
        let bundle = payment_validation::prove_payment_bundle(&fixture.tx, &fixture.witness, None)
            .expect("Mode A bundle should prove");
        let record =
            accepted_payment_record(&fixture.tx, &bundle).expect("Mode A bundle should validate");
        (fixture.tx, bundle, record)
    }

    fn build_mode_b_bundle_record() -> (PaymentTxV1, PaymentBundleProof, AcceptedTxRecord) {
        let fixture = valid_usdc_hush_fee_fixture();
        let bundle = payment_validation::prove_payment_bundle(
            &fixture.tx,
            &fixture.witness,
            fixture.fee_sidecar_witness.as_ref(),
        )
        .expect("Mode B bundle should prove");
        let record =
            accepted_payment_record(&fixture.tx, &bundle).expect("Mode B bundle should validate");
        (fixture.tx, bundle, record)
    }

    #[test]
    fn test_block_fee_bucket_accumulation_by_asset() {
        let (mode_a_tx, mode_a_bundle, _) = build_mode_a_bundle_record();
        let (mode_b_tx, mode_b_bundle, _) = build_mode_b_bundle_record();
        let action = ProtocolActionTx::build(TxKind::ValidatorAction, 7, 9)
            .expect("validator action should build");

        let mut block = BlockAccountingBuilder::new(10, 99);
        block
            .record_payment_bundle(&mode_a_tx, &mode_a_bundle)
            .expect("Mode A payment should count");
        block
            .record_payment_bundle(&mode_b_tx, &mode_b_bundle)
            .expect("Mode B payment should count");
        block.record_protocol_action(&action).expect("validator action should count");
        let record = block.finalize();
        record.validate().expect("block accounting should reconcile");

        assert_eq!(record.fee_buckets.usdc, 50);
        assert_eq!(record.fee_buckets.hush, 59);
        assert_eq!(record.fee_buckets.usdt, 0);
        assert_eq!(record.tx_counts_by_asset.usdc, 1);
        assert_eq!(record.tx_counts_by_asset.hush, 2);
        assert_eq!(record.tx_counts_by_kind.get(&TX_KIND_PAYMENT), Some(&2));
        assert_eq!(record.tx_counts_by_kind.get(&TxKind::ValidatorAction.as_u32()), Some(&1));
    }

    #[test]
    fn test_rejected_tx_does_not_affect_fee_buckets() {
        let fixture = valid_usdc_same_asset_fixture();
        let bundle = payment_validation::prove_payment_bundle(&fixture.tx, &fixture.witness, None)
            .expect("Mode A bundle should prove");
        let mut bad_tx = fixture.tx.clone();
        bad_tx.tx_binding_hash[0] = bad_tx.tx_binding_hash[0].wrapping_add(1);

        let mut block = BlockAccountingBuilder::new(11, 1);
        assert!(block.record_payment_bundle(&bad_tx, &bundle).is_err());
        assert_eq!(block.fee_buckets(), AssetFeeBuckets::default());
    }

    #[test]
    fn test_per_block_totals_reconcile() {
        let (_, _, record) = build_mode_a_bundle_record();
        let mut block = BlockAccountingBuilder::new(12, 2);
        block.record_accepted_tx_record(&record).expect("accepted record should count");
        let record = block.finalize();
        record.validate().expect("block accounting should reconcile");
    }

    #[test]
    fn test_epoch_fee_pool_accumulation_by_asset() {
        let (_, _, mode_a_record) = build_mode_a_bundle_record();
        let (_, _, mode_b_record) = build_mode_b_bundle_record();

        let mut block_a = BlockAccountingBuilder::new(20, 1);
        block_a.record_accepted_tx_record(&mode_a_record).expect("accepted record should count");
        let block_a = block_a.finalize();

        let mut block_b = BlockAccountingBuilder::new(21, 1);
        block_b.record_accepted_tx_record(&mode_b_record).expect("accepted record should count");
        let block_b = block_b.finalize();

        let validators = sample_validators();
        let participation = sample_participation_all_present();
        let mut epoch = EpochAccumulator::new(3);
        epoch
            .apply_block(&block_a, &validators, &participation)
            .expect("first block should accrue");
        epoch
            .apply_block(&block_b, &validators, &participation)
            .expect("second block should accrue");

        assert_eq!(epoch.fee_pools(), AssetFeeBuckets { hush: 50, usdc: 50, usdt: 0 });
    }

    #[test]
    fn test_validator_participation_affects_entitlement() {
        let (_, _, mode_a_record) = build_mode_a_bundle_record();
        let (_, _, mode_b_record) = build_mode_b_bundle_record();

        let mut block_a = BlockAccountingBuilder::new(30, 1);
        block_a.record_accepted_tx_record(&mode_a_record).expect("accepted record should count");
        let block_a = block_a.finalize();

        let mut block_b = BlockAccountingBuilder::new(31, 1);
        block_b.record_accepted_tx_record(&mode_b_record).expect("accepted record should count");
        let block_b = block_b.finalize();

        let validators = sample_validators();
        let mut epoch = EpochAccumulator::new(4);
        epoch
            .apply_block(&block_a, &validators, &sample_participation_all_present())
            .expect("first block should accrue");
        epoch
            .apply_block(
                &block_b,
                &validators,
                &[
                    ValidatorBlockParticipation {
                        validator_id: 1,
                        signed_block: true,
                        liveness_penalty_bps: 0,
                        slash_penalty_bps: 0,
                    },
                    ValidatorBlockParticipation {
                        validator_id: 2,
                        signed_block: false,
                        liveness_penalty_bps: 0,
                        slash_penalty_bps: 0,
                    },
                ],
            )
            .expect("second block should accrue");
        let settlement = epoch.close().expect("epoch should close");

        let validator_one =
            settlement.validator_entitlements.get(&1).expect("validator 1 should accrue");
        let validator_two =
            settlement.validator_entitlements.get(&2).expect("validator 2 should accrue");

        assert_eq!(validator_one.entitlement.usdc, 25);
        assert_eq!(validator_two.entitlement.usdc, 25);
        assert_eq!(validator_one.entitlement.hush, 50);
        assert_eq!(validator_two.entitlement.hush, 0);
        assert_eq!(validator_one.present_blocks, 2);
        assert_eq!(validator_two.missed_blocks, 1);
    }

    #[test]
    fn test_mixed_basket_payout_reconciliation() {
        let (_, _, mode_a_record) = build_mode_a_bundle_record();
        let (_, _, mode_b_record) = build_mode_b_bundle_record();
        let action = accepted_protocol_action_record(
            &ProtocolActionTx::build(TxKind::IssuerAction, 44, 7)
                .expect("issuer action should build"),
        )
        .expect("issuer action should validate");

        let mut block = BlockAccountingBuilder::new(40, 1);
        block.record_accepted_tx_record(&mode_a_record).expect("accepted record should count");
        block.record_accepted_tx_record(&mode_b_record).expect("accepted record should count");
        block.record_accepted_tx_record(&action).expect("accepted record should count");
        let block = block.finalize();

        let validators = sample_validators();
        let mut epoch = EpochAccumulator::new(5);
        epoch
            .apply_block(&block, &validators, &sample_participation_all_present())
            .expect("block should accrue");
        let settlement = epoch.close().expect("epoch should close");
        let payout_totals = settlement.total_payouts().expect("payouts should sum");

        assert_eq!(settlement.fee_pools, payout_totals);
        assert_eq!(payout_totals.usdc, 50);
        assert_eq!(payout_totals.hush, 57);
        assert_eq!(payout_totals.usdt, 0);
        assert_eq!(settlement.payout_records.len(), 2);
    }

    #[test]
    fn test_non_payment_action_with_non_hush_fee_rejected() {
        let mut action = ProtocolActionTx::build(TxKind::ProtocolAdminAction, 55, 3)
            .expect("protocol admin action should build");
        action.fee_asset = AssetId::Usdc as u32;
        assert!(validate_protocol_action_tx(&action).is_err());
    }

    #[test]
    fn test_canonical_transaction_kind_mapping_enforced() {
        let action = ProtocolActionTx::build(TxKind::CredentialStateAction, 66, 4)
            .expect("credential action should build");
        validate_protocol_action_tx(&action).expect("credential action should accept HUSH");

        assert!(validate_tx_kind_fee_asset_policy(
            TxKind::CredentialStateAction.as_u32(),
            None,
            AssetId::Usdt as u32,
        )
        .is_err());
    }

    #[test]
    fn test_accounting_does_not_drift_from_validated_tx_inputs() {
        let (tx, bundle, record) = build_mode_b_bundle_record();
        let mut block = BlockAccountingBuilder::new(50, 1);
        block.record_payment_bundle(&tx, &bundle).expect("validated bundle should count");
        let block = block.finalize();
        assert_eq!(record.fee_asset, tx.descriptor.fee_asset);
        assert_eq!(record.fee_amount, tx.descriptor.fee_amount);
        assert_eq!(block.fee_buckets.hush, tx.descriptor.fee_amount);
    }

    #[test]
    fn test_same_tx_cannot_be_double_counted() {
        let (_, _, record) = build_mode_a_bundle_record();
        let mut block = BlockAccountingBuilder::new(60, 1);
        block.record_accepted_tx_record(&record).expect("accepted record should count");
        assert!(block.record_accepted_tx_record(&record).is_err());
    }

    #[test]
    fn test_malformed_accounting_state_transition_rejected() {
        let (_, _, record) = build_mode_a_bundle_record();
        let mut block = BlockAccountingBuilder::new(70, 1);
        block.record_accepted_tx_record(&record).expect("accepted record should count");
        let mut block = block.finalize();
        block.fee_buckets.usdc += 1;

        let validators = sample_validators();
        let participation = sample_participation_all_present();
        let mut epoch = EpochAccumulator::new(6);
        assert!(epoch.apply_block(&block, &validators, &participation).is_err());
    }

    #[test]
    fn test_liveness_and_slash_hooks_reduce_entitlement() {
        let (_, _, mode_b_record) = build_mode_b_bundle_record();

        let mut block = BlockAccountingBuilder::new(80, 1);
        block.record_accepted_tx_record(&mode_b_record).expect("accepted record should count");
        let block = block.finalize();

        let validators = sample_validators();
        let participation = vec![
            ValidatorBlockParticipation {
                validator_id: 1,
                signed_block: true,
                liveness_penalty_bps: 2_500,
                slash_penalty_bps: 0,
            },
            ValidatorBlockParticipation {
                validator_id: 2,
                signed_block: true,
                liveness_penalty_bps: 0,
                slash_penalty_bps: 0,
            },
        ];

        let mut epoch = EpochAccumulator::new(7);
        epoch.apply_block(&block, &validators, &participation).expect("block should accrue");
        let settlement = epoch.close().expect("epoch should close");
        let validator_one =
            settlement.validator_entitlements.get(&1).expect("validator 1 should accrue");
        let validator_two =
            settlement.validator_entitlements.get(&2).expect("validator 2 should accrue");

        assert!(validator_two.entitlement.hush > validator_one.entitlement.hush);
        assert_eq!(validator_one.max_liveness_penalty_bps, 2_500);
    }
}
