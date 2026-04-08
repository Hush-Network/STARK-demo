use stwo::core::fields::m31::M31;

use crate::{
    payment_tx::{
        AssetId, HushFeeMerkleContext, NoteInput, PaymentMerkleContext, PaymentRoute, PaymentTxV1,
        RecipientIntent, SenderChangeIntent,
    },
    poseidon2,
    types::{HushFeeWitness, PaymentWitness, MERKLE_DEPTH},
};

#[derive(Clone, Debug)]
pub struct PaymentFixtureContext {
    pub tx: PaymentTxV1,
    pub witness: PaymentWitness,
    pub fee_sidecar_witness: Option<HushFeeWitness>,
    pub sender_binding_tag: u32,
}

type NoteMerkleContext = (u32, [(u32, u32); MERKLE_DEPTH], [(u32, u32); MERKLE_DEPTH]);

fn build_note_context(
    sk: u32,
    asset: AssetId,
    inputs: [NoteInput; 2],
) -> NoteMerkleContext {
    let owner = poseidon2::derive_owner(M31::from(sk));
    let asset = M31::from(asset.as_u32());

    let in_cm_0 = poseidon2::note_commitment_u64(
        asset,
        inputs[0].amount,
        owner,
        M31::from(inputs[0].randomness),
    );
    let in_cm_1 = poseidon2::note_commitment_u64(
        asset,
        inputs[1].amount,
        owner,
        M31::from(inputs[1].randomness),
    );
    let mut note_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
    note_tree.set_leaf(0, in_cm_0);
    note_tree.set_leaf(1, in_cm_1);

    let note_path_0_vec = note_tree.path(0);
    let note_path_1_vec = note_tree.path(1);
    let mut note_path_0 = [(0u32, 0u32); MERKLE_DEPTH];
    let mut note_path_1 = [(0u32, 0u32); MERKLE_DEPTH];
    for i in 0..MERKLE_DEPTH {
        note_path_0[i] = (note_path_0_vec[i].0 .0, note_path_0_vec[i].1);
        note_path_1[i] = (note_path_1_vec[i].0 .0, note_path_1_vec[i].1);
    }

    (note_tree.root().0, note_path_0, note_path_1)
}

pub fn build_payment_merkle_context(
    sk: u32,
    payment_inputs: [NoteInput; 2],
    payment_asset: AssetId,
    cred_issuer: u32,
    cred_expiry: u32,
    cred_secret: u32,
    epoch: u32,
) -> PaymentMerkleContext {
    let (note_root, note_path_0, note_path_1) = build_note_context(sk, payment_asset, payment_inputs);
    let owner = poseidon2::derive_owner(M31::from(sk));
    let cred_cm = poseidon2::credential_commitment(
        M31::from(cred_issuer),
        owner,
        M31::from(cred_expiry),
        M31::from(cred_secret),
    );
    let mut cred_tree = poseidon2::SparseMerkleTree::new(MERKLE_DEPTH);
    cred_tree.set_leaf(0, cred_cm);
    let cred_path_vec = cred_tree.path(0);
    let mut cred_path = [(0u32, 0u32); MERKLE_DEPTH];
    for i in 0..MERKLE_DEPTH {
        cred_path[i] = (cred_path_vec[i].0 .0, cred_path_vec[i].1);
    }

    PaymentMerkleContext {
        epoch,
        note_root,
        cred_root: cred_tree.root().0,
        cred_issuer,
        cred_expiry,
        cred_secret,
        note_path_0,
        note_path_1,
        cred_path,
    }
}

pub fn build_hush_fee_merkle_context(sk: u32, hush_inputs: [NoteInput; 2]) -> HushFeeMerkleContext {
    let (note_root, note_path_0, note_path_1) = build_note_context(sk, AssetId::Hush, hush_inputs);
    HushFeeMerkleContext { note_root, note_path_0, note_path_1 }
}

fn build_context(
    route: PaymentRoute,
    sk: u32,
    payment_asset: AssetId,
    payment_inputs: [NoteInput; 2],
    recipient_amount: u64,
    recipient_owner: u32,
    recipient_randomness: u32,
    sender_change_randomness: u32,
    hush_inputs: Option<[NoteInput; 2]>,
    hush_change_randomness: Option<u32>,
    cred_issuer: u32,
    cred_expiry: u32,
    cred_secret: u32,
    epoch: u32,
) -> PaymentFixtureContext {
    let tx = match route {
        PaymentRoute::SameAsset => PaymentTxV1::build_same_asset(
            payment_asset,
            payment_inputs.clone(),
            RecipientIntent { amount: recipient_amount, owner: recipient_owner, randomness: recipient_randomness },
            sender_change_randomness,
            sk,
        ),
        PaymentRoute::HushSidecar => PaymentTxV1::build_with_hush_fee(
            payment_asset,
            payment_inputs.clone(),
            RecipientIntent { amount: recipient_amount, owner: recipient_owner, randomness: recipient_randomness },
            sender_change_randomness,
            sk,
        ),
    }
    .expect("fixture tx should build");

    let payment_context = build_payment_merkle_context(
        sk,
        payment_inputs,
        payment_asset,
        cred_issuer,
        cred_expiry,
        cred_secret,
        epoch,
    );
    let witness = tx.build_witness(sk, &payment_context).expect("fixture witness should build");

    let fee_sidecar_witness = match (route, hush_inputs, hush_change_randomness) {
        (PaymentRoute::HushSidecar, Some(hush_inputs), Some(hush_change_randomness)) => {
            let hush_context = build_hush_fee_merkle_context(sk, hush_inputs.clone());
            let change_total =
                hush_inputs[0].amount + hush_inputs[1].amount - tx.descriptor.fee_amount;
            Some(
                tx.build_hush_fee_witness(
                    sk,
                    hush_inputs,
                    SenderChangeIntent {
                        amount: change_total,
                        randomness: hush_change_randomness,
                    },
                    &hush_context,
                )
                .expect("fixture HUSH sidecar should build"),
            )
        }
        (PaymentRoute::SameAsset, None, None) => None,
        _ => panic!("fixture route and HUSH sidecar parameters must match"),
    };

    PaymentFixtureContext {
        tx: tx.clone(),
        witness,
        fee_sidecar_witness,
        sender_binding_tag: tx.attachment.sender_binding_tag,
    }
}

pub fn valid_usdc_same_asset_fixture() -> PaymentFixtureContext {
    build_context(
        PaymentRoute::SameAsset,
        12_345,
        AssetId::Usdc,
        [
            NoteInput { amount: 7_000, randomness: 111 },
            NoteInput { amount: 3_000, randomness: 222 },
        ],
        8_000,
        99_999,
        333,
        444,
        None,
        None,
        1,
        2_000,
        777,
        1_000,
    )
}

pub fn valid_usdt_same_asset_fixture() -> PaymentFixtureContext {
    build_context(
        PaymentRoute::SameAsset,
        22_222,
        AssetId::Usdt,
        [
            NoteInput { amount: 9_500, randomness: 555 },
            NoteInput { amount: 1_500, randomness: 666 },
        ],
        10_000,
        88_888,
        777,
        888,
        None,
        None,
        1,
        2_000,
        999,
        1_000,
    )
}

pub fn valid_usdc_hush_fee_fixture() -> PaymentFixtureContext {
    build_context(
        PaymentRoute::HushSidecar,
        31_313,
        AssetId::Usdc,
        [
            NoteInput { amount: 7_000, randomness: 111 },
            NoteInput { amount: 3_000, randomness: 222 },
        ],
        8_000,
        77_777,
        333,
        444,
        Some([
            NoteInput { amount: 40, randomness: 515 },
            NoteInput { amount: 20, randomness: 616 },
        ]),
        Some(717),
        1,
        2_000,
        818,
        1_000,
    )
}

pub fn valid_usdt_hush_fee_fixture() -> PaymentFixtureContext {
    build_context(
        PaymentRoute::HushSidecar,
        41_414,
        AssetId::Usdt,
        [
            NoteInput { amount: 9_500, randomness: 555 },
            NoteInput { amount: 1_500, randomness: 666 },
        ],
        10_000,
        66_666,
        777,
        888,
        Some([
            NoteInput { amount: 45, randomness: 919 },
            NoteInput { amount: 15, randomness: 1_020 },
        ]),
        Some(1_121),
        1,
        2_000,
        1_222,
        1_000,
    )
}

pub fn missing_sidecar_hush_fee_fixture() -> PaymentFixtureContext {
    let mut fixture = valid_usdc_hush_fee_fixture();
    fixture.fee_sidecar_witness = None;
    fixture
}

pub fn malformed_sidecar_hush_fee_fixture() -> PaymentFixtureContext {
    let mut fixture = valid_usdc_hush_fee_fixture();
    let bad_root = fixture
        .fee_sidecar_witness
        .as_ref()
        .expect("valid Mode B fixture should include sidecar")
        .note_root
        .wrapping_add(1);
    fixture
        .fee_sidecar_witness
        .as_mut()
        .expect("valid Mode B fixture should include sidecar")
        .note_root = bad_root;
    fixture
}

pub fn wrong_sender_binding_tag_hush_fee_fixture() -> PaymentFixtureContext {
    let mut fixture = valid_usdc_hush_fee_fixture();
    fixture
        .fee_sidecar_witness
        .as_mut()
        .expect("valid Mode B fixture should include sidecar")
        .sender_binding_tag = fixture.sender_binding_tag.wrapping_add(1);
    fixture
}

pub fn wrong_tx_binding_hash_hush_fee_fixture() -> PaymentFixtureContext {
    let mut fixture = valid_usdc_hush_fee_fixture();
    fixture
        .fee_sidecar_witness
        .as_mut()
        .expect("valid Mode B fixture should include sidecar")
        .tx_binding_hash = fixture.tx.tx_binding_hash.wrapping_add(1);
    fixture
}

pub fn insufficient_hush_fee_coverage_fixture() -> PaymentFixtureContext {
    let mut fixture = valid_usdc_hush_fee_fixture();
    let sidecar = fixture
        .fee_sidecar_witness
        .as_mut()
        .expect("valid Mode B fixture should include sidecar");
    sidecar.in_amt_0 = 2;
    sidecar.in_amt_1 = 2;
    sidecar.change_amt = 0;
    fixture
}

pub fn invalid_hush_change_fixture() -> PaymentFixtureContext {
    let mut fixture = valid_usdc_hush_fee_fixture();
    fixture
        .fee_sidecar_witness
        .as_mut()
        .expect("valid Mode B fixture should include sidecar")
        .change_amt += 1;
    fixture
}
