/* tslint:disable */
/* eslint-disable */

/**
 * Result type for audit proofs, with serialized proof bytes for independent verification.
 */
export class AuditOutput {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    readonly claimed_total: number;
    readonly cred_null: string;
    readonly cred_root: string;
    readonly epoch: number;
    readonly log_num_rows: number;
    readonly message: string;
    readonly proof_bytes: string;
    readonly prove_time_ms: number;
    readonly success: boolean;
    readonly verify_time_ms: number;
    readonly window_end: number;
    readonly window_start: number;
}

export class CredentialIssuanceOutput {
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
    readonly accumulator_root: string;
    readonly epoch: number;
    readonly log_num_rows: number;
    readonly message: string;
    readonly note_root: string;
    readonly null_0: string;
    readonly null_1: string;
    readonly out_cm_0: string;
    readonly out_cm_1: string;
    readonly proof_bytes: string;
    readonly prove_time_ms: number;
    readonly success: boolean;
    readonly verify_time_ms: number;
}

export function dual_fee_quote_payment_with_schedule_json(payment_asset: number, fee_asset: number, amount: number, fee_schedule_version: number): string;

export function dual_fee_submit_demo_payment_json(payment_asset: number, fee_asset: number, amount: number, fee_schedule_version: number, recipient_owner: number, payment_balance: number, hush_balance: number, credential_expiry: number): string;

export function prove_demo_provenance_attestation(sk: number, issuer_key: number, expiry: number, secret: number): CredentialIssuanceOutput;

/**
 * Proves a time-window audit for the browser demo.
 * Amounts are passed as f64 (protocol units, same transport as payment circuit).
 */
export function prove_time_window_audit(window_start: number, window_end: number, amounts: Float64Array, timestamps: Uint32Array, sk: number, cred_issuer: number, cred_expiry: number, cred_secret: number): AuditOutput;

/**
 * Recompute tx_binding_hash from a JSON-encoded binding preimage.
 * Returns `{"hash": <u32>}` on success or `{"error": "..."}` on failure.
 * Uses a JSON interface instead of individual f64 parameters to avoid
 * deepening the fragile JS-to-WASM numeric boundary.
 */
export function recompute_tx_binding_hash_json(binding_json: string): string;

/**
 * Independently verify a serialized time-window audit proof.
 * Returns "ok" on success, error message on failure.
 */
export function verify_audit_proof(proof_b64: string, window_start: number, window_end: number, claimed_total: number, cred_root: Uint32Array, cred_null: Uint32Array, epoch: number, log_num_rows: number): string;

/**
 * Verify a serialized STARK proof against its public outputs.
 * proof_b64: base64-encoded JSON of the serialized StarkProof.
 * log_num_rows: the trace height exponent used when the proof was generated.
 *   This is required because different circuit shapes (single payment, batch)
 *   use different trace sizes. The prover returns this value in ProofOutput.
 * Returns a JS string: "ok" on success, error message on failure.
 */
export function verify_serialized_proof(proof_b64: string, note_root: Uint32Array, accumulator_root: Uint32Array, epoch: number, null_0: Uint32Array, null_1: Uint32Array, out_cm_0: Uint32Array, out_cm_1: Uint32Array, tx_binding_hash: Uint32Array, sender_binding_tag: Uint32Array, log_num_rows: number): string;

export function wasm_init(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly wasm_init: () => void;
    readonly __wbg_proofoutput_free: (a: number, b: number) => void;
    readonly proofoutput_success: (a: number) => number;
    readonly proofoutput_message: (a: number, b: number) => void;
    readonly proofoutput_prove_time_ms: (a: number) => number;
    readonly proofoutput_verify_time_ms: (a: number) => number;
    readonly proofoutput_null_0: (a: number, b: number) => void;
    readonly proofoutput_null_1: (a: number, b: number) => void;
    readonly proofoutput_out_cm_0: (a: number, b: number) => void;
    readonly proofoutput_out_cm_1: (a: number, b: number) => void;
    readonly proofoutput_proof_bytes: (a: number, b: number) => void;
    readonly proofoutput_note_root: (a: number, b: number) => void;
    readonly proofoutput_accumulator_root: (a: number, b: number) => void;
    readonly proofoutput_epoch: (a: number) => number;
    readonly proofoutput_log_num_rows: (a: number) => number;
    readonly recompute_tx_binding_hash_json: (a: number, b: number, c: number) => void;
    readonly dual_fee_quote_payment_with_schedule_json: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly dual_fee_submit_demo_payment_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly __wbg_auditoutput_free: (a: number, b: number) => void;
    readonly auditoutput_success: (a: number) => number;
    readonly auditoutput_message: (a: number, b: number) => void;
    readonly auditoutput_prove_time_ms: (a: number) => number;
    readonly auditoutput_verify_time_ms: (a: number) => number;
    readonly auditoutput_proof_bytes: (a: number, b: number) => void;
    readonly auditoutput_window_start: (a: number) => number;
    readonly auditoutput_window_end: (a: number) => number;
    readonly auditoutput_claimed_total: (a: number) => number;
    readonly auditoutput_cred_null: (a: number, b: number) => void;
    readonly auditoutput_cred_root: (a: number, b: number) => void;
    readonly auditoutput_epoch: (a: number) => number;
    readonly auditoutput_log_num_rows: (a: number) => number;
    readonly __wbg_credentialissuanceoutput_free: (a: number, b: number) => void;
    readonly credentialissuanceoutput_success: (a: number) => number;
    readonly credentialissuanceoutput_message: (a: number, b: number) => void;
    readonly credentialissuanceoutput_prove_time_ms: (a: number) => number;
    readonly prove_demo_provenance_attestation: (a: number, b: number, c: number, d: number) => number;
    readonly prove_time_window_audit: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => number;
    readonly verify_audit_proof: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => void;
    readonly verify_serialized_proof: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number, q: number, r: number, s: number, t: number, u: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export: (a: number, b: number, c: number) => void;
    readonly __wbindgen_export2: (a: number, b: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_start: () => void;
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
