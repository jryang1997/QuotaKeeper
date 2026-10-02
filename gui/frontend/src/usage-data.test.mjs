import test from 'node:test';
import assert from 'node:assert/strict';
import { aggregateIntraday, aggregateUsage, usageBounds } from './usage-data.js';

const agg = (total, messages, cost, input = total) => ({
  total, messages, cost, input, output: total - input,
  cache_read: 0, cache_write: 0, reasoning: 0,
});

test('a single range and agent filter drive totals and daily bars together', () => {
  const rows = [
    { date: '2026-09-27', client: 'codex', agg: agg(10, 1, 0.1), unpriced_messages: 0 },
    { date: '2026-09-28', client: 'codex', agg: agg(20, 2, 0.2), unpriced_messages: 1 },
    { date: '2026-09-28', client: 'claude', agg: agg(30, 3, 0.3), unpriced_messages: 0 },
    { date: '2026-09-22', client: 'codex', agg: agg(99, 9, 0.9), unpriced_messages: 0 },
  ];
  const bounds = usageBounds('7d', '2026-09-29');
  const all = aggregateUsage(rows, bounds);
  assert.equal(all.total.total, 60);
  assert.equal(all.total.messages, 6);
  assert.equal(all.days.length, 7);
  assert.equal(all.days[5].agg.total, 50);
  const codex = aggregateUsage(rows, bounds, 'codex');
  assert.equal(codex.total.total, 30);
  assert.equal(codex.days[5].agg.total, 20);
  assert.equal(codex.days[5].agents.get('codex').total, 20);
  assert.equal(codex.unpriced, 1);
});

test('invalid or overlong custom ranges cannot show totals without matching bars', () => {
  const row = [{ date: '2026-09-29', client: 'codex', agg: agg(10, 1, 0), unpriced_messages: 0 }];
  assert.equal(aggregateUsage(row, usageBounds('custom', '2026-09-29', '', '2026-09-29')).valid, false);
  assert.equal(aggregateUsage(row, usageBounds('custom', '2026-09-29', '2026-09-30', '2026-09-29')).valid, false);
  assert.equal(aggregateUsage(row, usageBounds('custom', '2026-09-29', '2025-01-01', '2026-09-29')).valid, false);
});

test('single-day trends use twelve two-hour buckets with the same totals', () => {
  const date = '2026-09-29';
  const rows = [
    { date, hour: 0, client: 'codex', agg: agg(10, 1, 0.1), unpriced_messages: 0 },
    { date, hour: 1, client: 'claude', agg: agg(20, 2, 0.2), unpriced_messages: 1 },
    { date, hour: 13, client: 'codex', agg: agg(30, 3, 0.3), unpriced_messages: 0 },
    { date: '2026-09-28', hour: 1, client: 'codex', agg: agg(99, 9, 0.9) },
  ];
  const all = aggregateIntraday(rows, date);
  assert.equal(all.length, 12);
  assert.equal(all[0].agg.total, 30);
  assert.equal(all[6].agg.total, 30);
  assert.equal(all.reduce((sum, bucket) => sum + bucket.agg.total, 0), 60);
  assert.equal(all[0].unpriced, 1);
  const codex = aggregateIntraday(rows, date, 'codex');
  assert.equal(codex[0].agg.total, 10);
  assert.equal(codex[6].agg.total, 30);
});

import { cacheHitRate, fmtDurationMs, speedBoard } from './usage-data.js';

test('cacheHitRate is cache share of all prompted tokens', () => {
  assert.equal(cacheHitRate({ input: 30, cache_read: 70 }), 0.7);
  assert.equal(cacheHitRate({ input: 10, cache_read: 0 }), 0);
  assert.equal(cacheHitRate({ input: 0, cache_read: 0 }), null);
  assert.equal(cacheHitRate(null), null);
});

test('fmtDurationMs picks the readable unit', () => {
  assert.equal(fmtDurationMs(800), '800ms');
  assert.equal(fmtDurationMs(3800), '3.8s');
  assert.equal(fmtDurationMs(12_400), '12s');
  assert.equal(fmtDurationMs(0), '—');
  assert.equal(fmtDurationMs(null), '—');
});

test('speedBoard ranks by tok/s and counts untimed models', () => {
  const models = [
    { model: 'slow', speed: { timed_calls: 2, ttft_samples: 2, ttft_ms_total: 4000, duration_ms_total: 20_000, output_total: 1000 } },
    { model: 'fast', speed: { timed_calls: 3, ttft_samples: 1, ttft_ms_total: 300, duration_ms_total: 3000, output_total: 900 } },
    { model: 'untimed', agg: {} },
  ];
  const board = speedBoard(models);
  assert.deepEqual(board.rows.map((r) => r.model), ['fast', 'slow']);
  assert.equal(board.rows[0].tokPerSec, 300);
  // ttft 只对有 ttft 样本的调用取均值:fast 仅 1 个样本,不被另外两次稀释
  assert.equal(board.rows[0].ttftMs, 300);
  assert.equal(board.rows[1].ttftMs, 2000);
  assert.equal(board.unTimed, 1);
  // limit 截断
  assert.equal(speedBoard(models, 1).rows.length, 1);
  assert.equal(speedBoard([]).rows.length, 0);
});
