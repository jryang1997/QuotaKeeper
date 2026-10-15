import test from 'node:test';
import assert from 'node:assert/strict';
import { fmtTokens } from './ui-data.js';
import * as uiData from './ui-data.js';

test('fmtTokens keeps the western ladder by default', () => {
  assert.equal(fmtTokens(856), '856');
  assert.equal(fmtTokens(12_340), '12.3K');
  assert.equal(fmtTokens(4_500_000), '4.50M');
});

test('fmtTokens switches to 亿/万 in Chinese', () => {
  assert.equal(fmtTokens(133_000_000, true), '1.33亿');
  assert.equal(fmtTokens(681_000, true), '68.1万');
  assert.equal(fmtTokens(1500, true), '1500');
  assert.equal(fmtTokens(null, true), '—');
});

function protocolPayload() {
  const agg = { input: 63, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, messages: 1, cost: 0, cost_reported: 0, cost_estimated: 0, cost_is_estimate: false, total: 63 };
  const row = { date: '2026-10-15', client: 'codex', project: null, model: 'unknown', raw_model: '', price_basis_id: 7, agg, unpriced_messages: 1 };
  return {
    cards: [{ marker: 'quota-kept' }],
    ledger: {
      schema_version: 1, report_id: 'report-a', timezone_offset: '+08:00', generated_at_ms: 123,
      price_snapshot: { version: 'prices-a', loaded_at_ms: 123, fetched_at_ms: null, dataset_url: null, source: 'supplements_only', cache_state: 'not_loaded' },
      price_bases: [{ id: 7, basis: { kind: 'unpriced', reason: 'unknown_model', attempted: null } }],
      sources: [{ client: 'codex', label: 'Codex CLI', records: 1, error: null }],
      today: agg, week: agg, month: agg, analysis_rows: [row], intraday_rows: [{ ...row, hour: 12 }],
    },
  };
}

test('wire entrypoint withholds old absent-model ledgers rather than fabricating unknown', () => {
  const payload = protocolPayload();
  assert.equal(uiData.readLedger(payload), payload.ledger);
  assert.equal(uiData.readLedger({ ...payload, stale: true }), payload.ledger);
  assert.equal(uiData.readLedger(payload).analysis_rows[0].agg.total, 63);
  assert.equal(uiData.readLedger(payload).analysis_rows[0].model, 'unknown');
  for (const remove of [
    (ledger) => { delete ledger.schema_version; },
    (ledger) => { ledger.schema_version = 0; },
    (ledger) => { delete ledger.report_id; },
    (ledger) => { delete ledger.timezone_offset; },
    (ledger) => { delete ledger.price_snapshot.fetched_at_ms; },
    (ledger) => { delete ledger.analysis_rows[0].model; },
    (ledger) => { delete ledger.analysis_rows[0].raw_model; },
    (ledger) => { delete ledger.analysis_rows[0].project; },
    (ledger) => { delete ledger.intraday_rows[0].project; },
    (ledger) => { ledger.analysis_rows[0].price_basis_id = 999; },
    (ledger) => { ledger.price_bases.push(ledger.price_bases[0]); },
    (ledger) => { ledger.price_bases[0].basis = { kind: 'estimated' }; },
    (ledger) => { ledger.price_bases[0].basis = { kind: 'unpriced', reason: 'unknown_model' }; },
  ]) {
    const legacy = structuredClone(payload);
    remove(legacy.ledger);
    const before = structuredClone(legacy);
    assert.equal(uiData.readLedger(legacy), null);
    assert.deepEqual(legacy, before);
    assert.deepEqual(legacy.cards, [{ marker: 'quota-kept' }]);
  }
  for (const payload of [null, {}, { ledger: null }, { ledger: [] }]) assert.equal(uiData.readLedger(payload), null);
});
