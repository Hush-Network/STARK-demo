export function esc(s) {
  if (s == null) return '';
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

// Format a [u32; 4] array as a 0x-prefixed 32-char hex string (4 x 8 hex digits).
export function fmtHash4(arr) {
  if (!Array.isArray(arr) || arr.length !== 4) return '0x' + String(arr);
  return '0x' + arr.map((v) => (v >>> 0).toString(16).padStart(8, '0')).join('');
}

export function fmtMoney(value) {
  return value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

export function fmtAssetValue(value) {
  return value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 3 });
}

export function fmtFee(value) {
  return value.toLocaleString('en-US', { minimumFractionDigits: 4, maximumFractionDigits: 4 });
}

export function relativeTime(date) {
  const diff = Math.max(0, Math.floor((Date.now() - date.getTime()) / 1000));
  if (diff < 60) return 'just now';
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

export function sanitizeAmountInput(raw) {
  const clean = raw.replace(/[^0-9.]/g, '');
  const parts = clean.split('.');
  const whole = parts[0] || '0';
  const decimals = parts[1] ? parts[1].slice(0, 2) : '';
  const formattedWhole = Number.parseInt(whole, 10).toLocaleString('en-US');
  return decimals.length ? `${formattedWhole}.${decimals}` : formattedWhole;
}

export function parseAmountInput(value) {
  const parsed = Number.parseFloat(value.replace(/,/g, ''));
  return Number.isFinite(parsed) ? parsed : 0;
}

export function createReceiptId() {
  const bytes = new Uint8Array(8);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}
