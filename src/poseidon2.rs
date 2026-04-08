//! Poseidon2 (M31, width-16). Constants from Plonky3 Grain LFSR.

use stwo::core::fields::m31::M31;

pub const WIDTH: usize = 16;
pub const RATE: usize = 8;
pub const NUM_FULL_ROUNDS_FIRST: usize = 4;
pub const NUM_PARTIAL_ROUNDS: usize = 14;
pub const NUM_FULL_ROUNDS_LAST: usize = 4;
pub const TOTAL_ROUNDS: usize = NUM_FULL_ROUNDS_FIRST + NUM_PARTIAL_ROUNDS + NUM_FULL_ROUNDS_LAST;

pub const DOMAIN_OWNER: u32 = 1;
pub const DOMAIN_NULLIFIER: u32 = 2;
pub const DOMAIN_NOTE_CM: u32 = 3;
pub const DOMAIN_CRED_CM: u32 = 4;
pub const DOMAIN_MERKLE: u32 = 5;
pub const DOMAIN_CRED_NULL: u32 = 6;
pub const DOMAIN_ISSUER_ID: u32 = 7;
pub const DOMAIN_TX_BINDING: u32 = 8;
pub const DOMAIN_SENDER_BINDING: u32 = 9;

pub const EXTERNAL_CONSTANTS: [[u32; 16]; 8] = [
    [
        0x768bab52, 0x70e0ab7d, 0x3d266c8a, 0x6da42045, 0x600fef22, 0x41dace6b, 0x64f9bdd4,
        0x5d42d4fe, 0x76b1516d, 0x6fc9a717, 0x70ac4fb6, 0x00194ef6, 0x22b644e2, 0x1f7916d5,
        0x47581be2, 0x2710a123,
    ],
    //
    [
        0x6284e867, 0x018d3afe, 0x5df99ef3, 0x4c1e467b, 0x566f6abc, 0x2994e427, 0x538a6d42,
        0x5d7bf2cf, 0x7fda2dab, 0x0fd854c4, 0x46922fca, 0x3d7763a1, 0x19fd05ca, 0x0a4bbb43,
        0x15075851, 0x3d903d76,
    ],
    //
    [
        0x2d290ff7, 0x40809fa0, 0x59dac6ec, 0x127927a2, 0x6bbf0ea0, 0x0294140f, 0x24742976,
        0x6e84c081, 0x22484f4a, 0x354cae59, 0x0453ffe1, 0x3f47a3cc, 0x0088204e, 0x6066e109,
        0x3b7c4b80, 0x6b55665d,
    ],
    //
    [
        0x3bc4b897, 0x735bf378, 0x508daf42, 0x1884fc2b, 0x7214f24c, 0x7498be0a, 0x1a60e640,
        0x3303f928, 0x29b46376, 0x5c96bb68, 0x65d097a5, 0x1d358e9f, 0x4a9a9017, 0x4724cf76,
        0x347af70f, 0x1e77e59a,
    ],
    //
    [
        0x57090613, 0x1fa42108, 0x17bbef50, 0x1ff7e11c, 0x047b24ca, 0x4e140275, 0x4fa086f5,
        0x079b309c, 0x1159bd47, 0x6d37e4e5, 0x075d8dce, 0x12121ca0, 0x7f6a7c40, 0x68e182ba,
        0x5493201b, 0x0444a80e,
    ],
    //
    [
        0x0064f4c6, 0x6467abe6, 0x66975762, 0x2af68f9b, 0x345b33be, 0x1b70d47f, 0x053db717,
        0x381189cb, 0x43b915f8, 0x20df3694, 0x0f459d26, 0x77a0e97b, 0x2f73e739, 0x1876c2f9,
        0x65a0e29a, 0x4cabefbe,
    ],
    //
    [
        0x5abd1268, 0x4d34a760, 0x12771799, 0x69a0c9ac, 0x39091e55, 0x7f611cd0, 0x3af055da,
        0x7ac0bbdf, 0x6e0f3a24, 0x41e3b6f7, 0x49b3756d, 0x568bc538, 0x20c079d8, 0x1701c72c,
        0x7670dc6c, 0x5a439035,
    ],
    //
    [
        0x7c93e00e, 0x561fbb4d, 0x1178907b, 0x02737406, 0x32fb24f1, 0x6323b60a, 0x6ab12418,
        0x42c99cea, 0x155a0b97, 0x53d1c6aa, 0x2bd20347, 0x279b3d73, 0x4f5f3c70, 0x0245af6c,
        0x238359d3, 0x49966a59,
    ],
];

pub const INTERNAL_CONSTANTS: [u32; 14] = [
    0x7f7ec4bf, 0x0421926f, 0x5198e669, 0x34db3148, 0x4368bafd, 0x66685c7f, 0x78d3249a, 0x60187881,
    0x76dad67a, 0x0690b437, 0x1ea95311, 0x40e5369a, 0x38f103fc, 0x1d226a21,
];

pub const INTERNAL_DIAG_M1: [i64; 16] =
    [-2, 1, 2, 4, 8, 16, 32, 64, 128, 256, 1024, 4096, 8192, 16384, 32768, 65536];

pub const M4: [[u32; 4]; 4] = [[2, 3, 1, 1], [1, 2, 3, 1], [1, 1, 2, 3], [3, 1, 1, 2]];

#[inline]
fn sbox(x: M31) -> M31 {
    let x2 = x * x;
    let x4 = x2 * x2;
    x4 * x
}

#[inline]
fn apply_m4(chunk: &mut [M31; 4]) {
    let input = *chunk;
    for i in 0..4 {
        chunk[i] = M31::from(0u32);
        for j in 0..4 {
            chunk[i] += M31::from(M4[i][j]) * input[j];
        }
    }
}

fn external_linear_layer(state: &mut [M31; WIDTH]) {
    let mut chunks: [[M31; 4]; 4] = [[M31::from(0u32); 4]; 4];
    for c in 0..4 {
        let mut chunk = [state[4 * c], state[4 * c + 1], state[4 * c + 2], state[4 * c + 3]];
        apply_m4(&mut chunk);
        chunks[c] = chunk;
    }

    let mut col_sums = [M31::from(0u32); 4];
    for j in 0..4 {
        for c in 0..4 {
            col_sums[j] += chunks[c][j];
        }
    }

    for c in 0..4 {
        for j in 0..4 {
            state[4 * c + j] = chunks[c][j] + col_sums[j];
        }
    }
}

fn internal_linear_layer(state: &mut [M31; WIDTH]) {
    let p = (1u64 << 31) - 1;
    let mut sum = M31::from(0u32);
    for i in 0..WIDTH {
        sum += state[i];
    }
    for i in 0..WIDTH {
        let v = INTERNAL_DIAG_M1[i];
        let vi_times_si = if v < 0 {
            let abs_v = (-v) as u64;
            let product = (state[i].0 as u64 * abs_v) % p;
            M31::from(((p - product) % p) as u32)
        } else {
            let product = (state[i].0 as u64 * v as u64) % p;
            M31::from(product as u32)
        };
        state[i] = sum + vi_times_si;
    }
}

pub fn permute_state(state: &mut [M31; WIDTH]) {
    external_linear_layer(state);

    for r in 0..NUM_FULL_ROUNDS_FIRST {
        for i in 0..WIDTH {
            state[i] += M31::from(EXTERNAL_CONSTANTS[r][i]);
        }
        for i in 0..WIDTH {
            state[i] = sbox(state[i]);
        }
        external_linear_layer(state);
    }

    for r in 0..NUM_PARTIAL_ROUNDS {
        state[0] += M31::from(INTERNAL_CONSTANTS[r]);
        state[0] = sbox(state[0]);
        internal_linear_layer(state);
    }

    for r in 0..NUM_FULL_ROUNDS_LAST {
        let ext_idx = NUM_FULL_ROUNDS_FIRST + r;
        for i in 0..WIDTH {
            state[i] += M31::from(EXTERNAL_CONSTANTS[ext_idx][i]);
        }
        for i in 0..WIDTH {
            state[i] = sbox(state[i]);
        }
        external_linear_layer(state);
    }
}

fn hash2_with_domain(a: M31, b: M31, domain: u32) -> M31 {
    let mut state = [M31::from(0u32); WIDTH];
    state[0] = a;
    state[1] = b;
    state[RATE] = M31::from(domain);
    permute_state(&mut state);
    state[0]
}

fn hash_many_4_with_domain(a: M31, b: M31, c: M31, d: M31, domain: u32) -> M31 {
    let mut state = [M31::from(0u32); WIDTH];
    state[0] = a;
    state[1] = b;
    state[2] = c;
    state[3] = d;
    state[RATE] = M31::from(domain);
    permute_state(&mut state);
    state[0]
}

pub fn domain_hash2(a: M31, b: M31, domain: u32) -> M31 {
    hash2_with_domain(a, b, domain)
}

pub fn domain_hash4(a: M31, b: M31, c: M31, d: M31, domain: u32) -> M31 {
    hash_many_4_with_domain(a, b, c, d, domain)
}

fn hash_many_7_with_domain(
    a: M31, b: M31, c: M31, d: M31, e: M31, f: M31, g: M31, domain: u32,
) -> M31 {
    let mut state = [M31::from(0u32); WIDTH];
    state[0] = a;
    state[1] = b;
    state[2] = c;
    state[3] = d;
    state[4] = e;
    state[5] = f;
    state[6] = g;
    state[RATE] = M31::from(domain);
    permute_state(&mut state);
    state[0]
}

pub fn domain_hash7(
    a: M31, b: M31, c: M31, d: M31, e: M31, f: M31, g: M31, domain: u32,
) -> M31 {
    hash_many_7_with_domain(a, b, c, d, e, f, g, domain)
}

// Raw hash without domain separation, used in tests only.
#[cfg(test)]
pub(crate) fn hash2(a: M31, b: M31) -> M31 {
    let mut state = [M31::from(0u32); WIDTH];
    state[0] = a;
    state[1] = b;
    permute_state(&mut state);
    state[0]
}

// Note commitment with 4-limb amount encoding.
// Preimage: (asset, amount_L0, amount_L1, amount_L2, amount_L3, owner, randomness)
// This uses 7 of the 8 rate elements in a single Poseidon2 permutation.
// TODO(prod): commitments should output multiple field elements (QM31) for collision resistance
pub fn note_commitment(
    asset: M31, a0: M31, a1: M31, a2: M31, a3: M31, owner: M31, randomness: M31,
) -> M31 {
    hash_many_7_with_domain(asset, a0, a1, a2, a3, owner, randomness, DOMAIN_NOTE_CM)
}

/// Convenience wrapper: decompose a u64 amount into 4 limbs and compute note commitment.
pub fn note_commitment_u64(asset: M31, amount: u64, owner: M31, randomness: M31) -> M31 {
    let limbs = crate::types::amount_to_limbs(amount);
    note_commitment(
        asset,
        M31::from(limbs[0]),
        M31::from(limbs[1]),
        M31::from(limbs[2]),
        M31::from(limbs[3]),
        owner,
        randomness,
    )
}

pub fn credential_commitment(issuer: M31, owner: M31, expiry: M31, secret: M31) -> M31 {
    hash_many_4_with_domain(issuer, owner, expiry, secret, DOMAIN_CRED_CM)
}

pub fn nullifier(sk: M31, commitment: M31) -> M31 {
    hash2_with_domain(sk, commitment, DOMAIN_NULLIFIER)
}

pub fn credential_nullifier(secret: M31, cred_cm: M31, epoch: M31) -> M31 {
    hash_many_4_with_domain(secret, cred_cm, epoch, M31::from(0u32), DOMAIN_CRED_NULL)
}

pub fn derive_owner(sk: M31) -> M31 {
    hash2_with_domain(sk, M31::from(0u32), DOMAIN_OWNER)
}

pub fn derive_issuer_id(issuer_key: M31) -> M31 {
    hash2_with_domain(issuer_key, M31::from(0u32), DOMAIN_ISSUER_ID)
}

pub fn merkle_hash(left: M31, right: M31) -> M31 {
    hash2_with_domain(left, right, DOMAIN_MERKLE)
}

pub fn build_merkle_tree(leaves: &[M31]) -> Vec<M31> {
    let n = leaves.len();
    assert!(n.is_power_of_two() && n >= 2);
    let mut tree = vec![M31::from(0u32); 2 * n];
    tree[n..].copy_from_slice(leaves);
    for i in (1..n).rev() {
        tree[i] = merkle_hash(tree[2 * i], tree[2 * i + 1]);
    }
    tree
}

pub fn merkle_root(leaves: &[M31]) -> M31 {
    build_merkle_tree(leaves)[1]
}

pub fn merkle_path(tree: &[M31], leaf_index: usize) -> Vec<(M31, u32)> {
    let n = tree.len() / 2;
    let mut path = Vec::new();
    let mut idx = n + leaf_index;
    while idx > 1 {
        let sibling = if idx.is_multiple_of(2) { tree[idx + 1] } else { tree[idx - 1] };
        let direction = (idx % 2) as u32;
        path.push((sibling, direction));
        idx /= 2;
    }
    path
}

// Sparse Merkle tree. Avoids hashing 2^depth empty nodes.
use std::collections::HashMap;

pub struct SparseMerkleTree {
    depth: usize,
    nodes: HashMap<usize, M31>,
    default_hashes: Vec<M31>,
}

impl SparseMerkleTree {
    pub fn new(depth: usize) -> Self {
        let mut defaults = vec![M31::from(0u32); depth + 1];
        for d in 1..=depth {
            defaults[d] = merkle_hash(defaults[d - 1], defaults[d - 1]);
        }
        SparseMerkleTree { depth, nodes: HashMap::new(), default_hashes: defaults }
    }

    pub fn set_leaf(&mut self, index: usize, value: M31) {
        let pos = (1 << self.depth) + index;
        self.nodes.insert(pos, value);
        let mut idx = pos;
        while idx > 1 {
            let parent = idx / 2;
            let left = self.get_node(2 * parent);
            let right = self.get_node(2 * parent + 1);
            self.nodes.insert(parent, merkle_hash(left, right));
            idx = parent;
        }
    }

    fn get_node(&self, pos: usize) -> M31 {
        if let Some(&v) = self.nodes.get(&pos) {
            return v;
        }
        let level_from_top = (usize::BITS - pos.leading_zeros() - 1) as usize;
        self.default_hashes[self.depth - level_from_top]
    }

    pub fn root(&self) -> M31 {
        self.get_node(1)
    }

    pub fn path(&self, leaf_index: usize) -> Vec<(M31, u32)> {
        let mut result = Vec::with_capacity(self.depth);
        let mut idx = (1 << self.depth) + leaf_index;
        while idx > 1 {
            let sibling =
                if idx.is_multiple_of(2) { self.get_node(idx + 1) } else { self.get_node(idx - 1) };
            let direction = (idx % 2) as u32;
            result.push((sibling, direction));
            idx /= 2;
        }
        result
    }
}

pub fn verify_merkle_path(leaf: M31, path: &[(M31, u32)], root: M31) -> bool {
    let mut current = leaf;
    for &(sibling, direction) in path {
        if direction == 0 {
            current = merkle_hash(current, sibling);
        } else {
            current = merkle_hash(sibling, current);
        }
    }
    current == root
}

#[cfg(test)]
mod tests {
    use super::*;

    // ====================================================================
    // Ground truth test suite for Poseidon2 over M31.
    // Cross-validated against Plonky3 (github.com/Plonky3/Plonky3).
    // Constants: Grain LFSR, field_type=1, alpha=5, n=31, t=16, R_F=8, R_P=14.
    // ====================================================================

    #[test]
    fn test_plonky3_vector() {
        // Plonky3 test vector 1: permute([0, 1, 2, ..., 15])
        // Source: Plonky3/mersenne-31/src/poseidon2.rs
        let mut state: [M31; 16] = core::array::from_fn(|i| M31::from(i as u32));
        permute_state(&mut state);

        let expected: [u32; 16] = [
            0x0b2c803a, 0x5b1ee4d1, 0x49c6b1e3, 0x2cdc280c, 0x310a60c8, 0x530a729e, 0x4e61bcb4,
            0x2e84d3c3, 0x58709c08, 0x7e82ac42, 0x2162bcef, 0x6d153ab6, 0x742cf0e3, 0x2f21632d,
            0x61adce1e, 0x1973d6f1,
        ];
        for i in 0..16 {
            assert_eq!(
                state[i].0, expected[i],
                "Plonky3 vector mismatch at index {}: got 0x{:08x}, expected 0x{:08x}",
                i, state[i].0, expected[i]
            );
        }
    }

    #[test]
    fn test_constants_spot_check() {
        // Verify round constants match Plonky3's Grain LFSR output exactly.
        // External round 0, first 4 elements:
        assert_eq!(EXTERNAL_CONSTANTS[0][0], 0x768bab52);
        assert_eq!(EXTERNAL_CONSTANTS[0][1], 0x70e0ab7d);
        assert_eq!(EXTERNAL_CONSTANTS[0][2], 0x3d266c8a);
        assert_eq!(EXTERNAL_CONSTANTS[0][3], 0x6da42045);
        // External round 7 (last final round), last 4 elements:
        assert_eq!(EXTERNAL_CONSTANTS[7][12], 0x4f5f3c70);
        assert_eq!(EXTERNAL_CONSTANTS[7][13], 0x0245af6c);
        assert_eq!(EXTERNAL_CONSTANTS[7][14], 0x238359d3);
        assert_eq!(EXTERNAL_CONSTANTS[7][15], 0x49966a59);
        // Internal constants:
        assert_eq!(INTERNAL_CONSTANTS[0], 0x7f7ec4bf);
        assert_eq!(INTERNAL_CONSTANTS[13], 0x1d226a21);
        // Internal diagonal V = [-2, 1, 2, 4, 8, 16, 32, 64, 128, 256, 1024, 4096, 8192, 16384, 32768, 65536]
        assert_eq!(INTERNAL_DIAG_M1[0], -2);
        assert_eq!(INTERNAL_DIAG_M1[1], 1);
        assert_eq!(INTERNAL_DIAG_M1[15], 65536);
        // M4 circulant matrix circ(2,3,1,1):
        assert_eq!(M4[0], [2, 3, 1, 1]);
        assert_eq!(M4[1], [1, 2, 3, 1]);
    }

    #[test]
    fn test_constants_in_range() {
        // Every round constant must be < 2^31 - 1 (valid M31 element)
        let p: u32 = (1u64 << 31) as u32 - 1;
        for r in 0..8 {
            for c in 0..16 {
                assert!(
                    EXTERNAL_CONSTANTS[r][c] < p,
                    "External constant [{}][{}] = 0x{:08x} >= p",
                    r,
                    c,
                    EXTERNAL_CONSTANTS[r][c]
                );
            }
        }
        for r in 0..14 {
            assert!(
                INTERNAL_CONSTANTS[r] < p,
                "Internal constant [{}] = 0x{:08x} >= p",
                r,
                INTERNAL_CONSTANTS[r]
            );
        }
    }

    #[test]
    fn test_sbox_x5() {
        // S-box is x^5 mod (2^31 - 1)
        let p = (1u64 << 31) - 1;
        // x = 0: 0^5 = 0
        assert_eq!(sbox(M31::from(0u32)), M31::from(0u32));
        // x = 1: 1^5 = 1
        assert_eq!(sbox(M31::from(1u32)), M31::from(1u32));
        // x = 2: 2^5 = 32
        assert_eq!(sbox(M31::from(2u32)), M31::from(32u32));
        // x = 3: 3^5 = 243
        assert_eq!(sbox(M31::from(3u32)), M31::from(243u32));
        // x = 10: 10^5 = 100000
        assert_eq!(sbox(M31::from(10u32)), M31::from(100000u32));
        // Large value: verify against direct computation mod p
        let x: u64 = 1_000_000;
        let x5 = ((x * x % p) * (x * x % p) % p) * x % p;
        assert_eq!(sbox(M31::from(x as u32)).0, x5 as u32);
    }

    #[test]
    fn test_m4_circulant_matrix() {
        // M4 = circ(2,3,1,1). Verify on a known input.
        // For input [1,0,0,0]: output should be column 0 = [2,1,1,3]
        let mut chunk = [M31::from(1u32), M31::from(0u32), M31::from(0u32), M31::from(0u32)];
        apply_m4(&mut chunk);
        assert_eq!(chunk[0].0, 2);
        assert_eq!(chunk[1].0, 1);
        assert_eq!(chunk[2].0, 1);
        assert_eq!(chunk[3].0, 3);

        // For input [1,1,1,1]: output should be [7,7,7,7] (row sum = 2+3+1+1 = 7)
        let mut chunk = [M31::from(1u32); 4];
        apply_m4(&mut chunk);
        for i in 0..4 {
            assert_eq!(chunk[i].0, 7, "M4 * [1,1,1,1] should be [7,7,7,7]");
        }
    }

    #[test]
    fn test_external_linear_layer_structure() {
        // External linear layer = M4 on each group of 4, then add column sums.
        // For all-ones input: M4 * [1,1,1,1] = [7,7,7,7] for each chunk.
        // Column sums = [7+7+7+7]*4 = each col_sum = 28.
        // Final: each element = 7 + 28 = 35.
        let mut state = [M31::from(1u32); 16];
        external_linear_layer(&mut state);
        for i in 0..16 {
            assert_eq!(
                state[i].0, 35,
                "external_linear_layer([1;16]) should give [35;16], got {} at index {}",
                state[i].0, i
            );
        }
    }

    #[test]
    fn test_internal_linear_layer_structure() {
        // Internal: result[i] = sum(state) + V[i] * state[i]
        // For input = [1, 0, 0, ..., 0]: sum = 1
        // result[0] = 1 + (-2)*1 = -1 mod p = p-1 = 2147483646
        // result[i>0] = 1 + V[i]*0 = 1
        let p = (1u64 << 31) - 1;
        let mut state = [M31::from(0u32); 16];
        state[0] = M31::from(1u32);
        internal_linear_layer(&mut state);
        assert_eq!(state[0].0, (p - 1) as u32, "Internal layer: sum + (-2)*1 should be p-1");
        for i in 1..16 {
            assert_eq!(state[i].0, 1, "Internal layer: sum + V[i]*0 should be 1 for i={i}");
        }
    }

    #[test]
    fn test_permutation_all_zeros() {
        // All-zeros is a valid input. Permutation should produce non-zero output
        // (round constants break symmetry).
        let mut state = [M31::from(0u32); 16];
        permute_state(&mut state);
        let all_zero = state.iter().all(|x| x.0 == 0);
        assert!(!all_zero, "Permutation of all-zeros must not be all-zeros");
        // Output should be deterministic
        let mut state2 = [M31::from(0u32); 16];
        permute_state(&mut state2);
        assert_eq!(state, state2, "Permutation must be deterministic");
    }

    #[test]
    fn test_permutation_is_deterministic() {
        // Same input always produces same output
        for seed in [0u32, 1, 42, 999999, 2147483646] {
            let mut s1: [M31; 16] =
                core::array::from_fn(|i| M31::from(seed.wrapping_add(i as u32)));
            let mut s2 = s1;
            permute_state(&mut s1);
            permute_state(&mut s2);
            assert_eq!(s1, s2, "Non-deterministic for seed {seed}");
        }
    }

    #[test]
    fn test_avalanche() {
        // Changing a single input element should change most output elements.
        // This is the avalanche property: a 1-bit change in input should flip
        // roughly 50% of output bits on average.
        let mut base: [M31; 16] = core::array::from_fn(|i| M31::from(i as u32 + 100));
        let mut modified = base;
        modified[0] = M31::from(101u32); // change input[0] from 100 to 101
        permute_state(&mut base);
        permute_state(&mut modified);

        let mut changed_elements = 0;
        let mut total_bit_diffs = 0u32;
        for i in 0..16 {
            if base[i] != modified[i] {
                changed_elements += 1;
            }
            total_bit_diffs += (base[i].0 ^ modified[i].0).count_ones();
        }
        // At minimum, most elements should differ (avalanche)
        assert!(
            changed_elements >= 14,
            "Avalanche: only {changed_elements}/16 elements changed, expected >= 14"
        );
        // Total bit difference should be significant (16 elements * 31 bits * ~50% = ~248)
        assert!(
            total_bit_diffs > 100,
            "Avalanche: only {total_bit_diffs} bits differ across all elements, expected > 100"
        );
    }

    #[test]
    fn test_permutation_no_fixed_points_near_identity() {
        // The permutation should not have trivial fixed points for small inputs
        for val in 0u32..20 {
            let mut state: [M31; 16] = core::array::from_fn(|_| M31::from(val));
            let input = state;
            permute_state(&mut state);
            assert_ne!(state, input, "Fixed point found at constant state [{val}; 16]");
        }
    }

    #[test]
    fn test_all_domains_distinct() {
        // Every domain tag pair should produce different outputs for same inputs
        let domains = [
            DOMAIN_OWNER,
            DOMAIN_NULLIFIER,
            DOMAIN_NOTE_CM,
            DOMAIN_CRED_CM,
            DOMAIN_MERKLE,
            DOMAIN_CRED_NULL,
            DOMAIN_ISSUER_ID,
        ];
        let a = M31::from(12345u32);
        let b = M31::from(67890u32);
        let mut outputs = std::collections::HashSet::new();
        for &d in &domains {
            let h = hash2_with_domain(a, b, d);
            assert!(
                outputs.insert(h.0),
                "Domain {} collides with a previous domain for inputs ({}, {})",
                d,
                a.0,
                b.0
            );
        }
    }

    #[test]
    fn test_commitment_binding() {
        // Changing any single input to note_commitment changes the output
        // Using note_commitment_u64(asset, amount_u64, owner, randomness)
        let base = note_commitment_u64(
            M31::from(1u32),
            100u64,
            M31::from(50u32),
            M31::from(999u32),
        );
        // Change asset
        assert_ne!(
            base,
            note_commitment_u64(
                M31::from(2u32),
                100u64,
                M31::from(50u32),
                M31::from(999u32),
            )
        );
        // Change amount
        assert_ne!(
            base,
            note_commitment_u64(
                M31::from(1u32),
                101u64,
                M31::from(50u32),
                M31::from(999u32),
            )
        );
        // Change owner
        assert_ne!(
            base,
            note_commitment_u64(
                M31::from(1u32),
                100u64,
                M31::from(51u32),
                M31::from(999u32),
            )
        );
        // Change blinding
        assert_ne!(
            base,
            note_commitment_u64(
                M31::from(1u32),
                100u64,
                M31::from(50u32),
                M31::from(998u32),
            )
        );
    }

    #[test]
    fn test_credential_commitment_binding() {
        let base = credential_commitment(
            M31::from(10u32),
            M31::from(20u32),
            M31::from(30u32),
            M31::from(40u32),
        );
        assert_ne!(
            base,
            credential_commitment(
                M31::from(11u32),
                M31::from(20u32),
                M31::from(30u32),
                M31::from(40u32)
            )
        );
        assert_ne!(
            base,
            credential_commitment(
                M31::from(10u32),
                M31::from(21u32),
                M31::from(30u32),
                M31::from(40u32)
            )
        );
        assert_ne!(
            base,
            credential_commitment(
                M31::from(10u32),
                M31::from(20u32),
                M31::from(31u32),
                M31::from(40u32)
            )
        );
        assert_ne!(
            base,
            credential_commitment(
                M31::from(10u32),
                M31::from(20u32),
                M31::from(30u32),
                M31::from(41u32)
            )
        );
    }

    #[test]
    fn test_merkle_hash_uses_domain() {
        // merkle_hash(a, b) should equal hash2_with_domain(a, b, DOMAIN_MERKLE)
        let a = M31::from(111u32);
        let b = M31::from(222u32);
        let mh = merkle_hash(a, b);
        let dh = hash2_with_domain(a, b, DOMAIN_MERKLE);
        assert_eq!(mh, dh, "merkle_hash should use DOMAIN_MERKLE");
    }

    #[test]
    fn test_nullifier_binding() {
        // Different secret keys produce different nullifiers for same note
        let note_cm = M31::from(12345u32);
        let n1 = nullifier(M31::from(1u32), note_cm);
        let n2 = nullifier(M31::from(2u32), note_cm);
        assert_ne!(n1, n2, "Different keys must produce different nullifiers");

        // Same key, different notes produce different nullifiers
        let sk = M31::from(42u32);
        let n1 = nullifier(sk, M31::from(100u32));
        let n2 = nullifier(sk, M31::from(101u32));
        assert_ne!(n1, n2, "Same key, different notes must produce different nullifiers");
    }

    #[test]
    fn test_frozen_vector_all_ones() {
        // Frozen test vector: permute([1; 16]).
        // Any change to permute_state will break this, catching regressions.
        let mut state = [M31::from(1u32); 16];
        permute_state(&mut state);
        let expected: [u32; 16] = [
            0x100312c7, 0x550d389b, 0x1bcb8a30, 0x6dcd6369, 0x51f58eef, 0x760c8d95, 0x212b163a,
            0x7af7afe7, 0x79cba4e2, 0x778c0704, 0x2bfeae0a, 0x2ef47d16, 0x6278bbe7, 0x5b115195,
            0x22bd3c30, 0x0b67d488,
        ];
        for i in 0..16 {
            assert_eq!(
                state[i].0, expected[i],
                "all-ones vector mismatch at {}: got 0x{:08x}, expected 0x{:08x}",
                i, state[i].0, expected[i]
            );
        }
    }

    #[test]
    fn test_frozen_vector_all_zeros() {
        // Frozen test vector: permute([0; 16]).
        let mut state = [M31::from(0u32); 16];
        permute_state(&mut state);
        let expected: [u32; 16] = [
            0x7603b10e, 0x58fe3309, 0x543945a9, 0x2f4c48cd, 0x76b45f8f, 0x5691d997, 0x7f8335f1,
            0x06e37263, 0x22757590, 0x34ee15b8, 0x789a34cb, 0x79a11245, 0x2e558d59, 0x62af14f7,
            0x60a19035, 0x349bd141,
        ];
        for i in 0..16 {
            assert_eq!(
                state[i].0, expected[i],
                "all-zeros vector mismatch at {}: got 0x{:08x}, expected 0x{:08x}",
                i, state[i].0, expected[i]
            );
        }
    }

    #[test]
    fn test_frozen_vector_max_field() {
        // Frozen test vector: permute([p-1; 16]) where p = 2^31 - 1.
        let p_minus_1 = (1u32 << 31) - 2;
        let mut state = [M31::from(p_minus_1); 16];
        permute_state(&mut state);
        let expected: [u32; 16] = [
            0x3e32c3cd, 0x0562ae57, 0x1e12f1bd, 0x071f8f8f, 0x0c3e4bd8, 0x6bd699b4, 0x0614e6ef,
            0x37031c07, 0x0bc3c08e, 0x2a16bcdd, 0x4de15393, 0x094255d9, 0x32e87fd9, 0x3fe11acb,
            0x1fa989bf, 0x6ef78d2f,
        ];
        for i in 0..16 {
            assert_eq!(
                state[i].0, expected[i],
                "max-field vector mismatch at {}: got 0x{:08x}, expected 0x{:08x}",
                i, state[i].0, expected[i]
            );
        }
    }

    #[test]
    fn test_hash_deterministic() {
        let a = M31::from(12345u32);
        let b = M31::from(67890u32);
        assert_eq!(hash2(a, b), hash2(a, b));
    }

    #[test]
    fn test_hash_different_inputs() {
        let h1 = hash2(M31::from(1u32), M31::from(2u32));
        let h2 = hash2(M31::from(2u32), M31::from(1u32));
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_domain_separation() {
        let sk = M31::from(42u32);
        // derive_owner and nullifier use different domains, so even if
        // the second input happens to collide, the outputs differ
        let owner = derive_owner(sk);
        let null = nullifier(sk, M31::from(0u32));
        assert_ne!(owner, null, "Domain separation should prevent collision");
    }

    #[test]
    fn test_note_commitment() {
        let cm = note_commitment_u64(
            M31::from(1u32),
            7000u64,
            M31::from(9999u32),
            M31::from(111u32),
        );
        let cm2 = note_commitment_u64(
            M31::from(1u32),
            7000u64,
            M31::from(9999u32),
            M31::from(111u32),
        );
        assert_eq!(cm, cm2);
    }

    #[test]
    fn test_derive_owner() {
        let sk = M31::from(12345u32);
        let owner = derive_owner(sk);
        assert_eq!(owner, derive_owner(sk));
        assert_ne!(owner, derive_owner(M31::from(99999u32)));
    }

    #[test]
    fn test_credential_nullifier_tied_to_commitment() {
        let secret = M31::from(777u32);
        let epoch = M31::from(1000u32);
        let cm1 = M31::from(111u32);
        let cm2 = M31::from(222u32);
        // Different credentials with same secret and epoch produce different nullifiers
        let null1 = credential_nullifier(secret, cm1, epoch);
        let null2 = credential_nullifier(secret, cm2, epoch);
        assert_ne!(null1, null2, "Credential nullifier must be tied to commitment");
    }

    #[test]
    fn test_merkle_tree_and_path() {
        let mut leaves = vec![M31::from(0u32); 256];
        leaves[0] = M31::from(111u32);
        leaves[1] = M31::from(222u32);
        leaves[5] = M31::from(333u32);

        let tree = build_merkle_tree(&leaves);
        let root = tree[1];

        for idx in [0, 1, 5, 100] {
            let path = merkle_path(&tree, idx);
            assert!(
                verify_merkle_path(leaves[idx], &path, root),
                "Merkle verification failed for leaf {idx}"
            );
        }

        // Wrong leaf should fail
        assert!(!verify_merkle_path(M31::from(999u32), &merkle_path(&tree, 0), root));
    }

    #[test]
    fn test_sparse_merkle_tree() {
        // Sparse tree should produce same root as dense tree for same leaves
        let mut dense_leaves = vec![M31::from(0u32); 256];
        dense_leaves[0] = M31::from(111u32);
        dense_leaves[3] = M31::from(444u32);
        let dense_root = build_merkle_tree(&dense_leaves)[1];

        let mut sparse = SparseMerkleTree::new(8);
        sparse.set_leaf(0, M31::from(111u32));
        sparse.set_leaf(3, M31::from(444u32));
        assert_eq!(sparse.root(), dense_root, "Sparse and dense roots must match");

        // Paths should also match
        let dense_tree = build_merkle_tree(&dense_leaves);
        for idx in [0, 3] {
            let dense_path = merkle_path(&dense_tree, idx);
            let sparse_path = sparse.path(idx);
            assert_eq!(dense_path.len(), sparse_path.len());
            for i in 0..dense_path.len() {
                assert_eq!(dense_path[i].0, sparse_path[i].0, "Sibling mismatch at level {i}");
                assert_eq!(dense_path[i].1, sparse_path[i].1, "Direction mismatch at level {i}");
            }
        }

        // Verify paths against root
        for idx in [0, 3] {
            let leaf = if idx == 0 { M31::from(111u32) } else { M31::from(444u32) };
            assert!(verify_merkle_path(leaf, &sparse.path(idx), sparse.root()));
        }
    }

    #[test]
    fn regression_hash_42_99() {
        // Hardcoded regression value, do not change
        let h = hash2_with_domain(M31::from(42u32), M31::from(99u32), DOMAIN_NULLIFIER);
        assert_eq!(h.0, nullifier(M31::from(42u32), M31::from(99u32)).0);
    }

    #[test]
    fn test_sparse_tree_single_leaf() {
        let mut tree = SparseMerkleTree::new(20);
        tree.set_leaf(7, M31::from(0xdeadu32));
        let path = tree.path(7);
        assert!(verify_merkle_path(M31::from(0xdeadu32), &path, tree.root()));
        assert!(!verify_merkle_path(M31::from(0xbeefu32), &path, tree.root()));
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn commitment_binding(
            asset in 0u32..((1u32 << 31) - 2),
            amt in 0u32..((1u32 << 21) - 1),
            owner in 1u32..((1u32 << 31) - 2),
            rand in 0u32..((1u32 << 31) - 2),
        ) {
            let base = note_commitment_u64(
                M31::from(asset), u64::from(amt), M31::from(owner), M31::from(rand),
            );
            // Changing any single input must change the output
            let c1 = note_commitment_u64(
                M31::from(asset.wrapping_add(1) % ((1u32 << 31) - 1)),
                u64::from(amt), M31::from(owner), M31::from(rand),
            );
            prop_assert_ne!(base, c1);
        }

        #[test]
        fn merkle_path_roundtrip(seed in 0u32..1000u32) {
            let leaves: Vec<M31> = (0..16).map(|i| M31::from(seed + i)).collect();
            let tree = build_merkle_tree(&leaves);
            let root = tree[1];
            for idx in 0..16 {
                let path = merkle_path(&tree, idx as usize);
                prop_assert!(verify_merkle_path(leaves[idx as usize], &path, root));
            }
        }

        #[test]
        fn distinct_keys_distinct_nullifiers(
            sk1 in 1u32..((1u32 << 31) - 2),
            sk2 in 1u32..((1u32 << 31) - 2),
            cm in 1u32..((1u32 << 31) - 2),
        ) {
            prop_assume!(sk1 != sk2);
            let n1 = nullifier(M31::from(sk1), M31::from(cm));
            let n2 = nullifier(M31::from(sk2), M31::from(cm));
            prop_assert_ne!(n1, n2);
        }
    }
}
