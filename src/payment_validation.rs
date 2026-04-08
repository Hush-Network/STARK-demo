use crate::{
    circuit,
    fee_sidecar,
    payment_tx::{validate_payment_tx, PaymentRoute, PaymentTxV1},
    types::{HushFeeWitness, PaymentWitness},
};

pub struct PaymentBundleProof {
    pub payment: circuit::ProofResult,
    pub fee_sidecar: Option<fee_sidecar::ProofResult>,
}

pub fn prove_payment_bundle(
    tx: &PaymentTxV1,
    payment_witness: &PaymentWitness,
    fee_sidecar_witness: Option<&HushFeeWitness>,
) -> Result<PaymentBundleProof, String> {
    let route = validate_payment_tx(tx)?;
    let payment = circuit::prove_payment(payment_witness)?;
    let fee_sidecar = match (route, fee_sidecar_witness) {
        (PaymentRoute::SameAsset, None) => None,
        (PaymentRoute::SameAsset, Some(_)) => {
            return Err("sidecar witness is disallowed for same-asset Mode A".to_string());
        }
        (PaymentRoute::HushSidecar, None) => {
            return Err("missing HUSH sidecar witness for Mode B".to_string());
        }
        (PaymentRoute::HushSidecar, Some(witness)) => Some(fee_sidecar::prove_hush_fee(witness)?),
    };

    let bundle = PaymentBundleProof { payment, fee_sidecar };
    validate_payment_bundle(tx, &bundle)?;
    Ok(bundle)
}

pub fn validate_payment_bundle(tx: &PaymentTxV1, bundle: &PaymentBundleProof) -> Result<(), String> {
    let route = validate_payment_tx(tx)?;

    circuit::verify_payment(&bundle.payment)?;
    if bundle.payment.public_data.tx_binding_hash != tx.tx_binding_hash {
        return Err(format!(
            "payment proof tx_binding_hash mismatch: proof {}, tx {}",
            bundle.payment.public_data.tx_binding_hash, tx.tx_binding_hash
        ));
    }
    if bundle.payment.public_data.sender_binding_tag != tx.attachment.sender_binding_tag {
        return Err(format!(
            "payment proof sender_binding_tag mismatch: proof {}, tx {}",
            bundle.payment.public_data.sender_binding_tag, tx.attachment.sender_binding_tag
        ));
    }

    match (route, &bundle.fee_sidecar) {
        (PaymentRoute::SameAsset, None) => Ok(()),
        (PaymentRoute::SameAsset, Some(_)) => {
            Err("sidecar proof is disallowed for same-asset Mode A".to_string())
        }
        (PaymentRoute::HushSidecar, None) => {
            Err("missing HUSH sidecar proof for Mode B".to_string())
        }
        (PaymentRoute::HushSidecar, Some(sidecar)) => {
            fee_sidecar::verify_hush_fee(sidecar)?;
            if sidecar.public_data.tx_binding_hash != tx.tx_binding_hash {
                return Err(format!(
                    "sidecar tx_binding_hash mismatch: proof {}, tx {}",
                    sidecar.public_data.tx_binding_hash, tx.tx_binding_hash
                ));
            }
            if sidecar.public_data.sender_binding_tag != tx.attachment.sender_binding_tag {
                return Err(format!(
                    "sidecar sender_binding_tag mismatch: proof {}, tx {}",
                    sidecar.public_data.sender_binding_tag, tx.attachment.sender_binding_tag
                ));
            }
            if sidecar.public_data.fee_amount != tx.descriptor.fee_amount {
                return Err(format!(
                    "sidecar fee amount mismatch: proof {}, tx {}",
                    sidecar.public_data.fee_amount, tx.descriptor.fee_amount
                ));
            }
            if bundle.payment.public_data.tx_binding_hash != sidecar.public_data.tx_binding_hash {
                return Err("payment proof and sidecar proof tx_binding_hash do not match".to_string());
            }
            if bundle.payment.public_data.sender_binding_tag
                != sidecar.public_data.sender_binding_tag
            {
                return Err(
                    "payment proof and sidecar proof sender_binding_tag do not match".to_string(),
                );
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        payment_fixtures::{
            malformed_sidecar_hush_fee_fixture, missing_sidecar_hush_fee_fixture,
            valid_usdc_hush_fee_fixture, valid_usdc_same_asset_fixture, valid_usdt_hush_fee_fixture,
            valid_usdt_same_asset_fixture,
        },
        payment_tx::{AssetId, FeeAuxProofDescriptor},
    };

    #[test]
    fn test_all_four_valid_combinations_accepted() {
        let usdc_same = valid_usdc_same_asset_fixture();
        prove_payment_bundle(&usdc_same.tx, &usdc_same.witness, None)
            .expect("USDC payment with USDC fee should validate");

        let usdt_same = valid_usdt_same_asset_fixture();
        prove_payment_bundle(&usdt_same.tx, &usdt_same.witness, None)
            .expect("USDT payment with USDT fee should validate");

        let usdc_hush = valid_usdc_hush_fee_fixture();
        prove_payment_bundle(
            &usdc_hush.tx,
            &usdc_hush.witness,
            usdc_hush.fee_sidecar_witness.as_ref(),
        )
        .expect("USDC payment with HUSH fee should validate");

        let usdt_hush = valid_usdt_hush_fee_fixture();
        prove_payment_bundle(
            &usdt_hush.tx,
            &usdt_hush.witness,
            usdt_hush.fee_sidecar_witness.as_ref(),
        )
        .expect("USDT payment with HUSH fee should validate");
    }

    #[test]
    fn test_missing_sidecar_rejected_when_required() {
        let fixture = missing_sidecar_hush_fee_fixture();
        let err = match prove_payment_bundle(&fixture.tx, &fixture.witness, None) {
            Ok(_) => panic!("Mode B bundle should reject missing sidecar"),
            Err(err) => err,
        };
        assert!(err.contains("missing HUSH sidecar"));
    }

    #[test]
    fn test_sidecar_rejected_when_disallowed() {
        let same_asset = valid_usdc_same_asset_fixture();
        let sidecar = valid_usdc_hush_fee_fixture();
        let err = match prove_payment_bundle(
            &same_asset.tx,
            &same_asset.witness,
            sidecar.fee_sidecar_witness.as_ref(),
        ) {
            Ok(_) => panic!("Mode A bundle should reject sidecar proof"),
            Err(err) => err,
        };
        assert!(err.contains("sidecar"));
    }

    #[test]
    fn test_wrong_sidecar_attached_to_payment_proof_rejected() {
        let usdc_hush = valid_usdc_hush_fee_fixture();
        let usdt_hush = valid_usdt_hush_fee_fixture();

        let payment = circuit::prove_payment(&usdc_hush.witness).expect("payment proof should succeed");
        let sidecar = fee_sidecar::prove_hush_fee(
            usdt_hush
                .fee_sidecar_witness
                .as_ref()
                .expect("Mode B fixture should include sidecar"),
        )
        .expect("sidecar proof should succeed");

        let err = match validate_payment_bundle(
            &usdc_hush.tx,
            &PaymentBundleProof { payment, fee_sidecar: Some(sidecar) },
        ) {
            Ok(_) => panic!("wrong sidecar pairing should be rejected"),
            Err(err) => err,
        };
        assert!(err.contains("mismatch") || err.contains("do not match"));
    }

    #[test]
    fn test_cross_stablecoin_mismatch_rejected() {
        let mut fixture = valid_usdc_same_asset_fixture();
        fixture.tx.descriptor.fee_asset = AssetId::Usdt as u32;
        let err = match prove_payment_bundle(&fixture.tx, &fixture.witness, None) {
            Ok(_) => panic!("cross-stable mismatch should be rejected"),
            Err(err) => err,
        };
        assert!(err.contains("cross-stablecoin"));
    }

    #[test]
    fn test_malformed_fee_descriptor_rejected() {
        let mut fixture = valid_usdc_same_asset_fixture();
        fixture.tx.descriptor.fee_class = 99;
        let err = match prove_payment_bundle(&fixture.tx, &fixture.witness, None) {
            Ok(_) => panic!("malformed descriptor should be rejected"),
            Err(err) => err,
        };
        assert!(err.contains("fee_class"));
    }

    #[test]
    fn test_malformed_sidecar_rejected() {
        let fixture = malformed_sidecar_hush_fee_fixture();
        let err = match prove_payment_bundle(
            &fixture.tx,
            &fixture.witness,
            fixture.fee_sidecar_witness.as_ref(),
        ) {
            Ok(_) => panic!("malformed sidecar should be rejected"),
            Err(err) => err,
        };
        assert!(err.contains("HUSH sidecar"));
    }

    #[test]
    fn test_proof_route_mismatch_rejected() {
        let mut fixture = valid_usdc_hush_fee_fixture();
        fixture.tx.attachment.fee_aux = Some(FeeAuxProofDescriptor { route: 99 });
        let err = match prove_payment_bundle(
            &fixture.tx,
            &fixture.witness,
            fixture.fee_sidecar_witness.as_ref(),
        ) {
            Ok(_) => panic!("proof-route mismatch should be rejected"),
            Err(err) => err,
        };
        assert!(err.contains("route"));
    }

    #[test]
    fn test_wrong_tx_binding_hash_rejected() {
        let fixture = valid_usdc_hush_fee_fixture();
        let mut tx = fixture.tx.clone();
        tx.tx_binding_hash = tx.tx_binding_hash.wrapping_add(1);
        let err = match prove_payment_bundle(&tx, &fixture.witness, fixture.fee_sidecar_witness.as_ref()) {
            Ok(_) => panic!("wrong tx_binding_hash should be rejected"),
            Err(err) => err,
        };
        assert!(err.contains("tx_binding_hash"));
    }

    #[test]
    fn test_wrong_sender_binding_tag_rejected() {
        let fixture = valid_usdc_hush_fee_fixture();
        let mut tx = fixture.tx.clone();
        tx.attachment.sender_binding_tag = tx.attachment.sender_binding_tag.wrapping_add(1);
        let err = match prove_payment_bundle(&tx, &fixture.witness, fixture.fee_sidecar_witness.as_ref()) {
            Ok(_) => panic!("wrong sender_binding_tag should be rejected"),
            Err(err) => err,
        };
        assert!(err.contains("sender_binding_tag"));
    }
}
