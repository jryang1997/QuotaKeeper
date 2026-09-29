import assert from 'node:assert/strict';
import { test } from 'node:test';
import React from 'react';
import { renderToString } from 'react-dom/server';
import { createServer } from 'vite';

test('all default panels render with ledger data', async () => {
  const server = await createServer({ server: { middlewareMode: true }, appType: 'custom' });
  try {
    const { default: PanelsPage } = await server.ssrLoadModule('/src/PanelsPage.jsx');
    const agg = { input: 100, output: 20, cache_read: 30, cache_write: 10, reasoning: 5, total: 165, cost: 0.12, messages: 2 };
    const ledger = {
      daily_totals: [{ date: '2026-09-29', tokens: 165, cost: 0.12, messages: 2 }],
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
