import test from 'node:test';
import assert from 'node:assert/strict';
import { aggregateUsage, usageBounds } from './usage-data.js';

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
