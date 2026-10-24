import test from 'node:test';
import assert from 'node:assert/strict';
import { aggregatePricing, snapshotFetchAge } from './pricing-data.js';
import { aggregateUsage } from './usage-data.js';

const syntheticRate = (entry, usdPerToken, origin = 'upstream') => ({
  entry, usd_per_token: usdPerToken, origin,
});
const syntheticEstimate = (entry = 'synthetic/model') => ({
  kind: 'estimated', matched_entry: entry, match_method: 'exact',
  family_fallback: false, cache_fill: false,
  input: syntheticRate(entry, 0.000001), output: syntheticRate(entry, 0.000002),
  cache_read: null, cache_write: null,
});
const syntheticRow = (priceBasisId, values = {}) => {
  const { records = 1, reported = 0, estimated = 0, unpriced = 0, ...selection } = values;
  return {
    date: '2026-10-15', client: 'codex', model: 'native-model', raw_model: 'raw/model',
    project: null, price_basis_id: priceBasisId, unpriced_messages: unpriced,
    agg: {
      input: records * 100, output: records * 20, cache_read: 0, cache_write: 0,
      reasoning: 0, total: records * 120, messages: records,
      cost: reported + estimated, cost_reported: reported, cost_estimated: estimated,
      cost_is_estimate: estimated > 0,
    },
    ...selection,
  };
};

test('selected pricing counts source-reported and estimated zeros as priced records', () => {
  const bases = [
    { id: 2, basis: { kind: 'reported' } },
    { id: 8, basis: syntheticEstimate() },
    { id: 13, basis: { kind: 'unpriced', reason: 'unknown_model', attempted: null } },
  ];
  const rows = [
    syntheticRow(2),
    syntheticRow(8),
    syntheticRow(8, { records: 2, estimated: 0.0017 }),
    syntheticRow(13, { records: 2, unpriced: 2 }),
  ];
  const result = aggregatePricing(rows, bases);
  assert.equal(result.valid, true);
  assert.deepEqual(result.reported, { records: 1, amount: 0 });
  assert.deepEqual(result.estimated, { records: 3, amount: 0.0017 });
  assert.deepEqual(result.unpriced, { records: 2 });
  assert.equal(result.reported.records + result.estimated.records + result.unpriced.records, 6);
  assert.equal(result.amount, 0.0017);
  assert.equal(aggregatePricing([rows[0]], bases).amount, 0);
  assert.equal(aggregatePricing([rows[1]], bases).amount, 0);
});

test('amount evidence uses the exact selected rows and preserves original nonconsecutive basis IDs', () => {
  const bases = [
    { id: 2, basis: { kind: 'reported' } },
    { id: 17, basis: syntheticEstimate('synthetic/unused') },
    {
      id: 29,
      basis: {
        ...syntheticEstimate('synthetic/channel'), match_method: 'explicit_alias',
        family_fallback: true, cache_fill: true,
        cache_read: syntheticRate('synthetic/cache-fill', 0.0000001, 'supplement'),
        cache_write: syntheticRate('synthetic/old-cache', 0.00000125, 'legacy_unknown'),
      },
    },
  ];
  const rows = [
    syntheticRow(29, { model: 'group/supplied', raw_model: 'Legacy-Alias', records: 2, estimated: 0.003 }),
    syntheticRow(2, { model: 'group/supplied', reported: 0.01 }),
    syntheticRow(17, { model: 'group/supplied', date: '2026-10-14', estimated: 9 }),
    syntheticRow(17, { model: 'group/supplied', client: 'omp', estimated: 8 }),
    syntheticRow(17, { model: 'other', estimated: 7 }),
  ];
  const summary = aggregateUsage(rows, { start: '2026-10-15', end: '2026-10-15' }, 'codex', 'group/supplied');
  const pricing = aggregatePricing(summary.selectedRows, bases);
  assert.equal(summary.total.messages, 3);
  assert.equal(pricing.amount, 0.013000000000000001);
  assert.deepEqual(pricing.bases.map(({ id, records, models, rawModels, clients }) => ({
    id, records, models, rawModels, clients,
  })), [
    { id: 2, records: 1, models: ['group/supplied'], rawModels: ['raw/model'], clients: ['codex'] },
    { id: 29, records: 2, models: ['group/supplied'], rawModels: ['Legacy-Alias'], clients: ['codex'] },
  ]);
  assert.deepEqual(pricing.bases[1].basis, {
    kind: 'estimated', matched_entry: 'synthetic/channel', match_method: 'explicit_alias',
    family_fallback: true, cache_fill: true,
    input: { entry: 'synthetic/channel', usd_per_token: 0.000001, origin: 'upstream' },
    output: { entry: 'synthetic/channel', usd_per_token: 0.000002, origin: 'upstream' },
    cache_read: { entry: 'synthetic/cache-fill', usd_per_token: 0.0000001, origin: 'supplement' },
    cache_write: { entry: 'synthetic/old-cache', usd_per_token: 0.00000125, origin: 'legacy_unknown' },
  });
});

test('all missing prices have no amount and retain the attempted rate and missing reason', () => {
  const { kind, ...attempted } = syntheticEstimate('synthetic/cache-missing');
  const bases = [{
    id: 31, basis: { kind: 'unpriced', reason: 'missing_cache_read', attempted },
  }];
  const result = aggregatePricing([syntheticRow(31, { records: 2, unpriced: 2 })], bases);
  assert.equal(result.amount, null);
  assert.deepEqual(result.unpriced, { records: 2 });
  assert.deepEqual(result.estimated, { records: 0, amount: 0 });
  assert.equal(result.bases[0].amount, null);
  assert.equal(result.bases[0].basis.reason, 'missing_cache_read');
  assert.equal(result.bases[0].basis.attempted.matched_entry, 'synthetic/cache-missing');
  assert.equal(result.bases[0].basis.attempted.cache_read, null);
  const empty = aggregatePricing([], bases);
  assert.equal(empty.amount, 0);
  assert.deepEqual(empty.bases, []);
  assert.equal(empty.reported.records + empty.estimated.records + empty.unpriced.records, 0);
});

test('unresolvable or incompatible basis IDs withhold the entire selected amount evidence', () => {
  const valid = { id: 2, basis: { kind: 'reported' } };
  for (const bases of [
    [valid],
    [valid, { id: 41, basis: { kind: 'unsupported' } }],
    [valid, { id: 41, basis: syntheticEstimate() }, { id: 41, basis: syntheticEstimate() }],
  ]) {
    const result = aggregatePricing([syntheticRow(2, { reported: 1 }), syntheticRow(41)], bases);
    assert.equal(result.valid, false);
    assert.equal(result.amount, null);
    assert.deepEqual(result.bases, []);
    assert.equal(result.reported.records + result.estimated.records + result.unpriced.records, 0);
  }
});

test('snapshot fetch age uses the frozen load time and never turns unknown or future fetches into fresh reads', () => {
  const syntheticSnapshot = { loaded_at_ms: 1792076400000, fetched_at_ms: 1792069200000, cache_state: 'fresh' };
  assert.equal(snapshotFetchAge(syntheticSnapshot), 7_200_000);
  assert.equal(snapshotFetchAge({ ...syntheticSnapshot, fetched_at_ms: null }), null);
  assert.equal(snapshotFetchAge({ ...syntheticSnapshot, fetched_at_ms: 1792076400001 }), null);
  assert.equal(snapshotFetchAge({ ...syntheticSnapshot, cache_state: 'unknown_age' }), null);
  assert.equal(snapshotFetchAge({ ...syntheticSnapshot, loaded_at_ms: 9e15 }), null);
});