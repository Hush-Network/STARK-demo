import { esc, fmtMoney, relativeTime } from './formatters.js';
import {
  DEMO_ASSISTANT_PROMPT,
  DEMO_FALLBACK_BALANCE,
  DEMO_IDENTITY_STATUS,
  DEMO_MEMBER_SINCE,
  DEMO_PRIVACY_NOTE,
  DEMO_SESSION_COPY,
  DEMO_SESSION_LABEL,
  DEMO_USER_ADDRESS,
  DEMO_USER_HANDLE,
} from '../config/demo-fixtures.js';

const svg = {
  sparkle: () => `
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M9.5 3 11 7l4 1.5-4 1.5L9.5 14 8 10 4 8.5 8 7Z"/><path d="M18 13.5 19 16l2.5 1-2.5 1L18 20.5 17 18l-2.5-1 2.5-1Z"/>
    </svg>`,
  sun: () => `
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41"/>
    </svg>`,
  moon: () => `
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79Z"/>
    </svg>`,
  bell: () => `
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/>
    </svg>`,
  send: () => `
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M5 19 19 5M9 5h10v10"/>
    </svg>`,
  request: () => `
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M19 5 5 19M15 19H5V9"/>
    </svg>`,
  plus: (size = 14) => `
    <svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M12 5v14M5 12h14"/>
    </svg>`,
  more: () => `
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <circle cx="5" cy="12" r="1.3"/><circle cx="12" cy="12" r="1.3"/><circle cx="19" cy="12" r="1.3"/>
    </svg>`,
  copy: () => `
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
    </svg>`,
  lock: (size = 11) => `
    <svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <rect x="4" y="11" width="16" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>
    </svg>`,
  wallet: () => `
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M19 7H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2Z"/><path d="M3 9V7a2 2 0 0 1 2-2h12"/><circle cx="17" cy="14" r="1.5"/>
    </svg>`,
  activity: () => `
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M3 12h4l3-9 4 18 3-9h4"/>
    </svg>`,
  shield: () => `
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10Z"/><path d="m9 12 2 2 4-4"/>
    </svg>`,
  settings: () => `
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3h.1a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8v.1a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z"/>
    </svg>`,
  help: () => `
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="10"/><path d="M9.1 9a3 3 0 0 1 5.8 1c0 2-3 3-3 3"/><path d="M12 17h.01"/>
    </svg>`,
  arrowIn: () => `
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M19 5 5 19M15 19H5V9"/>
    </svg>`,
  menu: () => `
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M4 6h16M4 12h16M4 18h16"/>
    </svg>`,
};

const NAV_ITEMS = [
  { id: 'wallet', label: 'Wallet', icon: 'wallet' },
  { id: 'activity', label: 'Activity', icon: 'activity' },
  { id: 'compliance', label: 'Disclose', icon: 'shield' },
];

const ACCENT_PRESETS = [
  { id: 'mint', label: 'Mint', accent: '#0891b2', accent2: '#22d3ee', deep: '#5eead4' },
  { id: 'sage', label: 'Sage', accent: '#10b981', accent2: '#5eead4', deep: '#34d399' },
  { id: 'violet', label: 'Violet', accent: '#8b5cf6', accent2: '#a78bfa', deep: '#c4b5fd' },
  { id: 'sky', label: 'Sky', accent: '#0ea5e9', accent2: '#38bdf8', deep: '#7dd3fc' },
  { id: 'rose', label: 'Rose', accent: '#f43f5e', accent2: '#fb7185', deep: '#fda4af' },
];

const ASSET_DEFS = [
  { sym: 'USDT', name: 'Tether USD', glyph: '$' },
  { sym: 'USDC', name: 'USD Coin', glyph: '$' },
  {
    sym: 'HUSH',
    name: 'HUSH gas reserve',
    glyph: '<img src="/images/hushlogo.png" alt="HUSH" class="asset-logo">',
    selectable: false,
  },
];

export function renderSidebar(activeView) {
  const items = NAV_ITEMS.map((item) => `
    <li class="nav-item ${activeView === item.id ? 'active' : ''}" onclick="setActiveView('${item.id}')">
      <span class="nav-ico">${svg[item.icon]()}</span>
      <span class="nav-lbl">${item.label}</span>
    </li>
  `).join('');

  return `
    <a class="brand" href="https://hushnetwork.io" target="_blank" rel="noreferrer" aria-label="Hush Network">
      <img src="/images/hushlogo.png" alt="HushPay" class="brand-logo">
      <span class="brand-name">Hush<span class="brand-pay">Pay</span></span>
    </a>

    <nav>
      <ul class="nav-group">${items}</ul>
    </nav>

    <div class="side-spacer"></div>

    <div class="session">
      <div class="session-hd">
        <span class="dot"></span>
        ${esc(DEMO_SESSION_LABEL)}
      </div>
      <p>${esc(DEMO_SESSION_COPY)}</p>
    </div>

    <div class="side-footer">
      <button class="foot-btn" onclick="toggleTweaks()">${svg.settings()} Customize UI</button>
      <button class="foot-btn muted" onclick="askComingSoon('Help')">${svg.help()} Help</button>
    </div>
  `;
}

export function renderTopbar(theme, handle = DEMO_USER_HANDLE) {
  const isDark = theme === 'dark';
  const initial = (handle || 'U').charAt(0).toUpperCase();
  return `
    <button class="menu-btn" onclick="toggleSidebar()" aria-label="Open menu">${svg.menu()}</button>
    <button class="ask" onclick="openComposer()">
      ${svg.sparkle()}
      <span class="ask-txt">${esc(DEMO_ASSISTANT_PROMPT)}</span>
      <span class="kbd">Cmd+K</span>
    </button>
    <div class="top-right">
      <button class="icon-btn theme-btn" onclick="toggleTheme()" aria-label="${isDark ? 'Switch to light mode' : 'Switch to dark mode'}" title="${isDark ? 'Light mode' : 'Dark mode'}">
        ${isDark ? svg.sun() : svg.moon()}
      </button>
      <button class="icon-btn" onclick="askComingSoon('Notifications')" aria-label="Notifications">${svg.bell()}</button>
      <div class="avatar-sm" title="${esc(handle)}"><span>${esc(initial)}</span></div>
    </div>
  `;
}

export function renderBalanceCard(_state, opts = {}) {
  const userHandle = opts.handle || DEMO_USER_HANDLE;
  const balance = opts.headlineBalance || DEMO_FALLBACK_BALANCE;
  const formattedBalance = balance.toLocaleString('en-US', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });

  return `
    <section class="card balance-card">
      <div class="hero-band">
        <div class="hero-bg hero-solid"></div>
      </div>

      <div class="hero-avatar">
        <div class="avatar-lg"><span>${esc(userHandle.charAt(0).toUpperCase())}</span></div>
      </div>

      <div class="balance-meta">
        <div class="identity">
          <h1 class="identity-name">${esc(userHandle)}</h1>
          <div class="identity-sub">
            <span>${esc(DEMO_IDENTITY_STATUS)}</span>
            <span class="dotsep">|</span>
            <span>${esc(DEMO_MEMBER_SINCE)}</span>
          </div>
        </div>

        <div class="balance-wrap">
          <div class="balance-label">Total balance</div>
          <div class="balance-num">$${formattedBalance}</div>
        </div>
      </div>

      <div class="actions-row">
        <button class="btn btn-primary" onclick="openComposer()">${svg.send()} Send</button>
        <button class="btn btn-ghost" onclick="askComingSoon('Request')">${svg.request()} Request</button>
        <button class="btn btn-ghost" onclick="askComingSoon('Deposit')">${svg.plus()} Deposit</button>
        <div class="spacer"></div>
        <button class="copy-pill" onclick="copyAddress()" title="Copy address">
          <span>${esc(DEMO_USER_ADDRESS)}</span>
          <span class="copy-ico">${svg.copy()}</span>
        </button>
        <button class="btn btn-round" aria-label="More" onclick="askComingSoon('More options')">${svg.more()}</button>
      </div>
    </section>
  `;
}

export function renderBalancesTable(state, balanceFor) {
  const rows = ASSET_DEFS.map((asset) => {
    const row = balanceFor(asset.sym);
    const selectable = asset.selectable !== false;
    return `
      <li class="asset-row ${selectable ? '' : 'is-static'} ${selectable && state.activeAsset === asset.sym ? 'is-active' : ''}" ${selectable ? `onclick="switchAsset('${asset.sym}')"` : ''}>
        <div class="asset-left">
          <div class="asset-bubble">${asset.glyph}</div>
          <div class="asset-name">
            <div class="nm">${asset.name}</div>
          </div>
        </div>
        <div class="asset-right">
          <div class="asset-val">${row.value}</div>
          <div class="asset-bal"><span class="num">${row.balance}</span> <span class="sym">${asset.sym}</span></div>
        </div>
      </li>
    `;
  }).join('');

  return `
    <section class="card balances-card">
      <div class="tabs">
        <button class="tab active" onclick="setActiveView('wallet')">Balances</button>
        <button class="tab" onclick="setActiveView('activity')">Activity</button>
        <button class="tab" onclick="setActiveView('compliance')">Disclose</button>
      </div>

      <ul class="asset-list">${rows}</ul>

      <button class="add-asset" onclick="askComingSoon('Add asset')">${svg.plus(13)} Add asset</button>
    </section>
  `;
}

export function renderRecentActivity(transactions) {
  if (!transactions.length) {
    return `
      <section class="card activity-card">
        <div class="act-hd">
          <h3>Recent activity</h3>
          <button class="see-all" onclick="setActiveView('activity')">See all</button>
        </div>
        <p class="empty-copy compact-empty">No payments yet.</p>
      </section>
    `;
  }

  const items = transactions.slice(0, 5).map((tx) => {
    const isPositive = tx.kind === 'received';
    const amountLabel = `${isPositive ? '+' : '-'} $${fmtMoney(tx.amount)}`;
    return `
      <li class="act-row" onclick="showReceipt('${esc(tx.id)}')">
        <div class="act-ico">${tx.kind === 'received' ? svg.arrowIn() : svg.send()}</div>
        <div class="act-body">
          <div class="act-who">${esc(tx.recipient || 'Counterparty')}</div>
          <div class="act-sub">
            <span>${esc(tx.note || tx.feeAsset || tx.asset)}</span>
            <span class="dotsep">|</span>
            <span>${esc(relativeTime(tx.time))}</span>
          </div>
        </div>
        <div class="act-amt ${isPositive ? 'pos' : 'neg'}">${amountLabel}</div>
      </li>
    `;
  }).join('');

  return `
    <section class="card activity-card">
      <div class="act-hd">
        <h3>Recent activity</h3>
        <button class="see-all" onclick="setActiveView('activity')">See all</button>
      </div>
      <ul class="act-list">${items}</ul>
    </section>
  `;
}

export function renderPrivacyNote() {
  return `
    <section class="priv-note">
      <div class="priv-hd">${svg.lock(14)}<span>Zero-knowledge privacy</span></div>
      <p>${esc(DEMO_PRIVACY_NOTE)}</p>
    </section>
  `;
}

export function renderTweaksPanel(state) {
  const accentRow = ACCENT_PRESETS.map((preset) => `
    <button class="sw ${state.tweaks.accent === preset.id ? 'active' : ''}"
            style="background: ${preset.accent}"
            onclick="setTweak('accent', '${preset.id}')"
            title="${preset.label}"></button>
  `).join('');

  return `
    <div class="tweaks-head">
      <h4>Customize UI</h4>
      <button class="close-button" onclick="toggleTweaks()" aria-label="Close">x</button>
    </div>

    <div class="tweaks-body">
      <div class="tweak-section">Theme</div>
      <div class="tweak-radio">
        <button class="tweak-opt ${state.theme === 'light' ? 'active' : ''}" onclick="setTheme('light')">Light</button>
        <button class="tweak-opt ${state.theme === 'dark' ? 'active' : ''}" onclick="setTheme('dark')">Dark</button>
      </div>

      <div class="tweak-section">Accent</div>
      <div class="swatch-grid">${accentRow}</div>

      <div class="tweak-section">Density</div>
      <div class="tweak-radio">
        <button class="tweak-opt ${state.tweaks.density === 'compact' ? 'active' : ''}" onclick="setTweak('density','compact')">Compact</button>
        <button class="tweak-opt ${state.tweaks.density === 'cozy' ? 'active' : ''}" onclick="setTweak('density','cozy')">Cozy</button>
        <button class="tweak-opt ${state.tweaks.density === 'comfy' ? 'active' : ''}" onclick="setTweak('density','comfy')">Comfy</button>
      </div>

      <div class="tweak-section">Card style</div>
      <div class="tweak-radio">
        <button class="tweak-opt ${state.tweaks.cardStyle === 'soft' ? 'active' : ''}" onclick="setTweak('cardStyle','soft')">Soft</button>
        <button class="tweak-opt ${state.tweaks.cardStyle === 'outline' ? 'active' : ''}" onclick="setTweak('cardStyle','outline')">Outline</button>
        <button class="tweak-opt ${state.tweaks.cardStyle === 'glass' ? 'active' : ''}" onclick="setTweak('cardStyle','glass')">Glass</button>
      </div>

      <div class="tweak-section">Roundness</div>
      <input type="range" min="6" max="22" value="${state.tweaks.radius}" oninput="setTweak('radius', this.value)">

      <div class="tweak-section">Font size</div>
      <input type="range" min="13" max="18" value="${state.tweaks.fontSize}" oninput="setTweak('fontSize', this.value)">
    </div>
  `;
}

export function renderComposerOverlay(composerInnerHtml) {
  return `
    <div class="composer-modal" onclick="closeComposerOverlay(event)">
      <div class="composer-sheet" onclick="event.stopPropagation()">
        <div class="composer-modal-head">
          <h2>Send private payment</h2>
          <button class="close-button" onclick="closeComposerOverlay()" aria-label="Close">x</button>
        </div>
        <div class="composer-modal-body">${composerInnerHtml}</div>
      </div>
    </div>
  `;
}
