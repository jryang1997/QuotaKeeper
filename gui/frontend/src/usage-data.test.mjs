import test from 'node:test';
import assert from 'node:assert/strict';
import { aggregateIntraday, aggregateUsage, seriesMatchesTotal, usageBounds, usageCoverage, usageMetric } from './usage-data.js';

const agg = (total, messages, cost, input = total) => ({
  total, messages, cost, cost_reported: 0, cost_estimated: cost, input, output: total - input,
  cache_read: 0, cache_write: 0, reasoning: 0,
});

const parts = (input, output, cacheRead, cacheWrite, reasoning, messages, cost, unpriced = 0) => ({
  unpriced_messages: unpriced,
  agg: {
    input, output, cache_read: cacheRead, cache_write: cacheWrite, reasoning,
    messages, cost, cost_reported: 0, cost_estimated: cost, total: input + output + cacheRead + cacheWrite,
  },
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
  assert.deepEqual(all.byClient.map((row) => [row.client, row.agg.total, row.unpriced]), [['claude', 30, 0], ['codex', 30, 1]]);
  assert.equal(all.byClient.reduce((sum, row) => sum + row.agg.total, 0), all.total.total);
  const codex = aggregateUsage(rows, bounds, 'codex');
  assert.equal(codex.total.total, 30);
  assert.equal(codex.days[5].agg.total, 20);
  assert.equal(codex.days[5].agents.get('codex').total, 20);
  assert.equal(codex.days[5].agents.has('claude'), false);
  assert.equal(codex.unpriced, 1);
  assert.deepEqual(codex.byClient.map((row) => [row.client, row.agg.total, row.unpriced]), [['codex', 30, 1]]);
});

test('invalid or overlong custom ranges cannot show totals without matching bars', () => {
  const row = [{ date: '2026-09-29', client: 'codex', agg: agg(10, 1, 0), unpriced_messages: 4 }];
  for (const bounds of [
    usageBounds('custom', '2026-09-29', '', '2026-09-29'),
    usageBounds('custom', '2026-09-29', '2026-09-30', '2026-09-29'),
    usageBounds('custom', '2026-09-29', '2025-01-01', '2026-09-29'),
    { start: '2026-09-29 ', end: '2026-09-29' },
  ]) {
    const summary = aggregateUsage(row, bounds);
    assert.equal(summary.valid, false);
    assert.equal(summary.total.total, 0);
    assert.equal(summary.total.messages, 0);
    assert.equal(summary.unpriced, 0);
    assert.deepEqual(summary.byClient, []);
    assert.deepEqual(summary.days, []);
  }
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
  assert.equal(seriesMatchesTotal(codex, { input: 40, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, messages: 4, cost: 0.4, cost_reported: 0, cost_estimated: 0.4, total: 40 }, 0), true);
});

test('preset ranges are local calendar spans', () => {
  assert.deepEqual(usageBounds('today', '2026-10-15'), { start: '2026-10-15', end: '2026-10-15' });
  assert.deepEqual(usageBounds('yesterday', '2026-10-15'), { start: '2026-10-14', end: '2026-10-14' });
  assert.deepEqual(usageBounds('yesterday', '2026-03-01'), { start: '2026-02-28', end: '2026-02-28' });
  assert.deepEqual(usageBounds('yesterday', '2024-03-01'), { start: '2024-02-29', end: '2024-02-29' });
  assert.deepEqual(usageBounds('7d', '2026-10-15'), { start: '2026-10-09', end: '2026-10-15' });
  assert.deepEqual(usageBounds('30d', '2026-10-15'), { start: '2026-09-16', end: '2026-10-15' });
  assert.deepEqual(usageBounds('month', '2026-10-15'), { start: '2026-10-01', end: '2026-10-15' });
  assert.deepEqual(usageBounds('custom', '2026-10-15', '2026-10-02', '2026-10-04'), { start: '2026-10-02', end: '2026-10-04' });
});

test('range and agent filters keep totals, days and clients on the same rows', () => {
  const rows = [
    { date: '2026-10-14', client: 'codex', ...parts(10, 4, 3, 2, 1, 2, 1.5, 1) },
    { date: '2026-10-15', client: 'codex', ...parts(1, 1, 1, 1, 1, 1, 0.5, 0) },
    { date: '2026-10-15', client: 'claude', ...parts(100, 50, 25, 25, 40, 8, 9, 4) },
    { date: '2026-10-01', client: 'codex', ...parts(7, 7, 0, 0, 7, 3, 3, 9) },
  ];
  const today = aggregateUsage(rows, usageBounds('today', '2026-10-15'), 'codex');
  assert.equal(today.valid, true);
  assert.deepEqual(today.total, {
    input: 1, output: 1, cache_read: 1, cache_write: 1, reasoning: 1, messages: 1, cost: 0.5, cost_reported: 0, cost_estimated: 0.5, total: 4,
  });
  assert.equal(today.total.total, today.total.input + today.total.output + today.total.cache_read + today.total.cache_write);
  assert.notEqual(today.total.total, today.total.input + today.total.output + today.total.cache_read + today.total.cache_write + today.total.reasoning);
  assert.deepEqual(today.byClient, [
    { client: 'codex', unpriced: 0, agg: { input: 1, output: 1, cache_read: 1, cache_write: 1, reasoning: 1, messages: 1, cost: 0.5, cost_reported: 0, cost_estimated: 0.5, total: 4 } },
  ]);
  assert.equal(today.unpriced, 0);
  assert.equal(today.days.length, 1);
  assert.equal(today.days[0].date, '2026-10-15');
  assert.equal(today.days[0].agg.total, 4);
  assert.equal(today.days[0].agents.get('claude'), undefined);
  assert.equal(today.days[0].agents.get('codex').total, today.total.total);

  const week = aggregateUsage(rows, usageBounds('7d', '2026-10-15'), 'codex');
  assert.deepEqual(week.total, {
    input: 11, output: 5, cache_read: 4, cache_write: 3, reasoning: 2, messages: 3, cost: 2, cost_reported: 0, cost_estimated: 2, total: 23,
  });
  assert.deepEqual(week.byClient, [
    { client: 'codex', unpriced: 1, agg: { input: 11, output: 5, cache_read: 4, cache_write: 3, reasoning: 2, messages: 3, cost: 2, cost_reported: 0, cost_estimated: 2, total: 23 } },
  ]);
  assert.equal(week.days.filter((day) => day.agg.total > 0).map((day) => day.date).join(), '2026-10-14,2026-10-15');
  assert.equal([...week.days[5].agents.keys()].join(), 'codex');

  const allToday = aggregateUsage(rows, usageBounds('today', '2026-10-15'));
  assert.deepEqual(allToday.total, {
    input: 101, output: 51, cache_read: 26, cache_write: 26, reasoning: 41, messages: 9, cost: 9.5, cost_reported: 0, cost_estimated: 9.5, total: 204,
  });
  assert.deepEqual(allToday.byClient.map((row) => [row.client, row.agg.total, row.agg.messages, row.unpriced]), [
    ['claude', 200, 8, 4],
    ['codex', 4, 1, 0],
  ]);
  assert.equal([...allToday.days[0].agents.values()].reduce((sum, item) => sum + item.total, 0), allToday.total.total);
  assert.equal(allToday.byClient.reduce((sum, row) => sum + row.agg.total, 0), allToday.total.total);
  assert.equal(allToday.unpriced, 4);
});

test('same client rows merge and missing agg fields stay zero', () => {
  const summary = aggregateUsage([
    { date: '2026-10-15', client: 'codex', ...parts(2, 2, 2, 2, 2, 1, 1, 1) },
    { date: '2026-10-15', client: 'codex', unpriced_messages: 1 },
    { date: '2026-10-15', client: 'claude', agg: { total: 5, messages: 1, input: 5 } },
    { date: '2026-10-16', client: 'codex', ...parts(9, 0, 0, 0, 0, 1, 1, 5) },
  ], { start: '2026-10-15', end: '2026-10-15' }, '');
  assert.deepEqual(summary.byClient, [
    { client: 'codex', unpriced: 2, agg: { input: 2, output: 2, cache_read: 2, cache_write: 2, reasoning: 2, messages: 1, cost: 1, cost_reported: 0, cost_estimated: 1, total: 8 } },
    { client: 'claude', unpriced: 0, agg: { input: 5, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, messages: 1, cost: 0, cost_reported: 0, cost_estimated: 0, total: 5 } },
  ]);
  assert.equal(summary.total.total, 13);
  assert.equal(summary.total.messages, 2);
  assert.equal(summary.unpriced, 2);
  assert.equal(summary.days[0].agents.get('codex').total, 8);
  assert.equal(aggregateUsage(null, usageBounds('today', '2026-10-15')).valid, true);
  assert.deepEqual(aggregateUsage(undefined, usageBounds('today', '2026-10-15')).byClient, []);
});

test('client ties sort by requests then name', () => {
  const summary = aggregateUsage([
    { date: '2026-10-15', client: 'omp', ...parts(1, 1, 0, 0, 0, 1, 0) },
    { date: '2026-10-15', client: 'codex', ...parts(2, 0, 0, 0, 0, 4, 0) },
    { date: '2026-10-15', client: 'claude', ...parts(0, 2, 0, 0, 1, 4, 0) },
  ], usageBounds('today', '2026-10-15'));
  assert.deepEqual(summary.byClient.map((row) => row.client), ['claude', 'codex', 'omp']);
});

test('a 366-day span keeps both ends and 367 days keeps none', () => {
  const rows = [
    { date: '2024-01-01', client: 'codex', ...parts(1, 0, 0, 0, 0, 1, 0) },
    { date: '2024-12-31', client: 'omp', ...parts(0, 3, 0, 0, 3, 1, 0, 2) },
  ];
  const inside = aggregateUsage(rows, { start: '2024-01-01', end: '2024-12-31' });
  assert.equal(inside.valid, true);
  assert.equal(inside.days.length, 366);
  assert.equal(inside.days[0].agg.total, 1);
  assert.equal(inside.days[365].agg.total, 3);
  assert.deepEqual(inside.byClient.map((row) => [row.client, row.agg.total, row.unpriced]), [['omp', 3, 2], ['codex', 1, 0]]);
  const outside = aggregateUsage(rows, { start: '2024-01-01', end: '2025-01-01' });
  assert.equal(outside.valid, false);
  assert.equal(outside.total.total, 0);
  assert.deepEqual(outside.byClient, []);
  assert.deepEqual(outside.days, []);
});

test('intraday drops hours outside 0-23', () => {
  const rows = [
    { date: '2026-10-15', hour: -1, client: 'codex', agg: agg(9, 1, 0), unpriced_messages: 1 },
    { date: '2026-10-15', hour: 24, client: 'codex', agg: agg(8, 1, 0) },
    { date: '2026-10-15', hour: 1.5, client: 'codex', agg: agg(7, 1, 0) },
    { date: '2026-10-15', hour: 2, client: 'codex', ...parts(4, 0, 0, 0, 0, 1, 0, 2) },
  ];
  const buckets = aggregateIntraday(rows, '2026-10-15', 'codex');
  assert.equal(buckets[1].agg.total, 4);
  assert.equal(buckets[1].unpriced, 2);
  assert.equal(seriesMatchesTotal(buckets, buckets[1].agg, 2), true);
  assert.equal(seriesMatchesTotal(buckets, buckets[1].agg, 1), false);
  assert.equal(seriesMatchesTotal([], { input: 0, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, messages: 0, cost: 0, cost_reported: 0, cost_estimated: 0, total: 0 }, 0), true);
});

test('every monetary field tolerates addition order and tokens stay exact', () => {
  const buckets = [
    { agg: { input: 1, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, messages: 1, cost: 0.1, cost_reported: 0.05, cost_estimated: 0.05, total: 1 }, unpriced: 0 },
    { agg: { input: 1, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, messages: 1, cost: 0.2, cost_reported: 0.1, cost_estimated: 0.1, total: 1 }, unpriced: 0 },
  ];
  const rounded = { input: 2, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, messages: 2, cost: 0.3, cost_reported: 0.15, cost_estimated: 0.15, total: 2 };
  assert.equal(seriesMatchesTotal(buckets, rounded, 0), true);
  assert.equal(seriesMatchesTotal(buckets, { ...rounded, total: 3 }, 0), false);
  assert.equal(seriesMatchesTotal(buckets, { ...rounded, messages: 3 }, 0), false);
  assert.equal(seriesMatchesTotal(buckets, { ...rounded, input: 3 }, 0), false);
  for (const key of ['cost', 'cost_reported', 'cost_estimated']) {
    assert.equal(seriesMatchesTotal(buckets, { ...rounded, [key]: rounded[key] + 0.01 }, 0), false, key);
  }
});

test('source errors do not turn a filtered zero into confirmed empty usage', () => {
  const sources = [
    { client: 'codex', error: null, records: 3 },
    { client: 'opencode', error: 'opencode.db 打不开', records: 0 },
    { client: 'claude', error: '2 个会话文件读取失败', records: 4 },
  ];
  const zero = { valid: true, total: { total: 0, messages: 0 } };
  const some = { valid: true, total: { total: 4, messages: 1 } };
  assert.deepEqual(usageCoverage(sources, '', { valid: false, total: some.total }), { state: 'invalid', failed: [] });
  assert.deepEqual(usageCoverage(sources, 'codex', zero), { state: 'empty', failed: [] });
  assert.deepEqual(usageCoverage(sources, 'codex', some), { state: 'complete', failed: [] });
  assert.deepEqual(usageCoverage(sources, 'opencode', zero), { state: 'unknown', failed: ['opencode'] });
  assert.deepEqual(usageCoverage(sources, 'claude', some), { state: 'partial', failed: ['claude'] });
  assert.deepEqual(usageCoverage(sources, 'claude', { valid: true, total: { total: 0, messages: 2 } }), { state: 'partial', failed: ['claude'] });
  assert.deepEqual(usageCoverage(sources, '', zero), { state: 'unknown', failed: ['opencode', 'claude'] });
  assert.deepEqual(usageCoverage(sources, '', some), { state: 'partial', failed: ['opencode', 'claude'] });
  assert.deepEqual(usageCoverage([], '', zero), { state: 'unknown', failed: [] });
  assert.deepEqual(usageCoverage(null, '', some), { state: 'unknown', failed: [] });
  assert.deepEqual(usageCoverage(undefined, '', some), { state: 'unknown', failed: [] });
  assert.deepEqual(usageCoverage(sources, 'ghost', some), { state: 'unknown', failed: [] });
  assert.deepEqual(usageCoverage(sources, 'ghost', zero), { state: 'unknown', failed: [] });
});

const modelRows = [
  { date: '2026-10-15', client: 'codex', model: 'gpt-5', raw_model: 'openai/gpt-5', unpriced_messages: 0,
    agg: { input: 4, output: 2, cache_read: 30, cache_write: 4, reasoning: 1, messages: 2, total: 40, cost: 0.2, cost_reported: 0.2, cost_estimated: 0 } },
  { date: '2026-10-15', client: 'opencode', model: 'gpt-5', raw_model: 'gpt-5-latest', unpriced_messages: 0,
    agg: { input: 2, output: 1, cache_read: 10, cache_write: 1, reasoning: 1, messages: 1, total: 14, cost: 0.1, cost_reported: 0, cost_estimated: 0.1 } },
  { date: '2026-10-15', client: 'codex', model: 'unknown', raw_model: '', unpriced_messages: 1,
    agg: { input: 5, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, messages: 1, total: 5, cost: 0, cost_reported: 0, cost_estimated: 0 } },
];

test('canonical model selection filters totals, daily buckets and collectors together', () => {
  const bounds = { start: '2026-10-15', end: '2026-10-15' };
  const selected = aggregateUsage(modelRows, bounds, '', 'gpt-5');
  assert.equal(selected.total.total, 54);
  assert.equal(selected.total.messages, 3);
  assert.equal(selected.unpriced, 0);
  assert.equal(selected.days[0].agg.total, 54);
  assert.deepEqual(selected.byClient.map((row) => [row.client, row.agg.total]), [['codex', 40], ['opencode', 14]]);
  const codex = aggregateUsage(modelRows, bounds, 'codex', 'gpt-5');
  assert.equal(codex.total.total, 40);
  assert.equal(codex.days[0].agents.has('opencode'), false);
});

test('model distribution conserves a 45-day selection with raw aliases, clients, cache and unknown models', () => {
  const rows = [
    ...modelRows,
    { date: '2026-09-01', client: 'codex', model: 'gpt-5', raw_model: 'provider/gpt-5', unpriced_messages: 0,
      agg: { input: 1, output: 1, cache_read: 3, cache_write: 1, reasoning: 0, messages: 1, total: 6, cost: 0.05, cost_reported: 0, cost_estimated: 0.05 } },
    { date: '2026-09-02', client: 'claude', model: 'unknown', raw_model: 'not-reported', unpriced_messages: 2,
      agg: { input: 1, output: 1, cache_read: 4, cache_write: 1, reasoning: 1, messages: 2, total: 7, cost: 0, cost_reported: 0, cost_estimated: 0 } },
    { date: '2026-08-31', client: 'codex', model: 'excluded', raw_model: 'excluded', unpriced_messages: 0,
      agg: { input: 99, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, messages: 1, total: 99, cost: 1, cost_reported: 1, cost_estimated: 0 } },
  ];
  const summary = aggregateUsage(rows, { start: '2026-09-01', end: '2026-10-15' });
  assert.ok(Array.isArray(summary.byModel), 'a model distribution is returned for the active range');
  assert.equal(summary.days.length, 45);
  assert.equal(summary.days[0].agg.total, 6);
  assert.equal(summary.total.total, 72);
  assert.equal(summary.total.input, 13);
  assert.equal(summary.total.output, 5);
  assert.equal(summary.total.cache_read, 47);
  assert.equal(summary.total.cache_write, 7);
  assert.equal(summary.total.reasoning, 3);
  assert.equal(summary.total.messages, 7);
  for (const [key, expected] of Object.entries({ cost: 0.35, cost_reported: 0.2, cost_estimated: 0.15 })) {
    assert.ok(Math.abs(summary.total[key] - expected) < 1e-10, key);
  }
  assert.equal(summary.unpriced, 3);
  assert.deepEqual(summary.byModel.map((row) => [row.model, row.agg.total, row.agg.messages, row.unpriced, row.rawModels, row.clients]), [
    ['gpt-5', 60, 4, 0, ['gpt-5-latest', 'openai/gpt-5', 'provider/gpt-5'], ['codex', 'opencode']],
    ['unknown', 12, 3, 3, ['', 'not-reported'], ['claude', 'codex']],
  ]);
  for (const split of [summary.days, summary.byClient, summary.byModel]) {
    assert.equal(seriesMatchesTotal(split, summary.total, summary.unpriced), true);
  }
});

test('custom ranges reject impossible calendar dates and accept leap day', () => {
  for (const date of ['2026-02-29', '2026-02-30', '2026-04-31', '2026-00-01', '2026-13-01', '2026-10-00']) {
    const summary = aggregateUsage(modelRows, { start: date, end: date });
    assert.equal(summary.valid, false, date);
    assert.equal(summary.total.total, 0);
    assert.deepEqual(summary.days, []);
    assert.deepEqual(summary.byModel, []);
  }
  const leapDay = aggregateUsage([], { start: '2024-02-29', end: '2024-02-29' });
  assert.equal(leapDay.valid, true);
  assert.deepEqual(leapDay.days.map((day) => day.date), ['2024-02-29']);
});

test('all-unpriced cost is unknown while an explicitly priced zero stays zero', () => {
  const unknown = modelRows[2];
  assert.equal(usageMetric(unknown.agg, 'cost', unknown.unpriced_messages), null);
  assert.equal(usageMetric(unknown.agg, 'cost', 0), 0);
  assert.equal(usageMetric({ ...unknown.agg, messages: 2 }, 'cost', 1), 0);
  assert.equal(usageMetric(modelRows[0].agg, 'cost', 0), 0.2);
  assert.equal(usageMetric(unknown.agg, 'tokens', 1), 5);
  assert.equal(usageMetric(unknown.agg, 'messages', 1), 1);
});

test('filtered hourly buckets match daily data only with the same models, collectors and all monetary fields', () => {
  const date = '2026-10-15';
  const intraday = modelRows.map((row, index) => ({ ...row, hour: [0, 3, 23][index] }));
  for (const [agent, model, expected] of [['', 'gpt-5', 54], ['codex', 'gpt-5', 40], ['', 'unknown', 5], ['opencode', 'unknown', 0]]) {
    const summary = aggregateUsage(modelRows, { start: date, end: date }, agent, model);
    const buckets = aggregateIntraday(intraday, date, agent, model);
    assert.equal(summary.total.total, expected);
    assert.equal(seriesMatchesTotal(buckets, summary.total, summary.unpriced), true, `${agent}/${model}`);
  }
  const selected = aggregateUsage(modelRows, { start: date, end: date }, '', 'gpt-5');
  const buckets = aggregateIntraday(intraday, date, '', 'gpt-5');
  assert.deepEqual(buckets[0].agg, modelRows[0].agg);
  assert.deepEqual(buckets[1].agg, modelRows[1].agg);
  assert.equal(buckets[11].agg.total, 0);
  assert.equal(seriesMatchesTotal(aggregateIntraday(intraday.slice(1), date, '', 'gpt-5'), selected.total, selected.unpriced), false);
  for (const key of ['cost', 'cost_reported', 'cost_estimated']) {
    const changed = buckets.map((bucket, index) => index === 0 ? { ...bucket, agg: { ...bucket.agg, [key]: bucket.agg[key] + 0.01 } } : bucket);
    assert.equal(seriesMatchesTotal(changed, selected.total, selected.unpriced), false, key);
  }
  assert.equal(seriesMatchesTotal(buckets, selected.total, 1), false);
});

test('model selection keeps failed collectors distinct from a successful empty collection', () => {
  const bounds = { start: '2026-10-15', end: '2026-10-15' };
  const sources = [{ client: 'codex', error: null, records: 3 }, { client: 'opencode', error: 'cannot read records', records: 0 }];
  const selected = aggregateUsage(modelRows, bounds, '', 'gpt-5');
  assert.deepEqual(usageCoverage(sources, '', selected), { state: 'partial', failed: ['opencode'] });
  const missing = aggregateUsage(modelRows, bounds, '', 'absent');
  assert.deepEqual(usageCoverage(sources, '', missing), { state: 'unknown', failed: ['opencode'] });
  assert.deepEqual(usageCoverage(sources, 'codex', aggregateUsage(modelRows, bounds, 'codex', 'absent')), { state: 'empty', failed: [] });
  assert.deepEqual(usageCoverage(sources.map((source) => ({ ...source, error: null })), '', missing), { state: 'empty', failed: [] });
});

test('model distribution sorts by tokens, usage record count and supplied name', () => {
  const rows = [['z-model', 8, 1], ['b-model', 4, 2], ['a-model', 4, 2], ['c-model', 4, 1]].map(([model, total, messages]) => ({
    date: '2026-10-15', client: 'codex', model, raw_model: model, unpriced_messages: 0, agg: agg(total, messages, 0),
  }));
  const summary = aggregateUsage(rows, { start: '2026-10-15', end: '2026-10-15' });
  assert.deepEqual(summary.byModel.map((row) => row.model), ['z-model', 'a-model', 'b-model', 'c-model']);
});

test('canonical model names come from native rows and raw names remain exact', () => {
  const rows = [...modelRows, {
    date: '2026-10-15', client: 'codex', model: 'gpt-5-latest', raw_model: '  gpt-5-latest  ', unpriced_messages: 0, agg: agg(3, 1, 0),
  }];
  const bounds = { start: '2026-10-15', end: '2026-10-15' };
  assert.equal(aggregateUsage(rows, bounds, '', 'gpt-5').total.total, 54);
  const supplied = aggregateUsage(rows, bounds, '', 'gpt-5-latest');
  assert.equal(supplied.total.total, 3);
  assert.deepEqual(supplied.byModel[0].rawModels, ['  gpt-5-latest  ']);
  const unknown = aggregateUsage(rows, bounds, '', 'unknown');
  assert.equal(unknown.total.total, 5);
  assert.deepEqual(unknown.byModel[0].rawModels, ['']);
  assert.equal(unknown.unpriced, 1);
});
