/* @ts-self-types="./hush_demo_stark.d.ts" */

/**
 * Simple result type for audit proofs.
 */
export class AuditOutput {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(AuditOutput.prototype);
        obj.__wbg_ptr = ptr;
        AuditOutputFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        AuditOutputFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_auditoutput_free(ptr, 0);
    }
    /**
     * @returns {string}
     */
    get message() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.auditoutput_message(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {number}
     */
    get prove_time_ms() {
        const ret = wasm.auditoutput_prove_time_ms(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {boolean}
     */
    get success() {
        const ret = wasm.auditoutput_success(this.__wbg_ptr);
        return ret !== 0;
    }
}
if (Symbol.dispose) AuditOutput.prototype[Symbol.dispose] = AuditOutput.prototype.free;

export class ProofOutput {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(ProofOutput.prototype);
        obj.__wbg_ptr = ptr;
        ProofOutputFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ProofOutputFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_proofoutput_free(ptr, 0);
    }
    /**
     * @returns {string}
     */
    get cred_null() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.proofoutput_cred_null(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    get cred_root() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.proofoutput_cred_root(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {number}
     */
    get epoch() {
        const ret = wasm.proofoutput_epoch(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get log_num_rows() {
        const ret = wasm.proofoutput_log_num_rows(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {string}
     */
    get message() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.proofoutput_message(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    get note_root() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.proofoutput_note_root(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    get null_0() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.proofoutput_null_0(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    get null_1() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.proofoutput_null_1(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    get out_cm_0() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.proofoutput_out_cm_0(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    get out_cm_1() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.proofoutput_out_cm_1(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    get proof_bytes() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.proofoutput_proof_bytes(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {number}
     */
    get prove_time_ms() {
        const ret = wasm.proofoutput_prove_time_ms(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {boolean}
     */
    get success() {
        const ret = wasm.proofoutput_success(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {number}
     */
    get verify_time_ms() {
        const ret = wasm.proofoutput_verify_time_ms(this.__wbg_ptr);
        return ret;
    }
}
if (Symbol.dispose) ProofOutput.prototype[Symbol.dispose] = ProofOutput.prototype.free;

/**
 * High-level wrapper for the browser demo: takes simple payment parameters,
 * computes randomness, builds Merkle trees and paths internally, proves and verifies.
 * Returns a ProofOutput including proof_bytes for independent verification.
 * @param {number} epoch
 * @param {number} sk
 * @param {number} in_asset
 * @param {number} in_amt_0
 * @param {number} in_amt_1
 * @param {number} out_amt_0
 * @param {Uint32Array} out_owner_0
 * @param {number} out_amt_1
 * @param {number} cred_issuer
 * @param {number} cred_expiry
 * @param {number} cred_secret
 * @returns {ProofOutput}
 */
export function build_witness_and_prove(epoch, sk, in_asset, in_amt_0, in_amt_1, out_amt_0, out_owner_0, out_amt_1, cred_issuer, cred_expiry, cred_secret) {
    const ptr0 = passArray32ToWasm0(out_owner_0, wasm.__wbindgen_export2);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.build_witness_and_prove(epoch, sk, in_asset, in_amt_0, in_amt_1, out_amt_0, ptr0, len0, out_amt_1, cred_issuer, cred_expiry, cred_secret);
    return ProofOutput.__wrap(ret);
}

/**
 * @param {number} sk
 * @param {number} issuer
 * @param {number} expiry
 * @param {number} secret
 * @returns {Uint32Array}
 */
export function compute_credential_root(sk, issuer, expiry, secret) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.compute_credential_root(retptr, sk, issuer, expiry, secret);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var v1 = getArrayU32FromWasm0(r0, r1).slice();
        wasm.__wbindgen_export(r0, r1 * 4, 4);
        return v1;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}

/**
 * @param {number} leaf_index
 * @param {Uint32Array} leaf_values_flat
 * @returns {Uint32Array}
 */
export function compute_merkle_path(leaf_index, leaf_values_flat) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passArray32ToWasm0(leaf_values_flat, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        wasm.compute_merkle_path(retptr, leaf_index, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
        var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
        if (r3) {
            throw takeObject(r2);
        }
        var v2 = getArrayU32FromWasm0(r0, r1).slice();
        wasm.__wbindgen_export(r0, r1 * 4, 4);
        return v2;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}

/**
 * @param {number} sk
 * @param {number} in_asset
 * @param {number} in_amt_0
 * @param {number} in_rand_0
 * @param {number} in_amt_1
 * @param {number} in_rand_1
 * @returns {Uint32Array}
 */
export function compute_note_root(sk, in_asset, in_amt_0, in_rand_0, in_amt_1, in_rand_1) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.compute_note_root(retptr, sk, in_asset, in_amt_0, in_rand_0, in_amt_1, in_rand_1);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
        var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
        if (r3) {
            throw takeObject(r2);
        }
        var v1 = getArrayU32FromWasm0(r0, r1).slice();
        wasm.__wbindgen_export(r0, r1 * 4, 4);
        return v1;
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}

/**
 * @param {number} payment_asset
 * @param {number} fee_asset
 * @param {number} amount
 * @returns {string}
 */
export function dual_fee_quote_payment_json(payment_asset, fee_asset, amount) {
    let deferred1_0;
    let deferred1_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.dual_fee_quote_payment_json(retptr, payment_asset, fee_asset, amount);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred1_0 = r0;
        deferred1_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
    }
}

/**
 * @returns {string}
 */
export function dual_fee_review_json() {
    let deferred1_0;
    let deferred1_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.dual_fee_review_json(retptr);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred1_0 = r0;
        deferred1_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
    }
}

/**
 * @param {number} payment_asset
 * @param {number} fee_asset
 * @param {number} amount
 * @param {number} recipient_owner
 * @param {number} payment_balance
 * @param {number} hush_balance
 * @param {number} credential_expiry
 * @returns {string}
 */
export function dual_fee_submit_demo_payment_json(payment_asset, fee_asset, amount, recipient_owner, payment_balance, hush_balance, credential_expiry) {
    let deferred1_0;
    let deferred1_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.dual_fee_submit_demo_payment_json(retptr, payment_asset, fee_asset, amount, recipient_owner, payment_balance, hush_balance, credential_expiry);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred1_0 = r0;
        deferred1_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export(deferred1_0, deferred1_1, 1);
    }
}

/**
 * @param {number} epoch
 * @param {Uint32Array} note_root
 * @param {Uint32Array} cred_root
 * @param {number} sk
 * @param {number} in_asset
 * @param {number} in_amt_0
 * @param {number} in_rand_0
 * @param {number} in_amt_1
 * @param {number} in_rand_1
 * @param {number} out_amt_0
 * @param {Uint32Array} out_owner_0
 * @param {number} out_rand_0
 * @param {number} out_amt_1
 * @param {number} out_rand_1
 * @param {number} cred_issuer
 * @param {number} cred_expiry
 * @param {number} cred_secret
 * @param {Uint32Array} note_path_0_flat
 * @param {Uint32Array} note_path_1_flat
 * @param {Uint32Array} cred_path_flat
 * @returns {ProofOutput}
 */
export function prove_and_verify(epoch, note_root, cred_root, sk, in_asset, in_amt_0, in_rand_0, in_amt_1, in_rand_1, out_amt_0, out_owner_0, out_rand_0, out_amt_1, out_rand_1, cred_issuer, cred_expiry, cred_secret, note_path_0_flat, note_path_1_flat, cred_path_flat) {
    const ptr0 = passArray32ToWasm0(note_root, wasm.__wbindgen_export2);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray32ToWasm0(cred_root, wasm.__wbindgen_export2);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passArray32ToWasm0(out_owner_0, wasm.__wbindgen_export2);
    const len2 = WASM_VECTOR_LEN;
    const ptr3 = passArray32ToWasm0(note_path_0_flat, wasm.__wbindgen_export2);
    const len3 = WASM_VECTOR_LEN;
    const ptr4 = passArray32ToWasm0(note_path_1_flat, wasm.__wbindgen_export2);
    const len4 = WASM_VECTOR_LEN;
    const ptr5 = passArray32ToWasm0(cred_path_flat, wasm.__wbindgen_export2);
    const len5 = WASM_VECTOR_LEN;
    const ret = wasm.prove_and_verify(epoch, ptr0, len0, ptr1, len1, sk, in_asset, in_amt_0, in_rand_0, in_amt_1, in_rand_1, out_amt_0, ptr2, len2, out_rand_0, out_amt_1, out_rand_1, cred_issuer, cred_expiry, cred_secret, ptr3, len3, ptr4, len4, ptr5, len5);
    return ProofOutput.__wrap(ret);
}

/**
 * Proves a time-window audit for the browser demo.
 * @param {number} window_start
 * @param {number} window_end
 * @param {Uint32Array} amounts
 * @param {Uint32Array} timestamps
 * @param {number} sk
 * @param {number} cred_issuer
 * @param {number} cred_expiry
 * @param {number} cred_secret
 * @returns {AuditOutput}
 */
export function prove_time_window_audit(window_start, window_end, amounts, timestamps, sk, cred_issuer, cred_expiry, cred_secret) {
    const ptr0 = passArray32ToWasm0(amounts, wasm.__wbindgen_export2);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray32ToWasm0(timestamps, wasm.__wbindgen_export2);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.prove_time_window_audit(window_start, window_end, ptr0, len0, ptr1, len1, sk, cred_issuer, cred_expiry, cred_secret);
    return AuditOutput.__wrap(ret);
}

/**
 * Recompute tx_binding_hash from a JSON-encoded binding preimage.
 * Returns `{"hash": <u32>}` on success or `{"error": "..."}` on failure.
 * Uses a JSON interface instead of individual f64 parameters to avoid
 * deepening the fragile JS-to-WASM numeric boundary.
 * @param {string} binding_json
 * @returns {string}
 */
export function recompute_tx_binding_hash_json(binding_json) {
    let deferred2_0;
    let deferred2_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(binding_json, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
        const len0 = WASM_VECTOR_LEN;
        wasm.recompute_tx_binding_hash_json(retptr, ptr0, len0);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred2_0 = r0;
        deferred2_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export(deferred2_0, deferred2_1, 1);
    }
}

/**
 * Verify a serialized STARK proof against its public outputs.
 * proof_b64: base64-encoded JSON of the serialized StarkProof.
 * log_num_rows: the trace height exponent used when the proof was generated.
 *   This is required because different circuit shapes (single payment, batch)
 *   use different trace sizes. The prover returns this value in ProofOutput.
 * Returns a JS string: "ok" on success, error message on failure.
 * @param {string} proof_b64
 * @param {Uint32Array} note_root
 * @param {Uint32Array} cred_root
 * @param {number} epoch
 * @param {Uint32Array} null_0
 * @param {Uint32Array} null_1
 * @param {Uint32Array} out_cm_0
 * @param {Uint32Array} out_cm_1
 * @param {Uint32Array} cred_null
 * @param {Uint32Array} tx_binding_hash
 * @param {Uint32Array} sender_binding_tag
 * @param {number} log_num_rows
 * @returns {string}
 */
export function verify_serialized_proof(proof_b64, note_root, cred_root, epoch, null_0, null_1, out_cm_0, out_cm_1, cred_null, tx_binding_hash, sender_binding_tag, log_num_rows) {
    let deferred11_0;
    let deferred11_1;
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        const ptr0 = passStringToWasm0(proof_b64, wasm.__wbindgen_export2, wasm.__wbindgen_export3);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray32ToWasm0(note_root, wasm.__wbindgen_export2);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArray32ToWasm0(cred_root, wasm.__wbindgen_export2);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passArray32ToWasm0(null_0, wasm.__wbindgen_export2);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passArray32ToWasm0(null_1, wasm.__wbindgen_export2);
        const len4 = WASM_VECTOR_LEN;
        const ptr5 = passArray32ToWasm0(out_cm_0, wasm.__wbindgen_export2);
        const len5 = WASM_VECTOR_LEN;
        const ptr6 = passArray32ToWasm0(out_cm_1, wasm.__wbindgen_export2);
        const len6 = WASM_VECTOR_LEN;
        const ptr7 = passArray32ToWasm0(cred_null, wasm.__wbindgen_export2);
        const len7 = WASM_VECTOR_LEN;
        const ptr8 = passArray32ToWasm0(tx_binding_hash, wasm.__wbindgen_export2);
        const len8 = WASM_VECTOR_LEN;
        const ptr9 = passArray32ToWasm0(sender_binding_tag, wasm.__wbindgen_export2);
        const len9 = WASM_VECTOR_LEN;
        wasm.verify_serialized_proof(retptr, ptr0, len0, ptr1, len1, ptr2, len2, epoch, ptr3, len3, ptr4, len4, ptr5, len5, ptr6, len6, ptr7, len7, ptr8, len8, ptr9, len9, log_num_rows);
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        deferred11_0 = r0;
        deferred11_1 = r1;
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
        wasm.__wbindgen_export(deferred11_0, deferred11_1, 1);
    }
}

export function wasm_init() {
    wasm.wasm_init();
}

function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_Error_55538483de6e3abe: function(arg0, arg1) {
            const ret = Error(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        },
        __wbg___wbindgen_throw_5549492daedad139: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_now_46736a527d2e74e7: function() {
            const ret = Date.now();
            return ret;
        },
        __wbg_random_09b0bd71e83551d7: function() {
            const ret = Math.random();
            return ret;
        },
    };
    return {
        __proto__: null,
        "./hush_demo_stark_bg.js": import0,
    };
}

const AuditOutputFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_auditoutput_free(ptr >>> 0, 1));
const ProofOutputFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_proofoutput_free(ptr >>> 0, 1));

function addHeapObject(obj) {
    if (heap_next === heap.length) heap.push(heap.length + 1);
    const idx = heap_next;
    heap_next = heap[idx];

    heap[idx] = obj;
    return idx;
}

function dropObject(idx) {
    if (idx < 1028) return;
    heap[idx] = heap_next;
    heap_next = idx;
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function getObject(idx) { return heap[idx]; }

let heap = new Array(1024).fill(undefined);
heap.push(undefined, null, true, false);

let heap_next = heap.length;

function passArray32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getUint32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeObject(idx) {
    const ret = getObject(idx);
    dropObject(idx);
    return ret;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasm;
function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('hush_demo_stark_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
