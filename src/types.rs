pub const MERKLE_DEPTH: usize = 20;

/// Amount encoding: four 15-bit limbs in radix R = 2^15 = 32768.
/// Max representable value: R^4 - 1 = 2^60 - 1 = 1,152,921,504,606,846,975.
/// At 4-decimal denomination (1 unit = $0.0001), max = ~$115.29 trillion per note.
pub const LIMB_BITS: usize = 15;
pub const NUM_LIMBS: usize = 4;
pub const RADIX: u64 = 1 << LIMB_BITS; // 32768
pub const RADIX_U32: u32 = RADIX as u32;
pub const LIMB_MAX: u32 = RADIX_U32 - 1; // 32767

/// Carry bias for limb-by-limb conservation. Carries are in [-2, 1].
/// Biased carry = carry + CARRY_BIAS is in [0, 3], representable in 2 bits.
pub const CARRY_BIAS: u32 = 2;
pub const CARRY_BITS: usize = 2;
pub const NUM_CARRIES: usize = NUM_LIMBS - 1; // 3

/// Decompose a u64 amount into four 15-bit limbs (little-endian radix-R).
pub fn amount_to_limbs(amount: u64) -> [u32; NUM_LIMBS] {
    [
        (amount % RADIX) as u32,
        ((amount / RADIX) % RADIX) as u32,
        ((amount / (RADIX * RADIX)) % RADIX) as u32,
        ((amount / (RADIX * RADIX * RADIX)) % RADIX) as u32,
    ]
}

/// Reconstruct a u64 amount from four 15-bit limbs.
pub fn limbs_to_amount(limbs: [u32; NUM_LIMBS]) -> u64 {
    u64::from(limbs[0])
        + u64::from(limbs[1]) * RADIX
        + u64::from(limbs[2]) * RADIX * RADIX
        + u64::from(limbs[3]) * RADIX * RADIX * RADIX
}

#[derive(Clone, Debug)]
pub struct PaymentWitness {
    pub epoch: u32,
    pub note_root: [u32; 4],
    pub cred_root: [u32; 4],
    pub sk: u32,
    pub in_asset: u32,
    pub in_amt_0: u64,
    pub in_rand_0: u32,
    pub in_amt_1: u64,
    pub in_rand_1: u32,
    pub out_amt_0: u64,
    pub out_owner_0: [u32; 4],
    pub out_rand_0: u32,
    pub out_amt_1: u64,
    pub out_rand_1: u32,
    pub payment_fee_amount: u64,
    pub binding_fee_asset: u32,
    pub fee_amount: u64,
    pub fee_class: u32,
    pub replay_domain: u32,
    pub tx_binding_hash: [u32; 4],
    pub sender_binding_tag: [u32; 4],
    pub cred_issuer: u32,
    pub cred_expiry: u32,
    pub cred_secret: u32,

    pub note_path_0: [([u32; 4], u32); MERKLE_DEPTH],
    pub note_path_1: [([u32; 4], u32); MERKLE_DEPTH],
    pub cred_path: [([u32; 4], u32); MERKLE_DEPTH],
}

#[derive(Clone, Debug)]
pub struct HushFeeWitness {
    pub note_root: [u32; 4],
    pub sk: u32,
    pub in_amt_0: u64,
    pub in_rand_0: u32,
    pub in_amt_1: u64,
    pub in_rand_1: u32,
    pub change_amt: u64,
    pub change_rand: u32,
    pub fee_amount: u64,
    pub tx_binding_hash: [u32; 4],
    pub sender_binding_tag: [u32; 4],
    pub note_path_0: [([u32; 4], u32); MERKLE_DEPTH],
    pub note_path_1: [([u32; 4], u32); MERKLE_DEPTH],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_limb_roundtrip() {
        let cases: &[u64] = &[
            0,
            1,
            50,
            32767,
            32768,
            1_000_000,
            1_250_000_000,
            40_000_000_000_000,
            (1u64 << 60) - 1,
        ];
        for &amount in cases {
            let limbs = amount_to_limbs(amount);
            assert_eq!(limbs_to_amount(limbs), amount, "roundtrip failed for {amount}");
            for &limb in &limbs {
                assert!(limb <= LIMB_MAX, "limb {limb} exceeds LIMB_MAX for amount {amount}");
            }
        }
    }

    #[test]
    fn test_radix_squared_fits_m31() {
        // R^2 = 32768^2 = 1,073,741,824 < p = 2,147,483,647. This is critical for
        // M31 field safety: RADIX can be used as a field constant without wrapping.
        let r2 = RADIX * RADIX;
        let p = (1u64 << 31) - 1;
        assert!(r2 < p, "R^2 = {r2} must be < p = {p}");
    }
}
