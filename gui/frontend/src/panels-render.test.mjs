import assert from 'node:assert/strict';
import { test } from 'node:test';
import React from 'react';
import { renderToString } from 'react-dom/server';
import { createServer } from 'vite';

test('all default panels render with ledger data', async () => {
  const server = await createServer({ server: { middlewareMode: true }, appType: 'custom' });
  try {
    const { default: PanelsPage } = await server.ssrLoadModule('/src/PanelsPage.jsx');
    const agg = { input: 100, output: 20, cache_read: 30, cache_write: 10, reasoning: 5, total: 160, cost: 0.12, messages: 2 };
    const ledger = {
      daily_totals: [{ date: '2026-09-29', tokens: 160, cost: 0.12, messages: 2 }],
      daily_detail: {},
      weekday_hour: Array.from({ length: 7 }, () => Array(24).fill(0)),
      hourly: Array(24).fill(0),
      weekday: Array(7).fill(0),
      by_model: [{ model: 'test', agg }],
      month_by_client: [{ client: 'codex', agg }],
      by_project: [{ project: 'test', agg, sessions: 1 }],
      by_session: [{ session: 'test', agg }],
      month: agg,
    };
    const html = renderToString(React.createElement(PanelsPage, { ledger, lang: 0 }));
    assert.equal((html.match(/class="panel-card/g) || []).length, 12);
    assert.doesNotMatch(html, /PANEL ERROR/);
    assert.match(html, /缓存命中/);
  } finally {
    await server.close();
  }
});

test('quota countdown advances without moving the subscription expiry date', async () => {
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
      sources: [], quotaFetchedAtMs: now - 2 * 86_400_000, now, lang: 1, quotaStale: true,
      preferences: { hiddenCards: ['deepseek'], pinnedCards: [], providerColors: {} },
    }));
    assert.match(html, /expires 11-15/);
    assert.doesNotMatch(html, /quota-expiry-warning/);
    assert.match(html, /1d 0h/);
  } finally {
    Date.now = originalNow;
    await server.close();
  }
});
