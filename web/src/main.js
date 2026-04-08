import init, {
  dual_fee_quote_payment_json,
  dual_fee_submit_demo_payment_json,
  prove_time_window_audit,
} from '../pkg/hush_demo_stark.js';

// 1 protocol unit = $0.0001 (4 decimal places). Circuit uses four-limb radix-2^15 encoding
// with u64 amounts (max ~$115 trillion per note). Display dollars * AMT_SCALE = protocol units.
const AMT_SCALE = 10_000;

const SK = 12345;
const CRED_ISSUER = 1;
const CRED_EXPIRY = 50_000;
const CRED_SECRET = 777;

function esc(s) {
  if (s == null) return '';
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

// Format a [u32; 4] array as a 0x-prefixed 32-char hex string (4 x 8 hex digits).
function fmtHash4(arr) {
  if (!Array.isArray(arr) || arr.length !== 4) return '0x' + String(arr);
  return '0x' + arr.map(v => (v >>> 0).toString(16).padStart(8, '0')).join('');
}

const DEFAULT_SETUP_METHOD = 'Device key';
const DEFAULT_RECIPIENT = 'Meridian Labs';
const DEFAULT_AMOUNT = '125,000.00';

let wasmReady = false;

const state = {
  screen: 'welcome',
  setupMethod: DEFAULT_SETUP_METHOD,
  credentialStatus: 'valid',
  activeAsset: 'USDC',
  feeMode: 'same_asset',
  currentRecipient: DEFAULT_RECIPIENT,
  currentAmountInput: DEFAULT_AMOUNT,
  balances: { USDC: 1_500_000, USDT: 500_000 },
  hushBalance: 250,
  activity: [],
  transactions: [],
  proofLog: [
    makeLog('info', 'Waiting for the first payment proof.'),
  ],
  proofOutputs: [],
  timings: null,
  isSending: false,
  isActivatingCredential: false,
  receiptTxId: null,
  successTxId: null,
  auditLoading: false,
  auditResult: null,
  walletSeeded: false,
  tourOpen: false,
  tourStep: 0,
  lastSubmission: null,
};

const TOUR_STEPS = [
  {
    target: '#wallet-balance-card',
    title: 'Wallet home',
    copy: 'This is the intended HushPay wallet view: stablecoin balance first, credential status visible, and no need to manage HUSH in the primary flow.',
  },
  {
    target: '#send-card',
    title: 'Amount, fee, total',
    copy: 'The sender sees amount, fee route, and total before sending. Both same-asset fees and the HUSH sidecar route run through the dual fee backend.',
  },
  {
    target: '#activity-card',
    title: 'Receipts and verification',
    copy: 'Payments can later generate receipts for merchants, auditors, or counterparties. The wallet keeps disclosure narrow instead of exposing the full payment graph.',
  },
  {
    target: '#truth-card',
    title: 'Technical details',
    copy: 'This panel separates what the backend already supports from what is still local demo scaffolding or not implemented yet.',
  },
];

const splash = document.getElementById('splash');
const app = document.getElementById('app');
const stage = document.getElementById('stage');
const rail = document.getElementById('rail');

async function boot() {
  const minimumSplash = new Promise(resolve => setTimeout(resolve, 1200));
  await Promise.all([init(), minimumSplash]);
  wasmReady = true;
  render();
  splash.classList.add('hidden');
  app.classList.add('visible');
}

boot().catch((error) => {
  console.error('WASM init failed:', error);
  const copy = document.querySelector('.splash-copy');
  if (copy) copy.textContent = `Failed to load prover: ${error.message}`;
});

function render() {
  stage.innerHTML = renderStage();
  rail.innerHTML = renderRail();

  if (state.screen === 'wallet') {
    const amountInput = document.getElementById('amount-input');
    const recipientInput = document.getElementById('recipient-input');
    if (amountInput) amountInput.value = state.currentAmountInput;
    if (recipientInput) recipientInput.value = state.currentRecipient;
    refreshSendSummary();
  }

  if (state.tourOpen && state.screen === 'wallet') {
    requestAnimationFrame(renderTourStep);
  }
}

function fmtMoney(value) {
  return value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function fmtAssetValue(value) {
  return value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 3 });
}

function fmtFee(value) {
  return value.toLocaleString('en-US', { minimumFractionDigits: 4, maximumFractionDigits: 4 });
}

function relativeTime(date) {
  const diff = Math.max(0, Math.floor((Date.now() - date.getTime()) / 1000));
  if (diff < 60) return 'just now';
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

function sanitizeAmountInput(raw) {
  const clean = raw.replace(/[^0-9.]/g, '');
  const parts = clean.split('.');
  const whole = parts[0] || '0';
  const decimals = parts[1] ? parts[1].slice(0, 2) : '';
  const formattedWhole = Number.parseInt(whole, 10).toLocaleString('en-US');
  return decimals.length ? `${formattedWhole}.${decimals}` : formattedWhole;
}

function parseAmountInput(value) {
  const parsed = Number.parseFloat(value.replace(/,/g, ''));
  return Number.isFinite(parsed) ? parsed : 0;
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

function currentFeeAsset() {
  return state.feeMode === 'hush' ? 'HUSH' : state.activeAsset;
}

function currentBalance() {
  return state.balances[state.activeAsset];
}

function currentQuote() {
  const amount = currentAmount();
  if (!wasmReady || amount <= 0) return null;
  const amountUnits = Math.max(1, Math.round(amount * AMT_SCALE));
  const raw = dual_fee_quote_payment_json(
    assetId(state.activeAsset),
    assetId(currentFeeAsset()),
    amountUnits,
  );
  const response = parseRuntimeResponse(raw);
  return response.ok ? response.data : null;
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

function refreshSendSummary() {
  const amount = currentAmount();
  const quote = currentQuote();
  const amountEl = document.getElementById('summary-amount');
  const feeEl = document.getElementById('summary-fee');
  const totalEl = document.getElementById('summary-total');
  const exactEl = document.getElementById('summary-delivery');
  if (amountEl) amountEl.textContent = `${fmtMoney(amount)} ${state.activeAsset}`;
  if (feeEl) {
    feeEl.textContent = quote
      ? `${fmtFee(quote.fee_amount / AMT_SCALE)} ${currentFeeAsset()}`
      : '--';
  }
  if (totalEl) totalEl.textContent = currentTotalLabel(quote);
  if (exactEl) exactEl.textContent = `Receiver gets the full amount: ${fmtMoney(amount)} ${state.activeAsset}.`;
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

function seedWalletActivity() {
  if (state.walletSeeded) return;
  state.activity = [];
  state.walletSeeded = true;
}

function credentialDescription() {
  if (state.credentialStatus === 'revoked') {
    return 'The demo wallet blocks the payment before proving because the issuer registry marks the credential as revoked.';
  }
  if (state.credentialStatus === 'expired') {
    return 'Expiry is enforced inside the payment proof. The wallet can try to send, but the proof will fail.';
  }
  return 'The wallet can generate a payment proof because the credential is active and eligible for network participation.';
}

function createReceiptId() {
  const bytes = new Uint8Array(8);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('');
}

function currentAssetTransactions() {
  return state.transactions.filter((tx) => tx.asset === state.activeAsset);
}

function renderStage() {
  if (state.screen === 'welcome') {
    return `
      <section class="hero-shell">
        <div class="hero-card">
          <div class="eyebrow">HushPay demo</div>
          <h1 class="hero-title">Private stablecoin payments with a real wallet flow.</h1>
          <p class="hero-copy">This demo shows the intended HushPay experience: guided setup, credential activation, amount-plus-fee payments, and receipts only when verification is required.</p>
          <div class="hero-note">Demo mode simulates wallet activation and credential issuance. It does not require email signup, wallet connection, passkeys, or document upload.</div>
          <div class="hero-actions">
            <button class="button-primary" onclick="startDemo()">Start demo</button>
          </div>
        </div>
      </section>
    `;
  }

  if (state.screen === 'setup') {
    return `
      <section class="hero-shell">
        <div class="flow-card">
          <div class="step-indicator">
            <div class="step-badge">1</div>
            <div class="step-label">Wallet setup</div>
          </div>
          <h2 class="flow-title">Choose how this demo wallet is secured.</h2>
          <p class="flow-copy">HushPay should feel like wallet activation, not software signup. Demo mode follows one curated path while still showing the choices a real wallet could offer.</p>
          <div class="setup-grid">
            ${renderSetupCard('Device key', 'Demo path. The wallet is secured locally on this device and ready to activate in one flow.')}
            ${renderSetupCard('Hardware signer', 'Shown as an option for higher-security setups, but not required in this demo.')}
            ${renderSetupCard('Recovery contact', 'A guided recovery path for managed deployments without turning the wallet into account software.')}
          </div>
          <div class="info-strip">
            <strong>Curated demo path</strong>
            <span>This walkthrough continues with <strong>${state.setupMethod}</strong> so first-time users stay focused on the payment product.</span>
          </div>
          <div class="flow-actions">
            <button class="button-primary" onclick="continueFromSetup()">Continue</button>
            <button class="button-ghost" onclick="restartDemo()">Start over</button>
          </div>
        </div>
      </section>
    `;
  }

  if (state.screen === 'credential') {
    return `
      <section class="hero-shell">
        <div class="flow-card">
          <div class="step-indicator">
            <div class="step-badge">2</div>
            <div class="step-label">Eligibility activation</div>
          </div>
          <h2 class="flow-title">Activate the wallet before it can send.</h2>
          <p class="flow-copy">Hush requires an eligibility credential before a wallet can move value. The credential proves the wallet is allowed on the network without writing identity onto the chain.</p>
          <div class="info-strip">
            <strong>Demo issuer path</strong>
            <span>This step simulates an approved issuer activating the wallet. The product goal is eligibility proof without exposing who the user is to the network.</span>
          </div>
          <div class="flow-actions">
            <button class="button-primary" onclick="activateCredential()" ${state.isActivatingCredential ? 'disabled' : ''}>${state.isActivatingCredential ? 'Issuing demo credential...' : 'Issue demo credential'}</button>
            <button class="button-ghost" onclick="restartDemo()">Start over</button>
          </div>
        </div>
      </section>
    `;
  }

  if (state.screen === 'ready') {
    return `
      <section class="hero-shell">
        <div class="flow-card">
          <div class="step-indicator">
            <div class="step-badge">3</div>
            <div class="step-label">Wallet ready</div>
          </div>
          <h2 class="flow-title">HushPay is active.</h2>
          <p class="flow-copy">The wallet is set up, the eligibility credential is active, and the guided payment experience is ready. From here the product should feel like a real private payment wallet, not a proving sandbox.</p>
          <div class="info-strip">
            <strong>What comes next</strong>
            <span>You will land in the wallet home, send a private stablecoin payment, and generate a receipt only if someone needs verification.</span>
          </div>
          <div class="flow-actions">
            <button class="button-primary" onclick="openWallet()">Open wallet</button>
          </div>
        </div>
      </section>
    `;
  }

  return `
    <section class="wallet-grid">
      <div class="wallet-column wallet-main-column">
        <div class="wallet-card balance-card wallet-overview-card" id="wallet-balance-card">
          <div class="overview-header">
            <div>
              <div class="balance-amount">$${fmtAssetValue(currentBalance())}</div>
            </div>
            <div class="status-pill">Credential active</div>
          </div>
          <div class="overview-controls compact-overview-controls">
            <div class="asset-tabs">
              <button class="asset-tab ${state.activeAsset === 'USDC' ? 'active' : ''}" onclick="switchAsset('USDC')">USDC</button>
              <button class="asset-tab ${state.activeAsset === 'USDT' ? 'active' : ''}" onclick="switchAsset('USDT')">USDT</button>
            </div>
          </div>
        </div>

        <div class="wallet-card send-composer-card" id="send-card">
          <div class="section-head">
            <h3>Send private payment</h3>
          </div>
          <div class="send-layout">
            <div class="send-form-column">
              <div class="field">
                <label for="recipient-input">Recipient</label>
                <input id="recipient-input" type="text" value="${esc(state.currentRecipient)}" placeholder="Recipient name or wallet reference" oninput="updateRecipient(this.value)">
              </div>
              <div class="field">
                <label for="amount-input">Amount</label>
                <input id="amount-input" type="text" value="${esc(state.currentAmountInput)}" inputmode="decimal" placeholder="0.00" oninput="updateAmount(this.value)">
              </div>
            </div>
            <div class="send-summary-panel">
              <div class="summary-panel-head">
                <div class="summary-panel-kicker">Payment preview</div>
              </div>
              <div class="fee-box">
                <div class="fee-line"><span>Payment amount</span><strong id="summary-amount">${fmtMoney(currentAmount())} ${state.activeAsset}</strong></div>
                <div class="fee-line"><span>Network fee</span><strong id="summary-fee">--</strong></div>
                <div class="fee-line"><span>Total debited</span><strong id="summary-total">${currentTotalLabel()}</strong></div>
                <div class="delivery-line" id="summary-delivery">Receiver gets the full amount: ${fmtMoney(currentAmount())} ${state.activeAsset}.</div>
              </div>
              <div class="send-actions send-actions-stacked">
                <button class="button-primary button-full" onclick="sendPayment()" ${state.isSending || !wasmReady ? 'disabled' : ''}>${state.isSending ? 'Generating payment proof...' : 'Send payment'}</button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="wallet-column wallet-side-column">
        <div class="wallet-card utility-stack-card" id="activity-card">
          <div class="section-head">
            <h3>Activity</h3>
          </div>
          <div class="activity-list">
            ${renderActivity()}
          </div>
          <div class="utility-divider"></div>
          <div class="audit-footer">
            <div class="audit-footer-text">
              <strong>Time-window audit</strong>
              <span>Prove a payment range for compliance without revealing individual amounts.</span>
            </div>
            <button class="button-secondary button-sm" onclick="openAuditModal()" ${state.transactions.length ? '' : 'disabled'}>Create proof</button>
          </div>
        </div>
      </div>
    </section>
  `;
}

function renderSetupCard(title, copy) {
  const active = state.setupMethod === title ? 'active' : '';
  return `
    <button class="option-card ${active}" onclick="chooseSetupMethod('${title}')">
      <div class="option-title">${title}</div>
      <div class="option-copy">${copy}</div>
    </button>
  `;
}

function renderActivity() {
  if (!state.activity.length) {
    return '<div class="empty-copy">No payments yet. Send one to create a receipt or audit summary.</div>';
  }

  return state.activity.map((item) => {
    const meta = [];
    const badgeClass = item.kind === 'payment' ? 'payment' : item.kind === 'audit' ? 'audit' : 'system';
    if (item.asset) meta.push(`${esc(fmtMoney(item.amount))} ${esc(item.asset)}`);
    if (item.feeAmount != null) meta.push(`fee ${esc(fmtFee(item.feeAmount))} ${esc(item.feeAsset)}`);
    if (item.kind === 'payment') meta.push('<a class="activity-link" onclick="showReceipt(\'' + esc(item.id) + '\')">Receipt</a>');
    if (item.kind === 'audit') meta.push('<a class="activity-link" onclick="renderAuditResult(); document.getElementById(\'audit-overlay\').classList.add(\'show\')">View proof</a>');
    return `
      <div class="activity-item">
        <div class="activity-badge ${esc(badgeClass)}">${esc(item.icon)}</div>
        <div class="activity-main">
          <div class="activity-title-row">
            <div class="activity-title">${esc(item.title)}</div>
            <div class="activity-time">${esc(relativeTime(item.time))}</div>
          </div>
          <div class="activity-copy">${esc(item.copy)}</div>
          <div class="activity-meta">${meta.join('<span>•</span>')}</div>
        </div>
      </div>
    `;
  }).join('');
}

function renderRail() {
  if (state.screen !== 'wallet') {
    return `
      <div class="rail-card">
        <div class="rail-kicker">Guided flow</div>
        <h3>What this demo is doing</h3>
        <p>Setup and credential activation are simulated here so the walkthrough stays focused on the wallet experience.</p>
        <div class="rail-list">
          <div class="rail-item"><strong>Wallet direction</strong><span>Stablecoin-first payments, amount plus fee shown up front, receipts only when needed.</span></div>
          <div class="rail-item"><strong>Verified today</strong><span>Same-asset fee payments, HUSH sidecar fee payments, receipt verification, local accounting, and audit proofs.</span></div>
          <div class="rail-item"><strong>Still simulated</strong><span>Setup, credential activation, wallet balances, and the live network path.</span></div>
        </div>
      </div>
    `;
  }

  return `
    <div class="rail-card session-card" id="truth-card">
      <div class="rail-kicker">Proof activity</div>
      ${state.isSending ? `
        <div class="proving-live">
          <span class="proving-dot"></span>
          <span>Generating STARK proof…</span>
        </div>
      ` : state.timings ? `
        <div class="proof-timing-line">
          <span>${state.timings.prove.toFixed(0)}ms prove</span>
          <span class="timing-sep">·</span>
          <span>${state.timings.verify.toFixed(0)}ms verify</span>
          <span class="timing-sep">·</span>
          <span>${state.timings.accounting.toFixed(1)}ms accounting</span>
        </div>
      ` : `
        <div class="session-empty">Send a payment to see the prover run.</div>
      `}
      <div class="proof-log-live">
        ${renderProofLog()}
      </div>
      <details class="rail-details">
        <summary>Credential simulation</summary>
        <div class="rail-details-body">
          <p>${credentialDescription()}</p>
          <div class="sim-controls">
            <button class="sim-button ${state.credentialStatus === 'valid' ? 'active' : ''}" onclick="setCredentialStatus('valid')">Valid</button>
            <button class="sim-button ${state.credentialStatus === 'revoked' ? 'active' : ''}" onclick="setCredentialStatus('revoked')">Revoked</button>
            <button class="sim-button ${state.credentialStatus === 'expired' ? 'active' : ''}" onclick="setCredentialStatus('expired')">Expired</button>
          </div>
        </div>
      </details>
      <details class="rail-details" ${state.lastSubmission ? 'open' : ''}>
        <summary>Payout preview</summary>
        <div class="rail-details-body">
          ${renderPayoutPreview()}
        </div>
      </details>
    </div>
  `;
}

function renderProofLog() {
  return state.proofLog.map((entry) => `
    <div class="proof-entry ${entry.kind}">
      <div class="proof-entry-top">
        <span class="proof-entry-kind">${entry.kind === 'success' ? 'Success' : entry.kind === 'error' ? 'Blocked or failed' : 'Info'}</span>
        <span class="proof-entry-time">${entry.time}</span>
      </div>
      <div class="proof-entry-message">${entry.message}</div>
    </div>
  `).join('');
}

function renderProofOutputs() {
  if (!state.proofOutputs.length) {
    return '<p class="empty-copy">No public proof outputs yet.</p>';
  }

  return state.proofOutputs.map((output) => `
    <div class="proof-output">
      <div class="proof-output-label">${output.label}</div>
      <div class="proof-output-value">${output.value}</div>
      <div class="proof-output-note">${output.note}</div>
    </div>
  `).join('');
}

function renderPayoutPreview() {
  const payoutRecords = state.lastSubmission?.payout_inspection?.payout_records || [];
  if (!payoutRecords.length) {
    return '<p class="empty-copy">No validator payout records yet.</p>';
  }

  return payoutRecords.map((record) => `
    <div class="proof-output">
      <div class="proof-output-label">Validator ${record.validator_id}</div>
      <div class="proof-output-value">Key ${record.payout_key}</div>
      <div class="proof-output-note">HUSH ${fmtFee(record.entitlement.hush / AMT_SCALE)} • USDC ${fmtFee(record.entitlement.usdc / AMT_SCALE)} • USDT ${fmtFee(record.entitlement.usdt / AMT_SCALE)}</div>
    </div>
  `).join('');
}

window.startDemo = function startDemo() {
  state.screen = 'setup';
  render();
};

window.chooseSetupMethod = function chooseSetupMethod(method) {
  state.setupMethod = method;
  render();
};

window.continueFromSetup = function continueFromSetup() {
  state.screen = 'credential';
  render();
};

window.activateCredential = async function activateCredential() {
  state.isActivatingCredential = true;
  render();
  await new Promise((resolve) => setTimeout(resolve, 900));
  state.isActivatingCredential = false;
  state.screen = 'ready';
  render();
};

window.openWallet = function openWallet() {
  state.screen = 'wallet';
  seedWalletActivity();
  render();
  setTimeout(() => {
    if (!sessionStorage.getItem('hushpay-tour-seen')) {
      state.tourOpen = true;
      state.tourStep = 0;
      renderTourStep();
      document.getElementById('tour-overlay').classList.add('show');
    }
  }, 200);
};

window.switchAsset = function switchAsset(asset) {
  state.activeAsset = asset;
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

window.setCredentialStatus = function setCredentialStatus(status) {
  state.credentialStatus = status;
  render();
};

window.restartDemo = function restartDemo() {
  state.screen = 'welcome';
  state.setupMethod = DEFAULT_SETUP_METHOD;
  state.credentialStatus = 'valid';
  state.activeAsset = 'USDC';
  state.feeMode = 'same_asset';
  state.currentRecipient = DEFAULT_RECIPIENT;
  state.currentAmountInput = DEFAULT_AMOUNT;
  state.balances = { USDC: 2_500_000, USDT: 600_000 };
  state.hushBalance = 250;
  state.activity = [];
  state.transactions = [];
  state.walletSeeded = false;
  state.receiptTxId = null;
  state.successTxId = null;
  state.auditLoading = false;
  state.auditResult = null;
  state.tourOpen = false;
  state.tourStep = 0;
  state.isSending = false;
  state.isActivatingCredential = false;
  state.lastSubmission = null;
  resetProofScope();
  document.getElementById('tour-overlay').classList.remove('show');
  closeOverlay('success-overlay');
  closeOverlay('receipt-overlay');
  closeOverlay('audit-overlay');
  render();
};

window.scrollToTruth = function scrollToTruth() {
  const truthCard = document.getElementById('truth-card');
  if (!truthCard) return;
  truthCard.scrollIntoView({ behavior: 'smooth', block: 'start' });
};

window.openVerifier = function openVerifier() {
  window.open('/verify.html', '_blank');
};

window.toggleTour = function toggleTour() {
  if (state.screen !== 'wallet') {
    showToast('Open the wallet first to start the guided tour.', 'info');
    return;
  }
  state.tourOpen = true;
  state.tourStep = 0;
  document.getElementById('tour-overlay').classList.add('show');
  renderTourStep();
};

window.endTour = function endTour() {
  state.tourOpen = false;
  document.getElementById('tour-overlay').classList.remove('show');
  sessionStorage.setItem('hushpay-tour-seen', '1');
};

window.nextTourStep = function nextTourStep() {
  state.tourStep += 1;
  if (state.tourStep >= TOUR_STEPS.length) {
    window.endTour();
    return;
  }
  renderTourStep();
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

  if (paymentDebit > currentBalance()) {
    showToast('Insufficient payment-asset balance for the selected route.', 'error');
    return;
  }

  if (hushDebit > state.hushBalance) {
    showToast('Insufficient HUSH balance for the sidecar fee path.', 'error');
    return;
  }

  state.isSending = true;
  resetProofScope();
  pushLog('info', `${quote.route === 'mode_b_hush_sidecar' ? 'HUSH sidecar' : 'Same-asset fee'}: ${fmtFee(quote.fee_amount / AMT_SCALE)} ${currentFeeAsset()}.`);
  render();

  await new Promise((resolve) => setTimeout(resolve, 80));

  if (state.credentialStatus === 'revoked') {
    pushLog('error', 'Credential blocked at the wallet layer before proving.');
    state.isSending = false;
    render();
    showToast('Credential revoked. Payment blocked before proving.', 'error');
    return;
  }

  try {
    const credExpiry = state.credentialStatus === 'expired' ? 1 : CRED_EXPIRY;
    const paymentBalanceUnits = Math.round(currentBalance() * AMT_SCALE);
    const hushBalanceUnits = Math.round(state.hushBalance * AMT_SCALE);
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
      recipientOwner,
      paymentBalanceUnits,
      hushBalanceUnits,
      credExpiry,
    ));

    if (!response.ok) {
      pushLog('error', response.error);
      state.isSending = false;
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
      { label: 'cred_null', value: fmtHash4(paymentProof.cred_null), note: 'Credential nullifier for this payment.' },
    ];
    if (result.hush_sidecar) {
      state.proofOutputs.push({
        label: 'hush_change_cm',
        value: fmtHash4(result.hush_sidecar.change_cm),
        note: 'Committed sender HUSH change note from the fee sidecar.',
      });
    }

    state.lastSubmission = result;
    state.balances[state.activeAsset] -= paymentDebit;
    if (quote.hush_fee_debit > 0) {
      state.hushBalance -= hushDebit;
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
        timestamp: new Date().toISOString(),
        recipient,
        asset: state.activeAsset,
        amount,
        proof: {
          null_0: fmtHash4(paymentProof.null_0),
          null_1: fmtHash4(paymentProof.null_1),
          out_cm_0: fmtHash4(paymentProof.out_cm_0),
          out_cm_1: fmtHash4(paymentProof.out_cm_1),
          cred_null: fmtHash4(paymentProof.cred_null),
          prove_ms: Math.round(paymentProof.prove_time_ms),
          verify_ms: Math.round(paymentProof.verify_time_ms),
          proof_bytes: paymentProof.proof_bytes,
          note_root: fmtHash4(paymentProof.note_root),
          cred_root: fmtHash4(paymentProof.cred_root),
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
      icon: '↗',
      title: `Sent ${fmtMoney(amount)} ${state.activeAsset}`,
      copy: quote.hush_fee_debit > 0
        ? `Receiver gets ${fmtMoney(amount)} ${state.activeAsset}. Sender paid the fee in HUSH.`
        : `Receiver gets the full amount: ${fmtMoney(amount)} ${state.activeAsset}.`,
      asset: state.activeAsset,
      amount,
      feeAmount: quote.fee_amount / AMT_SCALE,
      feeAsset: currentFeeAsset(),
      id: tx.id,
      time: tx.time,
    });

    state.successTxId = txId;
    state.receiptTxId = txId;
    state.currentAmountInput = DEFAULT_AMOUNT;
    state.isSending = false;
    render();
    showToast(`Payment sent to ${recipient}.`, 'success');
  } catch (error) {
    pushLog('error', `Payment proof failed: ${error.message}`);
    state.isSending = false;
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
    if (field === 'amount') receipt.amount = tx.amount;
    if (field === 'timestamp') receipt.timestamp = tx.receipt.timestamp;
    if (field === 'recipient') receipt.recipient = tx.recipient;
    if (field === 'asset') receipt.asset = tx.asset;
    if (field === 'txid') receipt.public_tx_id = tx.id;
    if (field === 'sender') receipt.sender = 'Wallet owner';
    if (field === 'balance') receipt.sender_balance = state.balances[tx.asset];
  });

  return receipt;
}

function openSuccessOverlay(txId) {
  const tx = getTransaction(txId);
  if (!tx) return;
  const container = document.getElementById('success-content');
  container.innerHTML = `
    <div class="success-kicker">Payment complete</div>
    <div class="modal-top">
      <div>
        <h3 class="modal-title">Payment sent.</h3>
        <p class="modal-copy">The wallet flow shows the intended HushPay experience: the sender pays amount plus fee, and the receiver gets the full amount.</p>
      </div>
      <button class="close-button" onclick="closeOverlay('success-overlay')">×</button>
    </div>
    <div class="success-summary">
      <div class="success-row"><span>Recipient</span><span>${esc(tx.recipient)}</span></div>
      <div class="success-row"><span>Amount</span><span>${esc(fmtMoney(tx.amount))} ${esc(tx.asset)}</span></div>
      <div class="success-row"><span>Network fee</span><span>${esc(fmtFee(tx.feeAmount))} ${esc(tx.feeAsset)}</span></div>
      <div class="success-row"><span>Total debited</span><span>${esc(tx.totalDebited)}</span></div>
    </div>
    <div class="modal-actions">
      <button class="button-primary" onclick="openReceiptFromSuccess('${esc(tx.id)}')">View receipt</button>
      <button class="button-secondary" onclick="scrollFromSuccess()">Open technical details</button>
    </div>
  `;
  document.getElementById('success-overlay').classList.add('show');
}

window.openReceiptFromSuccess = function openReceiptFromSuccess(txId) {
  closeOverlay('success-overlay');
  showReceipt(txId);
};

window.scrollFromSuccess = function scrollFromSuccess() {
  closeOverlay('success-overlay');
  scrollToTruth();
};

window.showReceipt = function showReceipt(txId) {
  state.receiptTxId = txId;
  const tx = getTransaction(txId);
  if (!tx) return;

  const container = document.getElementById('receipt-content');
  container.innerHTML = `
    <div class="modal-top">
      <div>
        <h3 class="modal-title">Payment receipt</h3>
        <p class="modal-copy">Choose what to disclose. The proof bytes verify independently of what you include here.</p>
      </div>
      <button class="close-button" onclick="closeOverlay('receipt-overlay')">×</button>
    </div>
    <div class="receipt-list">
      ${renderReceiptRow('amount', 'Amount', `${fmtMoney(tx.amount)} ${tx.asset}`, true)}
      ${renderReceiptRow('timestamp', 'Date and time', tx.receipt.timestamp.replace('T', ' ').slice(0, 19), true)}
      ${renderReceiptRow('recipient', 'Recipient', tx.recipient, false)}
      ${renderReceiptRow('asset', 'Asset', tx.asset, true)}
      ${renderReceiptRow('txid', 'Transaction reference', tx.id.slice(0, 10) + '…', false)}
      ${renderReceiptRow('sender', 'Sender', 'Wallet owner', false)}
      ${renderReceiptRow('balance', 'Sender balance', `${fmtAssetValue(state.balances[tx.asset])} ${tx.asset}`, false)}
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
        <h3 class="modal-title">Create audit summary</h3>
        <p class="modal-copy">Generate a browser-verified summary for a chosen period. This is a narrow demo of the time-window flow, not a full reporting product.</p>
      </div>
      <button class="close-button" onclick="closeOverlay('audit-overlay')">×</button>
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
      <button class="button-primary" onclick="generateAuditSummary()" ${state.auditLoading ? 'disabled' : ''}>${state.auditLoading ? 'Generating...' : 'Generate summary'}</button>
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

  pushLog('info', 'Generating time-window audit proof in the browser.');

  await new Promise((resolve) => setTimeout(resolve, 80));

  try {
    const txs = currentAssetTransactions();
    // Convert display dollars to protocol units (u32). AMT_SCALE maps dollars to protocol units.
    const amounts = new Uint32Array(txs.map((tx) => {
      const units = Math.round(tx.amount * AMT_SCALE);
      if (units > 0xFFFFFFFF) throw new Error('Amount exceeds u32 at current AMT_SCALE');
      return Math.max(1, units);
    }));
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
        trigger.textContent = 'Generate summary';
      }
      showToast(result.message, 'error');
      return;
    }

    const totalVolume = txs.reduce((sum, tx) => sum + tx.amount, 0);

    state.auditResult = {
      proveMs: result.prove_time_ms,
      totalVolume,
      startDate,
      endDate,
      selected,
      txs,
    };

    pushLog('success', `Time-window audit proof generated in ${result.prove_time_ms.toFixed(0)}ms.`);

    state.activity.unshift({
      kind: 'audit',
      icon: '⊞',
      title: `Audit proof — ${state.activeAsset}`,
      copy: `${txs.length} payment${txs.length !== 1 ? 's' : ''} · ${fmtMoney(totalVolume)} total · ${result.prove_time_ms.toFixed(0)}ms`,
      id: 'audit-' + Date.now(),
      time: new Date(),
    });

    renderAuditResult();
    showToast('Audit proof generated.', 'success');
  } catch (error) {
    pushLog('error', `Audit proof failed: ${error.message}`);
    showToast(`Audit proof failed: ${error.message}`, 'error');
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
  if (result.selected.amounts) disclosed.push(['Amounts', result.txs.map((tx) => `${fmtMoney(tx.amount)} ${tx.asset}`).join(' · ')]);

  const hidden = [];
  if (!result.selected.recipients) hidden.push('Counterparty identities');
  if (!result.selected.amounts) hidden.push('Individual payment amounts');
  hidden.push('Spending key material');
  hidden.push('Private note details');

  const container = document.getElementById('audit-content');
  container.innerHTML = `
    <div class="modal-top">
      <div>
        <h3 class="modal-title">Audit summary ready</h3>
        <p class="modal-copy">The proof covers the selected payment window. The summary below shows only the fields chosen for disclosure.</p>
        <p class="modal-copy" style="opacity:0.6;font-size:13px">A ZK proof was generated and verified locally. Standalone proof export is not yet implemented.</p>
      </div>
      <button class="close-button" onclick="closeOverlay('audit-overlay')">×</button>
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
      <button class="button-secondary" onclick="openAuditModal()">New proof</button>
      <button class="button-secondary" onclick="copyAuditProof()">Copy JSON</button>
      <button class="button-primary" onclick="closeOverlay('audit-overlay')">Done</button>
    </div>
  `;
}

window.copyAuditProof = async function copyAuditProof() {
  if (!state.auditResult) return;
  const payload = JSON.stringify({
    type: 'hush-audit-disclosure',
    version: 1,
    asset: state.activeAsset,
    proof_artifact: null,
    proof_note: 'Proof was generated and verified internally. Standalone export not yet implemented.',
    prove_ms: state.auditResult.proveMs,
    window: {
      start_date: state.auditResult.startDate,
      end_date: state.auditResult.endDate,
    },
    total_volume: state.auditResult.totalVolume,
    tx_count: state.auditResult.txs.length,
    disclosed: state.auditResult.selected,
  }, null, 2);
  try {
    await navigator.clipboard.writeText(payload);
    showToast('Audit proof copied to clipboard.', 'success');
  } catch {
    showToast('Copy failed.', 'error');
  }
};

function renderTourStep() {
  const step = TOUR_STEPS[state.tourStep];
  const target = document.querySelector(step.target);
  if (!target) return;

  const rect = target.getBoundingClientRect();
  const pad = 10;
  const highlight = document.getElementById('tour-highlight');
  highlight.style.top = `${rect.top - pad}px`;
  highlight.style.left = `${rect.left - pad}px`;
  highlight.style.width = `${rect.width + pad * 2}px`;
  highlight.style.height = `${rect.height + pad * 2}px`;

  document.getElementById('tour-step').textContent = `Step ${state.tourStep + 1} of ${TOUR_STEPS.length}`;
  document.getElementById('tour-title').textContent = step.title;
  document.getElementById('tour-copy').textContent = step.copy;
  document.getElementById('tour-dots').innerHTML = TOUR_STEPS.map((_, index) => `<div class="tour-dot ${index === state.tourStep ? 'active' : ''}"></div>`).join('');

  const card = document.getElementById('tour-card');
  const fitsRight = rect.right + 390 < window.innerWidth;
  card.style.left = fitsRight ? `${rect.right + 18}px` : `${Math.max(16, rect.left - 380)}px`;
  card.style.top = `${Math.min(window.innerHeight - 260, Math.max(20, rect.top))}px`;
}

window.addEventListener('resize', () => {
  if (state.tourOpen && state.screen === 'wallet') renderTourStep();
});

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
