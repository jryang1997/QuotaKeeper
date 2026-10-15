import test from 'node:test';
import assert from 'node:assert/strict';
import { fetchExchangeRate, formatCurrency, formatTokenRate, pricingCurrency } from './currency.js';

test('converts only with a known rate and keeps USD as the source currency', () => {
  assert.match(formatCurrency(10, { code: 'CNY', rate: 7 }), /70/);
  assert.equal(formatCurrency(10, { code: 'CNY', rate: null }), '—');
  assert.match(formatCurrency(10, { code: 'USD' }), /10/);
});

test('caches validated rates and reuses them without another request', async () => {
  const values = new Map();
  const storage = { getItem: (key) => values.get(key), setItem: (key, value) => values.set(key, value) };
  let calls = 0;
  const fetcher = async () => {
    calls++;
    return { ok: true, json: async () => ({ date: '2026-09-29', base: 'USD', quote: 'CNY', rate: 6.705 }) };
  };
  const first = await fetchExchangeRate('CNY', storage, fetcher);
  const second = await fetchExchangeRate('CNY', storage, fetcher);
  assert.equal(first.rate, 6.705);
  assert.deepEqual(second, first);
  assert.equal(calls, 1);
});

test('per-million-token rates retain tiny positive values, exact zero and an honest display currency', () => {
  assert.equal(formatTokenRate(0.000001, { code: 'USD' }), 'USD 1');
  assert.equal(formatTokenRate(0.000001, { code: 'CNY', rate: 7 }), 'CNY 7');
  assert.equal(formatTokenRate(0.00000125, { code: 'USD' }), 'USD 1.25');
  assert.equal(formatTokenRate(1e-15, { code: 'USD' }), 'USD 1E-9');
  assert.equal(formatTokenRate(0, { code: 'CNY', rate: 7 }), 'CNY 0');
  assert.equal(formatTokenRate(null, { code: 'USD' }), '—');
  for (const rate of [undefined, null, 0, -1, NaN, Infinity]) {
    assert.deepEqual(pricingCurrency({ code: 'CNY', rate }), { code: 'USD', rate: 1 });
    assert.equal(formatTokenRate(0.000001, { code: 'CNY', rate }), 'USD 1');
  }
});
