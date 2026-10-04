import assert from 'node:assert/strict';
import { test } from 'node:test';
import React from 'react';
import { renderToString } from 'react-dom/server';
import { createServer } from 'vite';
import { DEFAULT_PANELS } from './panel-layout.js';

const ledgerFixture = () => {
  const agg = { input: 100, output: 20, cache_read: 30, cache_write: 10, reasoning: 5, total: 160, cost: 0.12, messages: 2 };
  return {
    today: agg,
    week: agg,
    month: agg,
    daily_totals: [{ date: '2026-09-29', tokens: 160, cost: 0.12, messages: 2 }],
    daily_detail: {},
    weekday_hour: Array.from({ length: 7 }, () => Array(24).fill(0)),
    hourly: Array(24).fill(0),
    weekday: Array(7).fill(0),
    by_model: [{ model: 'test', agg, speed: { timed_calls: 2, duration_ms_total: 2000, output_total: 120, ttft_ms_total: 300, ttft_samples: 2 } }],
    month_by_client: [{ client: 'codex', agg }],
    by_project: [{ project: 'test', agg, sessions: 1 }],
    by_session: [{ client: 'codex', label: 'Codex', session: 'test', agg }],
    sources: [{ client: 'codex', label: 'Codex', records: 3 }],
    analysis_rows: [{ date: '2026-09-29', client: 'codex', agg, unpriced_messages: 0 }],
    intraday_rows: [],
    recent_by_agent: [],
    burn_rate: null,
    last_activity_ms: null,
  };
};

test('usage board renders every default module with ledger data', async () => {
  const server = await createServer({ server: { middlewareMode: true }, appType: 'custom' });
  try {
    const { default: UsageDashboard } = await server.ssrLoadModule('/src/UsageDashboard.jsx');
    const html = renderToString(React.createElement(UsageDashboard, {
      ledger: ledgerFixture(),
      monthlyValue: { month_api_equivalent_usd: 12.5, list_price_multiple: 3.1 },
      lang: 0,
      currency: { code: 'USD', rate: 1 },
      editing: false,
    }));
    assert.equal((html.match(/class="panel-card/g) || []).length, DEFAULT_PANELS.length);
    assert.doesNotMatch(html, /PANEL ERROR/);
    assert.match(html, /命中率/);
    assert.match(html, /成就/);
  } finally {
    await server.close();
  }
});

test('quota advice points at the calmest provider only when the tightest is strained', async () => {
  const server = await createServer({ server: { middlewareMode: true }, appType: 'custom' });
  try {
    const { pickQuotaAdvice } = await server.ssrLoadModule('/src/QuotaPage.jsx');
    const rows = [
      { provider: 'codex', name: 'Codex', kind: 'quota', used: 96, status: 'exhausted' },
      { provider: 'goat', name: 'Command Code GOAT', kind: 'quota', used: 9, status: 'ok' },
      { provider: 'opencode-go', name: 'OpenCode Go', kind: 'quota', used: 44, status: 'ok' },
      { provider: 'antigravity', name: 'Antigravity', kind: 'activity', used: 13, status: 'ok' },
    ];
    assert.deepEqual(pickQuotaAdvice(rows), { name: 'Command Code GOAT', pct: 9 });
    assert.equal(pickQuotaAdvice(rows.map((row) => ({ ...row, status: 'ok', used: 20 }))), null);
    assert.equal(pickQuotaAdvice([{ provider: 'x', name: 'X', kind: 'activity', used: 99, status: 'exhausted' }]), null);
  } finally {
    await server.close();
  }
});

test('quota page keeps the quota strip and mutes expired pace warnings', async () => {
  const server = await createServer({ server: { middlewareMode: true }, appType: 'custom' });
  const originalNow = Date.now;
  try {
    const { default: QuotaPage } = await server.ssrLoadModule('/src/QuotaPage.jsx');
    const endsAt = new Date(2026, 10, 15, 12).getTime();
    const now = endsAt - 9 * 86_400_000;
    Date.now = () => now;
    const html = renderToString(React.createElement(QuotaPage, {
      cards: [{ kind: 'Live', provider: 'codex', title: 'Codex', subscription_ends_at_ms: endsAt,
        limits: [{ label: '5 hours', used_pct: 20, resets_in_ms: 3 * 86_400_000, status: 'ok' }] }],
      ledger: ledgerFixture(),
      quotaFetchedAtMs: now - 2 * 86_400_000, now, lang: 1, quotaStale: true,
      preferences: { hiddenCards: ['deepseek'], pinnedCards: [], providerColors: {} },
    }));
    assert.match(html, /expires 11-15/);
    assert.doesNotMatch(html, /quota-expiry-warning/);
    assert.match(html, /1d 0h/);
    assert.match(html, /quota-strip/);
  } finally {
    Date.now = originalNow;
    await server.close();
  }
});
