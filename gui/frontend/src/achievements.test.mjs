import test from 'node:test';
import assert from 'node:assert/strict';
import { ACHIEVEMENTS, applyUnlocks, evaluateAchievements, ledgerFacts, readAchievementState, saveAchievementState, unlockedNonMetaCount } from './achievements.js';

const agg = (total, messages, cost, cacheRead = 0) => ({ input: total - cacheRead, output: 0, cache_read: cacheRead, cache_write: 0, reasoning: 0, total, messages, cost });

function ledgerWith(days) {
  return {
    daily_totals: days,
    weekday_hour: Array.from({ length: 7 }, () => Array(24).fill(0)),
    weekday: Array(7).fill(0),
    hourly: Array(24).fill(0),
    sources: [],
    by_model: [],
    by_project: [],
    by_session: [],
    month_by_client: [],
    month: agg(0, 0, 0),
  };
}

test('facts derive streaks, totals and best day from the ledger only', () => {
  const days = [
    { date: '2026-09-01', tokens: 10, cost: 0.1, messages: 1 },
    { date: '2026-09-02', tokens: 0, cost: 0, messages: 0 },
    { date: '2026-09-03', tokens: 50, cost: 0.5, messages: 3 },
    { date: '2026-09-04', tokens: 90, cost: 0.9, messages: 5 },
  ];
  const facts = ledgerFacts(ledgerWith(days));
  assert.equal(facts.tokens, 150);
  assert.equal(facts.messages, 9);
  assert.equal(facts.activeDays, 3);
  assert.equal(facts.bestStreak, 2);
  assert.equal(facts.currentStreak, 2);
  assert.equal(facts.bestDayTokens, 90);
  assert.equal(facts.busiestDayMessages, 5);
});

test('achievements unlock on threshold and report progress below it', () => {
  const list = evaluateAchievements(ledgerWith([{ date: '2026-09-04', tokens: 5_000, cost: 0.5, messages: 12 }]));
  const byId = Object.fromEntries(list.map((item) => [item.id, item]));
  assert.equal(byId['first-light'].unlocked, true);
  assert.equal(byId['calls-1k'].unlocked, false);
  assert.ok(byId['calls-1k'].pct > 0 && byId['calls-1k'].pct < 1);
  assert.equal(byId['cache-60'].unlocked, false, 'cache gate needs a 1M prompt sample');
  assert.equal(list.length, ACHIEVEMENTS.length);
});

test('unlocks are recorded once and keep their first timestamp', () => {
  const list = evaluateAchievements(ledgerWith([{ date: '2026-09-04', tokens: 1, cost: 0, messages: 1 }]));
  const first = applyUnlocks(list, { unlocked: {} }, 1_000);
  assert.deepEqual(first.fresh, ['first-light']);
  const second = applyUnlocks(list, first.state, 2_000);
  assert.deepEqual(second.fresh, []);
  assert.equal(second.state.unlocked['first-light'], 1_000);
});

test('achievement state survives damaged storage', () => {
  const broken = { getItem: () => '{not json', setItem: () => {} };
  assert.deepEqual(readAchievementState(broken), { unlocked: {} });
  const values = new Map();
  const memory = { getItem: (key) => values.get(key), setItem: (key, value) => values.set(key, value) };
  saveAchievementState({ unlocked: { 'first-light': 5 } }, memory);
  assert.deepEqual(readAchievementState(memory).unlocked, { 'first-light': 5 });
});

test('fun facts cover sessions, projects, hours and sources', () => {
  const ledger = ledgerWith([{ date: '2026-09-04', tokens: 10, cost: 0.1, messages: 2 }]);
  ledger.by_session = [
    { agg: { total: 900, messages: 1_200 } },
    { agg: { total: 100, messages: 40 } },
  ];
  ledger.by_project = [
    { agg: { total: 800 } },
    { agg: { total: 100 } },
    { agg: { total: 100 } },
  ];
  ledger.hourly = Array.from({ length: 24 }, (_, hour) => (hour < 18 ? 5 : 0));
  ledger.sources = [
    { client: 'codex', records: 3 },
    { client: 'claude', records: 0 },
    { client: 'dsh', records: 7, error: 'x' },
  ];
  ledger.weekday_hour = Array.from({ length: 7 }, () => { const row = Array(24).fill(0); row[12] = 30; row[19] = 70; return row; });
  const facts = ledgerFacts(ledger);
  assert.equal(facts.topSessionMessages, 1_200);
  assert.equal(facts.topProjectShare, 0.8);
  assert.equal(facts.hourCoverage, 18);
  assert.equal(facts.sourceCount, 2);
  assert.equal(facts.lunchTokens, 210);
  assert.equal(facts.eveningTokens, 490);

  const byId = Object.fromEntries(evaluateAchievements(ledger).map((item) => [item.id, item]));
  assert.equal(byId['long-haul'].unlocked, true);
  assert.equal(byId['one-trick'].unlocked, true, '80% share with 3 projects');
  assert.equal(byId['socializer'].unlocked, false, 'only 3 projects');
  assert.equal(byId['round-the-clock'].pct > 0.7 && byId['round-the-clock'].unlocked, false);
  assert.equal(byId['full-house'].unlocked, false, '2 of 6 sources');
});

test('meta achievements count non-meta unlocks and platinum targets all of them', () => {
  const empty = evaluateAchievements(ledgerWith([]), { unlockedCount: 0 });
  const byIdEmpty = Object.fromEntries(empty.map((item) => [item.id, item]));
  assert.equal(byIdEmpty.collector.unlocked, false);
  assert.equal(byIdEmpty.platinum.target, ACHIEVEMENTS.filter((def) => !def.meta).length);
  assert.equal(byIdEmpty.platinum.unlocked, false);

  const partial = Object.fromEntries(ACHIEVEMENTS.filter((def) => !def.meta).slice(0, 10).map((def) => [def.id, 1]));
  const mid = evaluateAchievements(ledgerWith([]), { unlockedCount: unlockedNonMetaCount({ unlocked: partial }) });
  const byIdMid = Object.fromEntries(mid.map((item) => [item.id, item]));
  assert.equal(byIdMid.collector.unlocked, true);
  assert.equal(byIdMid.completionist.unlocked, false);
  assert.equal(byIdMid.platinum.unlocked, false, 'meta unlocks must not count toward platinum');
});

test('extras feed money and interaction achievements', () => {
  const byId = Object.fromEntries(evaluateAchievements(ledgerWith([]), {
    monthlyValue: { list_price_multiple: 12.5, list_price_benchmark_usd: 20 },
    counters: { palette: 50, share: 2, lang: 10, overlay: 1 },
  }).map((item) => [item.id, item]));
  assert.equal(byId['break-even'].unlocked, true);
  assert.equal(byId['x10-value'].unlocked, true);
  assert.equal(byId['keyboard-flow'].unlocked, true);
  assert.equal(byId['polyglot'].unlocked, true);
  assert.equal(byId['desk-anchor'].unlocked, true);
  assert.equal(byId['card-sharer'].unlocked, false);
  const noBenchmark = Object.fromEntries(evaluateAchievements(ledgerWith([]), {
    monthlyValue: { list_price_multiple: 99, list_price_benchmark_usd: 0 },
  }).map((item) => [item.id, item]));
  assert.equal(noBenchmark['break-even'].unlocked, false, 'no benchmark, no unlock');
});
