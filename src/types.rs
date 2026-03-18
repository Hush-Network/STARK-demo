pub const MERKLE_DEPTH: usize = 20;

#[derive(Clone, Debug)]
pub struct PaymentWitness {
    pub epoch: u32,
    pub note_root: u32,
    pub cred_root: u32,
    pub sk: u32,
    pub in_asset: u32,
    pub in_amt_0: u32,
    pub in_rand_0: u32,
    pub in_amt_1: u32,
    pub in_rand_1: u32,
    pub out_amt_0: u32,
    pub out_owner_0: u32,
    pub out_rand_0: u32,
    pub out_amt_1: u32,
    pub out_rand_1: u32,
    pub cred_issuer: u32,
    pub cred_expiry: u32,
    pub cred_secret: u32,

    pub note_path_0: [(u32, u32); MERKLE_DEPTH],
    pub note_path_1: [(u32, u32); MERKLE_DEPTH],
    pub cred_path: [(u32, u32); MERKLE_DEPTH],
}
