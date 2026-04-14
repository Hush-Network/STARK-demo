import { esc, fmtFee, fmtMoney, relativeTime } from './formatters.js';

export function renderActivity(activity) {
  if (!activity.length) {
    return '<div class="empty-copy compact-empty">No payments yet.</div>';
  }

  return `
    <div class="ledger-table">
      <div class="ledger-head">
        <span>Counterparty</span>
        <span>Amount</span>
        <span>Route</span>
        <span>Time</span>
        <span>Action</span>
      </div>
      ${activity.map((item) => {
        if (item.kind === 'audit') {
          return `
            <div class="ledger-row ledger-row-audit">
              <span class="ledger-main ledger-counterparty">Audit summary</span>
              <span class="ledger-amount">${esc(item.copy.split('|')[1]?.trim() || '--')}</span>
              <span class="ledger-route">Window proof</span>
              <span class="ledger-time">${esc(relativeTime(item.time))}</span>
              <button class="ledger-action ledger-action-cell" onclick="renderAuditResult(); document.getElementById('audit-overlay').classList.add('show')">Open</button>
            </div>
          `;
        }

        return `
          <div class="ledger-row">
            <span class="ledger-main ledger-counterparty">${esc(item.recipient || 'Counterparty')}</span>
            <span class="ledger-amount">${esc(fmtMoney(item.amount))} ${esc(item.asset)}</span>
            <span class="ledger-route">${esc(item.feeAsset)}</span>
            <span class="ledger-time">${esc(relativeTime(item.time))}</span>
            <button class="ledger-action ledger-action-cell" onclick="showReceipt('${esc(item.id)}')">Receipt</button>
          </div>
        `;
      }).join('')}
    </div>
  `;
}

export function renderProofLog(proofLog) {
  return proofLog.map((entry) => `
    <div class="proof-entry ${entry.kind}">
      <div class="proof-entry-top">
        <span class="proof-entry-kind">${entry.kind === 'success' ? 'Success' : entry.kind === 'error' ? 'Blocked or failed' : 'Info'}</span>
        <span class="proof-entry-time">${entry.time}</span>
      </div>
      <div class="proof-entry-message">${esc(entry.message)}</div>
    </div>
  `).join('');
}

export function renderProofOutputs(proofOutputs) {
  if (!proofOutputs.length) {
    return '<p class="empty-copy">No public proof outputs yet.</p>';
  }

  return proofOutputs.map((output) => `
    <div class="proof-output">
      <div class="proof-output-label">${esc(output.label)}</div>
      <div class="proof-output-value">${esc(output.value)}</div>
      <div class="proof-output-note">${esc(output.note)}</div>
    </div>
  `).join('');
}

export function renderPayoutPreview(lastSubmission, amtScale) {
  const payoutRecords = lastSubmission?.payout_inspection?.payout_records || [];
  if (!payoutRecords.length) {
    return '<p class="empty-copy compact-empty">No payout records yet.</p>';
  }

  return payoutRecords.map((record) => `
    <div class="proof-output">
      <div class="proof-output-label">Validator ${record.validator_id}</div>
      <div class="proof-output-value">Key ${record.payout_key}</div>
      <div class="proof-output-note">HUSH ${fmtFee(record.entitlement.hush / amtScale)} | USDC ${fmtFee(record.entitlement.usdc / amtScale)} | USDT ${fmtFee(record.entitlement.usdt / amtScale)}</div>
    </div>
  `).join('');
}
