import './wallet.css';
import init, {
  dual_fee_quote_payment_with_schedule_json,
  dual_fee_submit_demo_payment_json,
  prove_demo_provenance_attestation,
  prove_time_window_audit,
} from '../pkg/hush_demo_stark.js';
import {
  createReceiptId,
  esc,
  fmtAssetValue,
  fmtFee,
  fmtHash4,
  fmtMoney,
  parseAmountInput,
  relativeTime,
  sanitizeAmountInput,
} from './lib/formatters.js';
import {
  renderActivity,
  renderPayoutPreview,
  renderProofLog,
  renderProofOutputs,
} from './lib/renderers.js';
import {
  renderSidebar,
  renderTopbar,
  renderBalanceCard,
  renderBalancesTable,
  renderRecentActivity,
  renderPrivacyNote,
  renderTweaksPanel,
  renderComposerOverlay,
  renderComingSoon,
} from './lib/walletShell.js';

// 1 protocol unit = $0.0001 (4 decimal places). Circuit uses four-limb radix-2^15 encoding
// with u64 amounts (max ~$115 trillion per note). Display dollars * AMT_SCALE = protocol units.
const AMT_SCALE = 10_000;

const SK = 12345;
const CRED_ISSUER = 1;
const CRED_EXPIRY = 50_000;
const CRED_SECRET = 777;

const USER_HANDLE = 'UserName.hush';
const DEFAULT_RECIPIENT = 'alice.hush';
const DEFAULT_AMOUNT = '500.00';
const INITIAL_BALANCES_UNITS = { USDC: 2_000 * AMT_SCALE, USDT: 1_000 * AMT_SCALE };
// 1,284.32 HUSH * AMT_SCALE (1284.32 -> 12,843,200 protocol units)
const INITIAL_HUSH_BALANCE_UNITS = 12_843_200;

let wasmReady = false;
let wasmError = null;

const state = {
  provenanceStatus: 'valid',
  activeAsset: 'USDC',
  feeMode: 'same_asset',
  currentRecipient: DEFAULT_RECIPIENT,
  currentAmountInput: DEFAULT_AMOUNT,
  balancesUnits: { ...INITIAL_BALANCES_UNITS },
  hushBalanceUnits: INITIAL_HUSH_BALANCE_UNITS,
  activity: [],
  transactions: [],
  proofLog: [
    makeLog('info', 'Waiting for the first payment proof.'),
  ],
  proofOutputs: [],
  timings: null,
  isSending: false,
  provenanceProof: null,
  receiptTxId: null,
  successTxId: null,
  auditLoading: false,
  auditResult: null,
  lastSubmission: null,
  // Wallet UI state
  theme: 'dark',
  activeView: 'wallet',
  composerOpen: false,
  tweaksOpen: false,
  sidebarOpen: false,
  balancesTab: 'balances',
  tweaks: {
    accent: 'mint',
    density: 'cozy',
    cardStyle: 'soft',
    radius: 14,
    fontSize: 14,
  },
};

const splash = document.getElementById('splash');
const app = document.getElementById('app');
const stage = document.getElementById('stage');
const rail = document.getElementById('rail');
const sidebarEl = document.getElementById('sidebar');
const topbarEl = document.getElementById('topbar');
const tweaksEl = document.getElementById('tweaks');
const composerOverlayEl = document.getElementById('composer-overlay');
const truthContent = document.getElementById('truth-content');

// HUSH spot-price assumption for the BalanceCard / asset list display only.
// Picked so the initial 1,284.32 HUSH balance values at $85.50, matching the
// design reference. The proving stack does not use this number.
const HUSH_USD_PRICE = 85.50 / 1284.32;

function balanceDisplayFor(sym) {
  if (sym === 'USDT' || sym === 'USDC') {
    const bal = state.balancesUnits[sym] / AMT_SCALE;
    return { balance: fmtMoney(bal), value: `$${fmtMoney(bal)}` };
  }
  if (sym === 'HUSH') {
    const bal = state.hushBalanceUnits / AMT_SCALE;
    return { balance: fmtMoney(bal), value: `$${fmtMoney(bal * HUSH_USD_PRICE)}` };
  }
  return { balance: '0.00', value: '$0.00' };
}

const ACCENT_PRESETS = {
  mint:   { accent: '#0891b2', accent2: '#22d3ee', deep: '#5eead4' },
  sage:   { accent: '#10b981', accent2: '#5eead4', deep: '#34d399' },
  violet: { accent: '#8b5cf6', accent2: '#a78bfa', deep: '#c4b5fd' },
  sky:    { accent: '#0ea5e9', accent2: '#38bdf8', deep: '#7dd3fc' },
  rose:   { accent: '#f43f5e', accent2: '#fb7185', deep: '#fda4af' },
};

async function boot() {
  const minimumSplash = new Promise(resolve => setTimeout(resolve, 450));
  await Promise.all([init(), minimumSplash]);
  wasmReady = true;
  try {
    state.provenanceProof = prove_demo_provenance_attestation(SK, CRED_ISSUER, CRED_EXPIRY, CRED_SECRET);
  } catch (error) {
    console.error('Provenance attestation proof unavailable:', error);
  }
  render();
  splash.classList.add('hidden');
  app.classList.add('visible');
}

boot().catch((error) => {
  wasmError = error;
  console.error('WASM init failed:', error);
  const copy = document.querySelector('.splash-copy');
  if (copy) copy.textContent = `Failed to load prover: ${error.message}`;
});

function applyShellAttributes() {
  if (!app) return;
  app.setAttribute('data-theme', state.theme);
  app.setAttribute('data-density', state.tweaks.density);
  app.setAttribute('data-card', state.tweaks.cardStyle);
  const preset = ACCENT_PRESETS[state.tweaks.accent] || ACCENT_PRESETS.mint;
  app.style.setProperty('--brand-accent', preset.accent);
  app.style.setProperty('--brand-accent-2', preset.accent2);
  app.style.setProperty('--brand-accent-deep', preset.deep);
  app.style.setProperty('--w-radius', `${state.tweaks.radius}px`);
  app.style.setProperty('--w-size-base', `${state.tweaks.fontSize}px`);
  if (tweaksEl) tweaksEl.classList.toggle('open', state.tweaksOpen);
  app.classList.toggle('sidebar-open', state.sidebarOpen);
}

function render() {
  applyShellAttributes();
  if (sidebarEl) sidebarEl.innerHTML = renderSidebar(state.activeView);
  if (topbarEl) topbarEl.innerHTML = renderTopbar(state.theme, USER_HANDLE);
  if (stage) stage.innerHTML = renderStage();
  if (rail) rail.innerHTML = renderRail();
  if (tweaksEl) tweaksEl.innerHTML = renderTweaksPanel(state);
  if (composerOverlayEl) {
    composerOverlayEl.innerHTML = state.composerOpen ? renderComposerOverlay(renderComposerBody()) : '';
    composerOverlayEl.classList.toggle('show', state.composerOpen);
  }
  if (truthContent) truthContent.innerHTML = renderTruthOverlayView();
  const amountInput = document.getElementById('amount-input');
  const recipientInput = document.getElementById('recipient-input');
  if (amountInput) amountInput.value = state.currentAmountInput;
  if (recipientInput) recipientInput.value = state.currentRecipient;
  refreshSendSummary();
}

function renderRail() {
  return renderRecentActivity(state.transactions) + renderPrivacyNote();
}

function currentAmount() {
  return parseAmountInput(state.currentAmountInput);
}

function assetId(asset) {
  if (asset === 'USDC') return 1;
  if (asset === 'USDT') return 2;
  if (asset === 'HUSH') return 3;
  throw new Error(`Unsupported asset ${asset}`);
}

function balanceAmountFor(asset) {
  if (asset === 'HUSH') return state.hushBalanceUnits / AMT_SCALE;
  return state.balancesUnits[asset] / AMT_SCALE;
}

function balanceUnitsFor(asset) {
  if (asset === 'HUSH') return state.hushBalanceUnits;
  return state.balancesUnits[asset];
}

function currentFeeAsset() {
  return state.feeMode === 'hush' ? 'HUSH' : state.activeAsset;
}

function currentBalance() {
  return balanceAmountFor(state.activeAsset);
}

function currentBalanceUnits() {
  return balanceUnitsFor(state.activeAsset);
}

function currentHushBalance() {
  return state.hushBalanceUnits / AMT_SCALE;
}

function currentQuote() {
  const amount = currentAmount();
  if (!wasmReady || amount <= 0) return null;
  const amountUnits = Math.max(1, Math.round(amount * AMT_SCALE));
  const raw = dual_fee_quote_payment_with_schedule_json(
    assetId(state.activeAsset),
    assetId(currentFeeAsset()),
    amountUnits,
    currentFeeScheduleVersion(),
  );
  const response = parseRuntimeResponse(raw);
  return response.ok ? response.data : null;
}

function currentFeeScheduleVersion() {
  const count = state.transactions.length;
  if (count >= 8) return 3;
  if (count >= 3) return 2;
  return 1;
}

function currentFeeScheduleLabel() {
  const version = currentFeeScheduleVersion();
  if (version === 3) return 'Peak';
  if (version === 2) return 'Busy';
  return 'Standard';
}

function currentPaymentDebit(quote = currentQuote()) {
  return quote ? quote.payment_debit / AMT_SCALE : currentAmount();
}

function currentHushDebit(quote = currentQuote()) {
  return quote ? quote.hush_fee_debit / AMT_SCALE : 0;
}

function currentTotalLabel(quote = currentQuote()) {
  if (!quote) return `${fmtMoney(currentAmount())} ${state.activeAsset}`;
  if (quote.hush_fee_debit > 0) {
    return `${fmtMoney(currentAmount())} ${state.activeAsset} + ${fmtFee(currentHushDebit(quote))} HUSH`;
  }
  return `${fmtAssetValue(currentPaymentDebit(quote))} ${state.activeAsset}`;
}

function canSendCurrentPayment() {
  if (!wasmReady || wasmError || state.isSending) return false;
  const recipient = state.currentRecipient.trim();
  const amount = currentAmount();
  const quote = currentQuote();
  if (!recipient || !amount || amount <= 0 || !quote) return false;
  if (quote.payment_debit > currentBalanceUnits()) return false;
  if (quote.hush_fee_debit > state.hushBalanceUnits) return false;
  return true;
}

function parseRuntimeResponse(raw) {
  try {
    return JSON.parse(raw);
  } catch (error) {
    return { ok: false, error: `Invalid runtime response: ${error.message}` };
  }
}

function showToast(message, type = 'info') {
  const toasts = document.getElementById('toasts');
  const toast = document.createElement('div');
  toast.className = `toast ${type}`;
  toast.textContent = message;
  toasts.appendChild(toast);
  setTimeout(() => toast.remove(), 3800);
}

function pushLog(kind, message) {
  state.proofLog.unshift(makeLog(kind, message));
  state.proofLog = state.proofLog.slice(0, 8);
}

function resetProofScope() {
  state.timings = null;
  state.proofOutputs = [];
  state.proofLog = [makeLog('info', 'Waiting for the first payment proof.')];
}

function provenanceStatusLabel() {
  if (state.provenanceStatus === 'revoked') return 'Revoked';
  if (state.provenanceStatus === 'sanctioned') return 'Sanctioned';
  return 'Valid';
}

function currentAssetTransactions() {
  return state.transactions.filter((tx) => tx.asset === state.activeAsset);
}

function refreshSendSummary() {
  const amount = currentAmount();
  const quote = currentQuote();
  const amountEl = document.getElementById('summary-amount');
  const feeEl = document.getElementById('summary-fee');
  const totalEl = document.getElementById('summary-total');
  const routeEl = document.getElementById('summary-route');

  if (amountEl) amountEl.textContent = `${fmtMoney(amount)} ${state.activeAsset}`;
  if (feeEl) {
    feeEl.textContent = quote
      ? `${fmtFee(quote.fee_amount / AMT_SCALE)} ${currentFeeAsset()}`
      : '--';
  }
  if (totalEl) totalEl.textContent = currentTotalLabel(quote);
  if (routeEl) {
    routeEl.textContent = `${state.feeMode === 'hush' ? `${state.activeAsset} -> HUSH` : `${state.activeAsset} -> ${state.activeAsset}`} | ${currentFeeScheduleLabel()}`;
  }
}

// The composer body is the existing payment panel content. Same IDs and same
// onclick handlers, preserving all wiring to sendPayment, updateAmount, etc.
function renderComposerBody() {
  const quote = currentQuote();
  const routeArrowLabel = state.feeMode === 'hush' ? `${state.activeAsset} -> HUSH` : `${state.activeAsset} -> ${state.activeAsset}`;
  return `
    <section class="payment-panel" id="composer">
      <div class="payment-grid">
        <div class="composer-form">
          <div class="field">
            <label for="recipient-input">Recipient</label>
            <input id="recipient-input" type="text" value="${esc(state.currentRecipient)}" placeholder="Recipient name or wallet reference" oninput="updateRecipient(this.value)">
          </div>
          <div class="field">
            <label for="amount-input">Amount</label>
            <input id="amount-input" type="text" value="${esc(state.currentAmountInput)}" inputmode="decimal" placeholder="0.00" oninput="updateAmount(this.value)">
          </div>
          <div class="composer-presets">
            <button class="asset-tab ${currentAmount() === 50 ? 'active' : ''}" onclick="setAmountPreset('50.00')">$50</button>
            <button class="asset-tab ${currentAmount() === 100 ? 'active' : ''}" onclick="setAmountPreset('100.00')">$100</button>
            <button class="asset-tab ${currentAmount() === 500 ? 'active' : ''}" onclick="setAmountPreset('500.00')">$500</button>
            <button class="asset-tab ${currentAmount() === 1000 ? 'active' : ''}" onclick="setAmountPreset('1,000.00')">$1,000</button>
          </div>
          <div class="field">
            <label>Asset</label>
            <div class="asset-tabs">
              <button class="asset-tab ${state.activeAsset === 'USDC' ? 'active' : ''}" onclick="switchAsset('USDC')">USDC</button>
              <button class="asset-tab ${state.activeAsset === 'USDT' ? 'active' : ''}" onclick="switchAsset('USDT')">USDT</button>
              <button class="asset-tab ${state.activeAsset === 'HUSH' ? 'active' : ''}" onclick="switchAsset('HUSH')">HUSH</button>
            </div>
          </div>
          <div class="field">
            <label>Fee</label>
            <div class="asset-tabs composer-route-tabs">
              <button class="asset-tab ${state.feeMode === 'same_asset' ? 'active' : ''}" onclick="switchFeeMode('same_asset')">Pay in ${state.activeAsset}</button>
              ${state.activeAsset === 'HUSH' ? '' : `<button class="asset-tab ${state.feeMode === 'hush' ? 'active' : ''}" onclick="switchFeeMode('hush')">Pay in HUSH</button>`}
            </div>
          </div>
        </div>

        <div class="quote-panel">
          <div class="quote-list">
            <div class="quote-row"><span>Payment</span><strong id="summary-amount">${fmtMoney(currentAmount())} ${state.activeAsset}</strong></div>
            <div class="quote-row"><span>Fee</span><strong id="summary-fee">${quote ? `${fmtFee(quote.fee_amount / AMT_SCALE)} ${currentFeeAsset()}` : '--'}</strong></div>
            <div class="quote-row"><span>Total debit</span><strong id="summary-total">${currentTotalLabel(quote)}</strong></div>
            <div class="quote-row"><span>Route</span><strong id="summary-route">${routeArrowLabel}</strong></div>
          </div>
          <button class="button-primary button-full" onclick="sendPayment()" ${state.isSending ? 'disabled' : ''}>${state.isSending ? 'Generating proof...' : 'Send private payment'}</button>
        </div>
      </div>
    </section>
  `;
}

function renderStage() {
  switch (state.activeView) {
    case 'wallet': {
      const stablesUsd = (state.balancesUnits.USDC + state.balancesUnits.USDT) / AMT_SCALE;
      const hushUsd = (state.hushBalanceUnits / AMT_SCALE) * HUSH_USD_PRICE;
      const headline = stablesUsd + hushUsd;
      return renderBalanceCard(state, { handle: USER_HANDLE, headlineBalance: headline })
        + renderBalancesTable(state, balanceDisplayFor);
    }
    case 'activity': {
      const latestTx = state.successTxId ? getTransaction(state.successTxId) : null;
      const tools = latestTx
        ? `
          <div class="card stub-card" style="display:flex; gap:12px; flex-wrap:wrap;">
            <button class="btn btn-primary" onclick="showReceipt('${esc(latestTx.id)}')">Latest receipt</button>
            <button class="btn btn-ghost" onclick="openAuditModal()">Create audit key</button>
            <button class="btn btn-ghost" onclick="openVerifierFromSuccess('${esc(latestTx.id)}')">Verify receipt</button>
          </div>`
        : `
          <div class="card stub-card" style="display:flex; gap:12px; flex-wrap:wrap; align-items:center;">
            <span style="color:var(--ink-2);">Send a payment from the Wallet view to generate a receipt and audit key.</span>
            <button class="btn btn-ghost" onclick="openAuditModal()" ${state.transactions.length ? '' : 'disabled'}>Create audit key</button>
            <a class="btn btn-ghost" href="/verify.html">Open verifier</a>
          </div>`;
      return tools + `
        <section class="card ledger-panel" id="activity-card">
          <div class="panel-head"><div><div class="panel-kicker">Ledger</div><h2>Payments and receipts</h2></div></div>
          ${renderActivity(state.activity)}
        </section>`;
    }
    case 'compliance':
      return renderComingSoon(
        'Compliance',
        'Audit keys are issued from the Wallet view. Open the modal directly:'
      ) + `<div class="card stub-card"><button class="btn btn-primary" onclick="openAuditModal()">Open audit modal</button></div>`;
    case 'contacts':
      return renderComingSoon('Contacts', 'Contact list coming soon.');
    case 'topup':
      return renderComingSoon('Top up', 'Top up flow coming soon.');
    case 'payout':
      return renderComingSoon('Payout', 'Payout flow coming soon.');
    default:
      return renderBalanceCard(state, { handle: USER_HANDLE, headlineBalance: 2847.50 })
        + renderBalancesTable(state, balanceDisplayFor);
  }
}

function renderTruthOverlayView() {
  const hasPaymentMetrics = Boolean(state.timings);
  const hasAuditMetric = Boolean(state.auditResult);
  const hasAnySessionData = hasPaymentMetrics || hasAuditMetric;

  return `
    <div class="rail-section truth-modal-section">
      ${hasAnySessionData ? `
        <div class="metrics-grid">
          ${hasPaymentMetrics ? `
            <div class="metric-card"><span>Payment prove</span><strong>${state.timings.prove.toFixed(0)}ms</strong></div>
            <div class="metric-card"><span>Payment verify</span><strong>${state.timings.verify.toFixed(0)}ms</strong></div>
            <div class="metric-card"><span>Accounting</span><strong>${state.timings.accounting.toFixed(1)}ms</strong></div>
          ` : ''}
          ${hasAuditMetric ? `
            <div class="metric-card"><span>Audit key</span><strong>${state.auditResult.proveMs.toFixed(0)}ms</strong></div>
          ` : ''}
        </div>
      ` : `
        <div class="session-empty">No proof run yet.</div>
      `}

      <div class="rail-card session-card">
        <div class="truth-block-head">
          <div class="rail-kicker">Latest proof</div>
          <a class="truth-modal-link" href="https://github.com/Hush-Network/stark-demo" target="_blank" rel="noreferrer">Public repo</a>
        </div>
        ${wasmError ? `
          <div class="session-empty" style="border-color:rgba(239,68,68,0.35);color:#fca5a5;">WASM failed to load: ${esc(wasmError.message)}</div>
        ` : state.isSending ? `
          <div class="proving-live">
            <span class="proving-dot"></span>
            <span>Generating STARK proof...</span>
          </div>
        ` : state.timings ? `
          <div class="proof-log-live">
            ${renderProofLog(state.proofLog)}
          </div>
          <details class="rail-details" ${state.proofOutputs.length ? 'open' : ''}>
            <summary>Public outputs</summary>
            <div class="rail-details-body">
              ${renderProofOutputs(state.proofOutputs)}
            </div>
          </details>
        ` : `
          <div class="session-empty">No payment proof yet.</div>
        `}
        <details class="rail-details">
          <summary>Provenance</summary>
          <div class="rail-details-body">
            ${
              state.provenanceProof
                ? `<div class="proof-output" style="margin-bottom:12px;">
                    <div class="proof-output-label">Local proof</div>
                    <div class="proof-output-value">${state.provenanceProof.success ? 'Verified' : 'Failed'}</div>
                    <div class="proof-output-note">${state.provenanceProof.prove_time_ms ? `${state.provenanceProof.prove_time_ms.toFixed(0)}ms` : '--'}</div>
                  </div>`
                : ''
            }
            <div class="sim-controls">
              <button class="sim-button ${state.provenanceStatus === 'valid' ? 'active' : ''}" onclick="setProvenanceStatus('valid')">Valid</button>
              <button class="sim-button ${state.provenanceStatus === 'revoked' ? 'active' : ''}" onclick="setProvenanceStatus('revoked')">Revoked</button>
              <button class="sim-button ${state.provenanceStatus === 'sanctioned' ? 'active' : ''}" onclick="setProvenanceStatus('sanctioned')">Sanctioned</button>
            </div>
          </div>
        </details>
        <details class="rail-details" ${state.lastSubmission ? 'open' : ''}>
          <summary>Payout preview</summary>
          <div class="rail-details-body">
            ${renderPayoutPreview(state.lastSubmission, AMT_SCALE)}
          </div>
        </details>
      </div>
    </div>
  `;
}

window.switchAsset = function switchAsset(asset) {
  state.activeAsset = asset;
  // HUSH same-asset is the only valid HUSH-payment route; the sidecar mode
  // would be (Hush, Hush) which is identical, so force same-asset feeMode.
  if (asset === 'HUSH') state.feeMode = 'same_asset';
  render();
};

window.switchFeeMode = function switchFeeMode(mode) {
  state.feeMode = mode;
  refreshSendSummary();
  render();
};

window.updateRecipient = function updateRecipient(value) {
  state.currentRecipient = value;
};

window.updateAmount = function updateAmount(value) {
  const normalized = sanitizeAmountInput(value);
  state.currentAmountInput = normalized;
  const input = document.getElementById('amount-input');
  if (input && input.value !== normalized) input.value = normalized;
  refreshSendSummary();
};

window.setAmountPreset = function setAmountPreset(value) {
  state.currentAmountInput = value;
  render();
};

window.setProvenanceStatus = function setProvenanceStatus(status) {
  state.provenanceStatus = status;
  render();
};

window.restartDemo = function restartDemo() {
  state.provenanceStatus = 'valid';
  state.activeAsset = 'USDC';
  state.feeMode = 'same_asset';
  state.currentRecipient = DEFAULT_RECIPIENT;
  state.currentAmountInput = DEFAULT_AMOUNT;
  state.balancesUnits = { ...INITIAL_BALANCES_UNITS };
  state.hushBalanceUnits = INITIAL_HUSH_BALANCE_UNITS;
  state.activity = [];
  state.transactions = [];
  state.receiptTxId = null;
  state.successTxId = null;
  state.auditLoading = false;
  state.auditResult = null;
  state.isSending = false;
  state.lastSubmission = null;
  resetProofScope();
  closeOverlay('receipt-overlay');
  closeOverlay('audit-overlay');
  render();
};

window.scrollToTruth = function scrollToTruth() {
  window.openTruthModal();
};

window.openTruthModal = function openTruthModal() {
  const element = document.getElementById('truth-overlay');
  if (element) element.classList.add('show');
};

function closeOverlay(id, event) {
  if (event && event.target !== event.currentTarget) return;
  const element = document.getElementById(id);
  if (element) element.classList.remove('show');
}

window.closeOverlay = closeOverlay;

async function sendPayment() {
  if (!wasmReady || state.isSending) return;

  const recipient = state.currentRecipient.trim();
  const amount = currentAmount();
  const quote = currentQuote();

  if (!recipient) {
    showToast('Choose a recipient before sending.', 'error');
    return;
  }

  if (!amount || amount <= 0) {
    showToast('Enter an amount before sending.', 'error');
    return;
  }

  if (!quote) {
    showToast('Unable to quote this payment route.', 'error');
    return;
  }

  const paymentDebit = currentPaymentDebit(quote);
  const hushDebit = currentHushDebit(quote);

  if (quote.payment_debit > currentBalanceUnits()) {
    showToast('Insufficient balance.', 'error');
    return;
  }

  if (quote.hush_fee_debit > state.hushBalanceUnits) {
    showToast('Insufficient balance.', 'error');
    return;
  }

  state.isSending = true;
  resetProofScope();
  pushLog('info', `${quote.route === 'mode_b_hush_sidecar' ? 'HUSH sidecar' : 'Same-asset fee'}: ${fmtFee(quote.fee_amount / AMT_SCALE)} ${currentFeeAsset()}.`);
  render();

  await new Promise((resolve) => setTimeout(resolve, 80));

  if (state.provenanceStatus === 'revoked') {
    pushLog('error', 'Spend blocked: lineage is in the revocation accumulator.');
    state.isSending = false;
    state.composerOpen = false;
    render();
    showToast('Provenance revoked. Payment blocked before proving.', 'error');
    return;
  }

  try {
    // 'sanctioned' state: simulate in-circuit non-revocation failure by
    // passing an expired CRED_EXPIRY value through the existing WASM API.
    const credExpiry = state.provenanceStatus === 'sanctioned' ? 1 : CRED_EXPIRY;
    const paymentBalanceUnits = currentBalanceUnits();
    const hushBalanceUnits = state.hushBalanceUnits;
    const amountUnits = Math.max(1, Math.round(amount * AMT_SCALE));

    let hash = 0;
    for (let i = 0; i < recipient.length; i += 1) {
      hash = ((hash << 5) - hash + recipient.charCodeAt(i)) | 0;
    }
    const recipientOwner = (Math.abs(hash) % 90_000) + 10_000;

    pushLog('info', 'Generating dual fee payment bundle.');

    const response = parseRuntimeResponse(dual_fee_submit_demo_payment_json(
      assetId(state.activeAsset),
      assetId(currentFeeAsset()),
      amountUnits,
      currentFeeScheduleVersion(),
      recipientOwner,
      paymentBalanceUnits,
      hushBalanceUnits,
      credExpiry,
    ));

    if (!response.ok) {
      pushLog('error', response.error);
      state.isSending = false;
      state.composerOpen = false;
      render();
      showToast(response.error, 'error');
      return;
    }

    const result = response.data;
    const paymentProof = result.payment_proof;
    const accountingStage = result.stage_timings.find((entry) => entry.label === 'accounting');
    const epochCloseStage = result.stage_timings.find((entry) => entry.label === 'epoch_close');

    pushLog('success', `Payment bundle accepted. Prove ${paymentProof.prove_time_ms.toFixed(0)}ms, verify ${paymentProof.verify_time_ms.toFixed(0)}ms.`);
    pushLog('success', `Accounting ${accountingStage.duration_ms.toFixed(2)}ms, epoch close ${epochCloseStage.duration_ms.toFixed(2)}ms.`);
    if (result.hush_sidecar) {
      pushLog('info', 'HUSH sidecar validated.');
    } else {
      pushLog('info', 'Same-asset fee path.');
    }

    state.timings = {
      prove: paymentProof.prove_time_ms,
      verify: paymentProof.verify_time_ms,
      accounting: accountingStage.duration_ms,
      epochClose: epochCloseStage.duration_ms,
    };

    state.proofOutputs = [
      { label: 'null_0', value: fmtHash4(paymentProof.null_0), note: 'First consumed payment note.' },
      { label: 'null_1', value: fmtHash4(paymentProof.null_1), note: 'Second consumed payment note.' },
      { label: 'out_cm_0', value: fmtHash4(paymentProof.out_cm_0), note: 'Committed note for the recipient.' },
      { label: 'out_cm_1', value: fmtHash4(paymentProof.out_cm_1), note: 'Committed sender payment-asset change note.' },
      { label: 'cred_null', value: fmtHash4(paymentProof.cred_null), note: 'Lineage marker for this payment (used by the revocation accumulator).' },
    ];
    if (result.hush_sidecar) {
      state.proofOutputs.push({
        label: 'hush_change_cm',
        value: fmtHash4(result.hush_sidecar.change_cm),
        note: 'Committed sender HUSH change note from the fee sidecar.',
      });
    }

    state.lastSubmission = result;
    if (state.activeAsset === 'HUSH') {
      state.hushBalanceUnits -= quote.payment_debit;
    } else {
      state.balancesUnits[state.activeAsset] -= quote.payment_debit;
    }
    if (quote.hush_fee_debit > 0) {
      state.hushBalanceUnits -= quote.hush_fee_debit;
    }

    const txId = createReceiptId();
    const tx = {
      id: txId,
      recipient,
      amount,
      asset: state.activeAsset,
      feeAmount: quote.fee_amount / AMT_SCALE,
      feeAsset: currentFeeAsset(),
      totalDebited: currentTotalLabel(quote),
      time: new Date(),
      unixTimestamp: Math.floor(Date.now() / 1000),
      receipt: {
        version: 2,
        receipt_id: txId,
        amt_scale: AMT_SCALE,
        amount_units: amountUnits,
        timestamp: new Date().toISOString(),
        recipient,
        asset: state.activeAsset,
        amount,
        proof: {
          null_0: fmtHash4(paymentProof.null_0),
          null_1: fmtHash4(paymentProof.null_1),
          out_cm_0: fmtHash4(paymentProof.out_cm_0),
          out_cm_1: fmtHash4(paymentProof.out_cm_1),
          prove_ms: Math.round(paymentProof.prove_time_ms),
          verify_ms: Math.round(paymentProof.verify_time_ms),
          proof_bytes: paymentProof.proof_bytes,
          note_root: fmtHash4(paymentProof.note_root),
          accumulator_root: fmtHash4(paymentProof.accumulator_root),
          epoch: paymentProof.epoch,
          tx_binding_hash: fmtHash4(paymentProof.tx_binding_hash),
          sender_binding_tag: fmtHash4(paymentProof.sender_binding_tag),
          log_num_rows: paymentProof.log_num_rows,
        },
        binding: {
          replay_domain: result.tx.descriptor.replay_domain,
          payment_asset: result.tx.descriptor.payment_asset,
          fee_asset: result.tx.descriptor.fee_asset,
          fee_class: result.tx.descriptor.fee_class,
          fee_amount: result.tx.descriptor.fee_amount,
          fee_schedule_version: result.tx.descriptor.fee_schedule_version,
          recipient_amount: result.tx.recipient.amount,
          recipient_owner: result.tx.recipient.owner,
          recipient_randomness: result.tx.recipient.randomness,
          sender_change_amount: result.tx.sender_change.amount,
          sender_change_randomness: result.tx.sender_change.randomness,
        },
      },
    };

    state.transactions.unshift(tx);
    state.activity.unshift({
      kind: 'payment',
      icon: 'PAY',
      title: `Sent ${fmtMoney(amount)} ${state.activeAsset}`,
      copy: quote.hush_fee_debit > 0
        ? `${fmtMoney(amount)} ${state.activeAsset} sent. Network fee paid in HUSH.`
        : `${fmtMoney(amount)} ${state.activeAsset} sent.`,
      recipient,
      asset: state.activeAsset,
      amount,
      feeAmount: quote.fee_amount / AMT_SCALE,
      feeAsset: currentFeeAsset(),
      id: tx.id,
      time: tx.time,
    });

    state.successTxId = txId;
    state.receiptTxId = txId;
    state.isSending = false;
    state.composerOpen = false;
    render();
    showToast(`Payment sent to ${recipient}.`, 'success');
  } catch (error) {
    pushLog('error', `Payment proof failed: ${error.message}`);
    state.isSending = false;
    state.composerOpen = false;
    render();
    showToast(`Payment bundle failed: ${error.message}`, 'error');
  }
}

window.sendPayment = sendPayment;

function getTransaction(txId) {
  return state.transactions.find((tx) => tx.id === txId);
}

function buildReceiptPayload(txId) {
  const tx = getTransaction(txId);
  if (!tx) return null;

  const receipt = {
    version: 2,
    receipt_id: tx.receipt.receipt_id,
    amt_scale: AMT_SCALE,
    proof: tx.receipt.proof,
    binding: tx.receipt.binding,
    fee: { amount: tx.feeAmount, asset: tx.feeAsset },
  };

  document.querySelectorAll('#receipt-content [data-field]').forEach((row) => {
    const checkbox = row.querySelector('input[type="checkbox"]');
    if (!checkbox || !checkbox.checked) return;
    const field = row.dataset.field;
    if (field === 'amount') {
      receipt.amount = tx.amount;
      receipt.amount_units = tx.receipt.amount_units;
    }
    if (field === 'timestamp') receipt.timestamp = tx.receipt.timestamp;
    if (field === 'recipient') receipt.recipient = tx.recipient;
    if (field === 'asset') receipt.asset = tx.asset;
    if (field === 'txid') receipt.public_tx_id = tx.id;
    if (field === 'sender') receipt.sender = 'Wallet owner';
    if (field === 'balance') receipt.sender_balance = balanceAmountFor(tx.asset);
  });

  return receipt;
}

window.openVerifierFromSuccess = function openVerifierFromSuccess(txId) {
  state.receiptTxId = txId;
  window.copyReceiptAndVerify();
};

window.showReceipt = function showReceipt(txId) {
  state.receiptTxId = txId;
  const tx = getTransaction(txId);
  if (!tx) return;

  const container = document.getElementById('receipt-content');
  container.innerHTML = `
    <div class="modal-top">
      <div>
        <div class="success-kicker">Selective disclosure</div>
        <h3 class="modal-title">Payment receipt</h3>
        <p class="modal-copy">Choose what to disclose. The proof bytes verify independently of what you include here.</p>
      </div>
      <button class="close-button" onclick="closeOverlay('receipt-overlay')">x</button>
    </div>
    <div class="receipt-list">
      ${renderReceiptRow('amount', 'Amount', `${fmtMoney(tx.amount)} ${tx.asset}`, true)}
      ${renderReceiptRow('timestamp', 'Date and time', tx.receipt.timestamp.replace('T', ' ').slice(0, 19), true)}
      ${renderReceiptRow('recipient', 'Recipient', tx.recipient, false)}
      ${renderReceiptRow('asset', 'Asset', tx.asset, true)}
      ${renderReceiptRow('txid', 'Payment ID', tx.id.slice(0, 10) + '...', false)}
      ${renderReceiptRow('sender', 'Sender', 'Wallet owner', false)}
      ${renderReceiptRow('balance', 'Sender balance', `${fmtAssetValue(balanceAmountFor(tx.asset))} ${tx.asset}`, false)}
    </div>
    <div class="always-hidden">
      <strong>Always hidden</strong>
      <ul>
        <li>Past and future counterparties</li>
        <li>Full wallet history</li>
        <li>Spending key material and private note contents</li>
      </ul>
    </div>
    <div class="modal-actions">
      <button class="button-primary" onclick="copyReceipt()">Copy receipt JSON</button>
      <button class="button-secondary" onclick="copyReceiptAndVerify()">Copy and verify</button>
    </div>
  `;

  document.getElementById('receipt-overlay').classList.add('show');
};

function renderReceiptRow(field, label, value, checked) {
  return `
    <div class="receipt-row" data-field="${esc(field)}">
      <input type="checkbox" ${checked ? 'checked' : ''}>
      <div class="receipt-label">${esc(label)}</div>
      <div class="receipt-value">${esc(value)}</div>
    </div>
  `;
}

window.copyReceipt = async function copyReceipt() {
  const receipt = buildReceiptPayload(state.receiptTxId);
  if (!receipt) return;
  const payload = JSON.stringify(receipt, null, 2);
  try {
    await navigator.clipboard.writeText(payload);
    localStorage.setItem('hush-receipt', payload);
    showToast('Receipt copied to clipboard.', 'success');
    closeOverlay('receipt-overlay');
  } catch {
    showToast('Failed to copy receipt.', 'error');
  }
};

window.copyReceiptAndVerify = async function copyReceiptAndVerify() {
  const receipt = buildReceiptPayload(state.receiptTxId);
  if (!receipt) return;
  const payload = JSON.stringify(receipt, null, 2);
  try {
    await navigator.clipboard.writeText(payload);
  } catch {
    // ignore clipboard failure, still continue with verifier handoff
  }
  localStorage.setItem('hush-receipt', payload);
  window.open('/verify.html', '_blank');
};

window.openAuditModal = function openAuditModal() {
  const txs = currentAssetTransactions();
  if (!txs.length) {
    showToast(`Send a ${state.activeAsset} payment first so the demo has something to summarize.`, 'info');
    return;
  }

  state.auditResult = null;
  const today = new Date().toISOString().split('T')[0];
  const container = document.getElementById('audit-content');
  container.innerHTML = `
    <div class="modal-top">
      <div>
        <h3 class="modal-title">Create audit key</h3>
        <p class="modal-copy">Generate a browser-verified audit key for a chosen period. This is a narrow demo of the time-window flow, not a full reporting product.</p>
      </div>
      <button class="close-button" onclick="closeOverlay('audit-overlay')">x</button>
    </div>
    <div class="audit-grid">
      <div class="audit-field">
        <label for="audit-start">Start date</label>
        <input id="audit-start" type="date" value="${today}">
      </div>
      <div class="audit-field">
        <label for="audit-end">End date</label>
        <input id="audit-end" type="date" value="${today}">
      </div>
    </div>
    <div class="receipt-list">
      ${renderReceiptRow('total_volume', 'Total volume', `${fmtMoney(txs.reduce((sum, tx) => sum + tx.amount, 0))} ${state.activeAsset}`, true, 'Reveal the total amount across the selected period.')}
      ${renderReceiptRow('tx_count', 'Transaction count', String(txs.length), true, 'Reveal how many payments were included.')}
      ${renderReceiptRow('time_period', 'Time period', 'Selected date range', true, 'Reveal the requested audit window.')}
      ${renderReceiptRow('recipients', 'Recipients', 'Optional', false, 'Useful only when counterparties must be disclosed.')}
      ${renderReceiptRow('amounts', 'Individual amounts', 'Optional', false, 'Useful only when line-item payment values must be disclosed.')}
    </div>
    <div class="modal-actions">
      <button class="button-primary" onclick="generateAuditSummary()" ${state.auditLoading ? 'disabled' : ''}>${state.auditLoading ? 'Generating...' : 'Generate audit key'}</button>
    </div>
  `;
  document.getElementById('audit-overlay').classList.add('show');
};

window.generateAuditSummary = async function generateAuditSummary() {
  if (state.auditLoading) return;
  const selected = {};
  document.querySelectorAll('#audit-content [data-field]').forEach((row) => {
    const checkbox = row.querySelector('input[type="checkbox"]');
    selected[row.dataset.field] = !!checkbox?.checked;
  });
  const startDate = document.getElementById('audit-start')?.value;
  const endDate = document.getElementById('audit-end')?.value;

  state.auditLoading = true;
  const trigger = document.querySelector('#audit-content .button-primary');
  if (trigger) {
    trigger.disabled = true;
    trigger.textContent = 'Generating...';
  }

  pushLog('info', 'Generating audit key in the browser.');

  await new Promise((resolve) => setTimeout(resolve, 80));

  try {
    const txs = currentAssetTransactions();
    // Convert display dollars to protocol units (u64 via f64 transport, same as payment circuit).
    const amounts = Float64Array.from(txs.map((tx) => Math.max(1, Math.round(tx.amount * AMT_SCALE))));
    // Use real Unix-second timestamps from transactions (fallback to current time if absent).
    const nowTs = Math.floor(Date.now() / 1000);
    const timestamps = new Uint32Array(txs.map((tx) => tx.unixTimestamp || nowTs));
    // Convert user-selected date range to Unix-second window bounds.
    const startTs = startDate ? Math.floor(new Date(startDate + 'T00:00:00').getTime() / 1000) : timestamps[0] || nowTs;
    const endTs = endDate ? Math.floor(new Date(endDate + 'T23:59:59').getTime() / 1000) : nowTs;
    const result = prove_time_window_audit(startTs, endTs, amounts, timestamps, SK, CRED_ISSUER, CRED_EXPIRY, CRED_SECRET);

    if (!result.success) {
      pushLog('error', result.message);
      state.auditLoading = false;
      if (trigger) {
        trigger.disabled = false;
        trigger.textContent = 'Generate audit key';
      }
      showToast(result.message, 'error');
      return;
    }

    const totalVolume = txs.reduce((sum, tx) => sum + tx.amount, 0);

    state.auditResult = {
      proveMs: result.prove_time_ms,
      verifyMs: result.verify_time_ms,
      totalVolume,
      startDate,
      endDate,
      selected,
      txs,
      proof_bytes: result.proof_bytes,
      window_start: result.window_start,
      window_end: result.window_end,
      claimed_total: result.claimed_total,
      cred_root: result.cred_root,
      cred_null: result.cred_null,
      epoch: result.epoch,
      log_num_rows: result.log_num_rows,
    };

    pushLog('success', `Audit key generated in ${result.prove_time_ms.toFixed(0)}ms, verified in ${result.verify_time_ms.toFixed(0)}ms.`);

    state.activity.unshift({
      kind: 'audit',
      icon: 'AUD',
      title: `Audit key for ${state.activeAsset}`,
      copy: `${txs.length} payment${txs.length !== 1 ? 's' : ''} | ${fmtMoney(totalVolume)} total | ${result.prove_time_ms.toFixed(0)}ms`,
      id: 'audit-' + Date.now(),
      time: new Date(),
    });

    renderAuditResult();
    showToast('Audit key generated.', 'success');
  } catch (error) {
    pushLog('error', `Audit key failed: ${error.message}`);
    showToast(`Audit key failed: ${error.message}`, 'error');
  }

  state.auditLoading = false;
  if (!state.auditResult && trigger) {
    trigger.disabled = false;
    trigger.textContent = 'Generate summary';
  }
};

function renderAuditResult() {
  const result = state.auditResult;
  if (!result) return;

  const disclosed = [];
  if (result.selected.total_volume) disclosed.push(['Total volume', `${fmtMoney(result.totalVolume)} ${state.activeAsset}`]);
  if (result.selected.tx_count) disclosed.push(['Transaction count', String(result.txs.length)]);
  if (result.selected.time_period) disclosed.push(['Period', `${result.startDate} to ${result.endDate}`]);
  if (result.selected.recipients) disclosed.push(['Recipients', result.txs.map((tx) => tx.recipient).join(', ')]);
  if (result.selected.amounts) disclosed.push(['Amounts', result.txs.map((tx) => `${fmtMoney(tx.amount)} ${tx.asset}`).join(' | ')]);

  const hidden = [];
  if (!result.selected.recipients) hidden.push('Counterparty identities');
  if (!result.selected.amounts) hidden.push('Individual payment amounts');
  hidden.push('Spending key material');
  hidden.push('Private note details');

  const container = document.getElementById('audit-content');
  container.innerHTML = `
    <div class="modal-top">
      <div>
        <h3 class="modal-title">Audit key ready</h3>
        <p class="modal-copy">The key covers the selected payment window and exports a verifiable audit payload.</p>
      </div>
      <button class="close-button" onclick="closeOverlay('audit-overlay')">x</button>
    </div>
    <div class="audit-result-block">
      <div class="audit-result-card">
        <h4>Disclosed</h4>
        ${disclosed.map(([label, value]) => `<div class="audit-result-row"><span>${esc(label)}</span><span>${esc(value)}</span></div>`).join('')}
        <div class="audit-result-row"><span>Proof time</span><span>${esc(result.proveMs.toFixed(0))}ms</span></div>
      </div>
      <div class="audit-result-card">
        <h4>Still private</h4>
        ${hidden.map((label) => `<div class="audit-result-row"><span>${esc(label)}</span><span>Hidden</span></div>`).join('')}
      </div>
    </div>
    <div class="modal-actions">
      <button class="button-secondary" onclick="openAuditModal()">New key</button>
      <button class="button-secondary" onclick="copyAuditProof()">Copy JSON</button>
      <button class="button-primary" onclick="closeOverlay('audit-overlay')">Done</button>
    </div>
  `;
}

window.copyAuditProof = async function copyAuditProof() {
  if (!state.auditResult) return;
  const r = state.auditResult;
  const payload = JSON.stringify({
    type: 'hush-audit-key',
    version: 2,
    asset: state.activeAsset,
    amt_scale: AMT_SCALE,
    prove_ms: r.proveMs,
    verify_ms: r.verifyMs,
    window: {
      start_date: r.startDate,
      end_date: r.endDate,
    },
    total_volume: r.totalVolume,
    total_volume_units: r.claimed_total,
    tx_count: r.txs.length,
    disclosed: r.selected,
    proof: {
      proof_bytes: r.proof_bytes,
      window_start: r.window_start,
      window_end: r.window_end,
      claimed_total: r.claimed_total,
      cred_root: r.cred_root,
      cred_null: r.cred_null,
      epoch: r.epoch,
      log_num_rows: r.log_num_rows,
    },
  }, null, 2);
  try {
    await navigator.clipboard.writeText(payload);
    showToast('Audit key copied to clipboard.', 'success');
  } catch {
    showToast('Copy failed.', 'error');
  }
};

function makeLog(kind, message) {
  return { kind, message, time: stamp() };
}

function stamp() {
  return new Date().toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  });
}



// ============================================================================
// Wallet UI globals: handlers wired by walletShell render templates.
// ============================================================================

window.toggleTheme = function toggleTheme() {
  state.theme = state.theme === 'dark' ? 'light' : 'dark';
  render();
};

window.setTheme = function setTheme(theme) {
  state.theme = theme;
  render();
};

window.setActiveView = function setActiveView(viewId) {
  state.activeView = viewId;
  if (viewId === 'compliance') {
    // Compliance view shows a stub plus opens the audit modal
    state.activeView = 'compliance';
  }
  // Auto-close the mobile sidebar drawer after a nav selection
  state.sidebarOpen = false;
  render();
};

window.toggleSidebar = function toggleSidebar() {
  state.sidebarOpen = !state.sidebarOpen;
  render();
};

window.closeSidebar = function closeSidebar() {
  if (state.sidebarOpen) {
    state.sidebarOpen = false;
    render();
  }
};

window.setBalancesTab = function setBalancesTab(tab) {
  state.balancesTab = tab;
  render();
};

window.openComposer = function openComposer() {
  state.composerOpen = true;
  render();
};

window.closeComposerOverlay = function closeComposerOverlay(event) {
  if (event && event.target !== event.currentTarget) return;
  state.composerOpen = false;
  render();
};

window.toggleTweaks = function toggleTweaks() {
  state.tweaksOpen = !state.tweaksOpen;
  render();
};

window.setTweak = function setTweak(key, value) {
  if (key === 'radius' || key === 'fontSize') {
    // Apply continuously without re-rendering. Re-render would re-create the
    // slider element each frame and break the drag interaction.
    const num = Number(value);
    state.tweaks[key] = num;
    if (app) {
      if (key === 'radius') app.style.setProperty('--w-radius', `${num}px`);
      if (key === 'fontSize') app.style.setProperty('--w-size-base', `${num}px`);
    }
    return;
  }
  state.tweaks[key] = value;
  render();
};

window.askComingSoon = function askComingSoon(label) {
  showToast(`${label}: coming soon.`, 'info');
};

window.copyAddress = async function copyAddress() {
  try {
    await navigator.clipboard.writeText('0xf3A2…9c7b');
    showToast('Address copied.', 'success');
  } catch {
    showToast('Copy failed.', 'error');
  }
};
