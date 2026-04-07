/* tslint:disable */
/* eslint-disable */

/**
 * Simple result type for audit proofs.
 */
export class AuditOutput {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    readonly message: string;
    readonly prove_time_ms: number;
    readonly success: boolean;
}

export class ProofOutput {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    readonly cred_null: number;
    readonly cred_root: number;
    readonly epoch: number;
    readonly message: string;
    readonly note_root: number;
    readonly null_0: number;
    readonly null_1: number;
    readonly out_cm_0: number;
    readonly out_cm_1: number;
    readonly proof_bytes: string;
    readonly prove_time_ms: number;
    readonly success: boolean;
    readonly verify_time_ms: number;
}

/**
 * Proves a payment from raw parameters. Builds all witness data internally.
 */
export function build_witness_and_prove(epoch: number, sk: number, in_asset: number, in_amt_0: number, in_amt_1: number, out_amt_0: number, out_owner_0: number, out_amt_1: number, cred_issuer: number, cred_expiry: number, cred_secret: number): ProofOutput;

export function compute_credential_root(sk: number, issuer: number, expiry: number, secret: number): number;

export function compute_merkle_path(leaf_index: number, leaf_values_flat: Uint32Array): Uint32Array;

export function compute_note_root(sk: number, in_asset: number, in_amt_0: number, in_rand_0: number, in_amt_1: number, in_rand_1: number): number;

export function dual_fee_quote_payment_json(payment_asset: number, fee_asset: number, amount: number): string;

export function dual_fee_review_json(): string;

export function dual_fee_submit_demo_payment_json(payment_asset: number, fee_asset: number, amount: number, recipient_owner: number, payment_balance: number, hush_balance: number, credential_expiry: number): string;

export function prove_and_verify(epoch: number, note_root: number, cred_root: number, sk: number, in_asset: number, in_amt_0: number, in_rand_0: number, in_amt_1: number, in_rand_1: number, out_amt_0: number, out_owner_0: number, out_rand_0: number, out_amt_1: number, out_rand_1: number, cred_issuer: number, cred_expiry: number, cred_secret: number, note_path_0_flat: Uint32Array, note_path_1_flat: Uint32Array, cred_path_flat: Uint32Array): ProofOutput;

/**
 * Proves a time-window audit for the browser demo.
 */
export function prove_time_window_audit(window_start: number, window_end: number, amounts: Uint32Array, timestamps: Uint32Array, sk: number, cred_issuer: number, cred_expiry: number, cred_secret: number): AuditOutput;

/**
 * Verify a serialized STARK proof against its public outputs.
 * proof_b64: base64-encoded JSON of the serialized StarkProof.
 * Returns a JS string: "ok" on success, error message on failure.
 */
export function verify_serialized_proof(proof_b64: string, note_root: number, cred_root: number, epoch: number, null_0: number, null_1: number, out_cm_0: number, out_cm_1: number, cred_null: number): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_proofoutput_free: (a: number, b: number) => void;
    readonly proofoutput_success: (a: number) => number;
    readonly proofoutput_message: (a: number, b: number) => void;
    readonly proofoutput_verify_time_ms: (a: number) => number;
    readonly proofoutput_null_0: (a: number) => number;
    readonly proofoutput_null_1: (a: number) => number;
    readonly proofoutput_out_cm_0: (a: number) => number;
    readonly proofoutput_out_cm_1: (a: number) => number;
    readonly proofoutput_cred_null: (a: number) => number;
    readonly proofoutput_proof_bytes: (a: number, b: number) => void;
    readonly proofoutput_note_root: (a: number) => number;
    readonly proofoutput_cred_root: (a: number) => number;
    readonly proofoutput_epoch: (a: number) => number;
    readonly dual_fee_review_json: (a: number) => void;
    readonly dual_fee_quote_payment_json: (a: number, b: number, c: number, d: number) => void;
    readonly dual_fee_submit_demo_payment_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly prove_and_verify: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number, q: number, r: number, s: number, t: number, u: number, v: number, w: number) => number;
    readonly __wbg_auditoutput_free: (a: number, b: number) => void;
    readonly auditoutput_success: (a: number) => number;
    readonly auditoutput_message: (a: number, b: number) => void;
    readonly auditoutput_prove_time_ms: (a: number) => number;
    readonly prove_time_window_audit: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => number;
    readonly build_witness_and_prove: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => number;
    readonly verify_serialized_proof: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => void;
    readonly compute_credential_root: (a: number, b: number, c: number, d: number) => number;
    readonly compute_note_root: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly compute_merkle_path: (a: number, b: number, c: number, d: number) => void;
    readonly proofoutput_prove_time_ms: (a: number) => number;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export: (a: number, b: number, c: number) => void;
    readonly __wbindgen_export2: (a: number, b: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number, d: number) => number;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
