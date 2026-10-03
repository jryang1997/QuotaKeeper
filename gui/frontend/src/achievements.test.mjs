import test from 'node:test';
import assert from 'node:assert/strict';
import { ACHIEVEMENTS, applyUnlocks, evaluateAchievements, ledgerFacts, readAchievementState, saveAchievementState } from './achievements.js';

const agg = (total, messages, cost, cacheRead = 0) => ({ input: total - cacheRead, output: 0, cache_read: cacheRead, cache_write: 0, reasoning: 0, total, messages, cost });

function ledgerWith(days) {
  return {
    daily_totals: days,
    weekday_hour: Array.from({ length: 7 }, () => Array(24).fill(0)),
    weekday: Array(7).fill(0),
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
