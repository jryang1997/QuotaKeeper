import test from 'node:test';
import assert from 'node:assert/strict';
import { arrangeQuota, formatObservedAt, projectExpiry, projectWindow } from './quota-data.js';

const DAY = 86_400_000;
const observedAtMs = 1_700_000_000_000;

test('20% used reads as 80 remaining and color still follows used', () => {
  const context = { observedAtMs, now: observedAtMs, stale: false, remainingView: true };
  const limit = { label: '5h', used_pct: 20, resets_in_ms: 3 * DAY, status: 'ok' };
  const remaining = projectWindow(limit, context);
  assert.equal(remaining.percent, 80);
  assert.equal(remaining.usedPct, 20);
  assert.equal(remaining.tone, 'ok');
  assert.equal(projectWindow(limit, { ...context, remainingView: undefined }).percent, 80);
  assert.equal(projectWindow(limit, { ...context, remainingView: false }).percent, 20);

  const tight = projectWindow({ ...limit, used_pct: 80 }, context);
  assert.equal(tight.percent, 20);
  assert.equal(tight.tone, 'warning');
});

test('a 3 day reset aged by 2 days keeps 1 day', () => {
  const context = { observedAtMs, now: observedAtMs + 2 * DAY, stale: false, remainingView: true };
  const limit = { label: 'week', used_pct: 20, resets_in_ms: 3 * DAY };
  const first = projectWindow(limit, context);
  assert.equal(first.reset.state, 'countdown');
  assert.equal(first.reset.remainingMs, DAY);
});

test('a reset instant that has passed waits for a new reading without refilling', () => {
  const window = projectWindow(
    { label: '5h', used_pct: 20, resets_in_ms: DAY },
    { observedAtMs, now: observedAtMs + 2 * DAY, stale: false, remainingView: true },
  );
  assert.equal(window.reset.state, 'awaiting');
  assert.equal(window.reset.remainingMs, null);
  assert.equal(window.percent, 80);
  assert.equal(window.usedPct, 20);
  assert.equal(projectWindow(
    { label: '5h', used_pct: 20, resets_in_ms: 0 },
    { observedAtMs, now: observedAtMs, stale: false, remainingView: false },
  ).reset.state, 'awaiting');
});

test('null or blank used percent is not a live full remaining window', () => {
  const context = { observedAtMs, now: observedAtMs, stale: false, remainingView: true };
  for (const usedPct of [null, undefined, '', '20']) {
    const window = projectWindow({ label: '5h', used_pct: usedPct, resets_in_ms: DAY }, context);
    assert.equal(window.usedPct, null);
    assert.equal(window.percent, null);
    assert.equal(window.tone, null);
  }
  const arranged = arrangeQuota({
    cards: [
      { kind: 'Live', provider: 'goat', title: 'GOAT', method: 'goat_credits', limits: [{ label: '5h', used_pct: null, resets_in_ms: -1 }] },
      { kind: 'Live', provider: 'codex', title: 'Codex', method: 'provider_fraction', limits: [{ label: '5h', used_pct: '', resets_in_ms: DAY }] },
    ],
    observedAtMs,
    now: observedAtMs,
    stale: false,
    remainingView: true,
  });
  assert.equal(arranged.quota.length, 0);
  assert.deepEqual(arranged.empty.map((item) => item.provider), ['goat', 'codex']);
  assert.equal(arranged.empty.every((item) => item.windows.length === 0 && item.peakUsed == null), true);
});

test('a negative reset is awaiting and does not refill the last percent', () => {
  const window = projectWindow(
    { label: '5h', used_pct: 20, resets_in_ms: -1 },
    { observedAtMs, now: observedAtMs, stale: false, remainingView: true },
  );
  assert.equal(window.reset.state, 'awaiting');
  assert.equal(window.reset.remainingMs, null);
  assert.equal(window.percent, 80);
  assert.equal(window.usedPct, 20);
});

test('a stale reset is not a countdown and the last percent stays', () => {
  const window = projectWindow(
    { label: '5h', used_pct: 20, resets_in_ms: 3 * DAY },
    { observedAtMs, now: observedAtMs + 2 * DAY, stale: true, remainingView: true },
  );
  assert.equal(window.percent, 80);
  assert.equal(window.reset.state, 'stale');
  assert.equal(window.reset.remainingMs, null);
});

test('unavailable, empty live, real zero, and activity stay distinct', () => {
  const arranged = arrangeQuota({
    cards: [
      { kind: 'Missing', provider: 'claude', title: 'Claude', hints: ['登录', 'Sign in'] },
      { kind: 'Live', provider: 'kimi', title: 'Kimi', method: 'provider_fraction', limits: [] },
      { kind: 'Live', provider: 'codex', title: 'Codex', method: 'provider_fraction', limits: [{ label: '5h', used_pct: 0, resets_in_ms: DAY, status: 'ok' }] },
      { kind: 'Live', provider: 'antigravity', title: 'Antigravity', method: 'activity_vs_average', limits: [{ label: 'today', used_pct: 90, resets_in_ms: 3 * DAY, status: 'warning' }] },
    ],
    hiddenCards: ['claude'],
    observedAtMs,
    now: observedAtMs,
    stale: false,
    remainingView: true,
  });
  assert.deepEqual(arranged.quota.map((item) => item.provider), ['codex']);
  assert.equal(arranged.quota[0].countsAsQuota, true);
  assert.equal(arranged.quota[0].windows[0].percent, 100);
  assert.equal(arranged.quota[0].windows[0].tone, 'ok');
  assert.deepEqual(arranged.empty.map((item) => item.provider), ['kimi']);
  assert.equal(arranged.empty[0].countsAsQuota, false);
  assert.equal(arranged.empty[0].peakUsed, null);
  assert.equal(arranged.empty[0].windows.length, 0);
  assert.deepEqual(arranged.activity.map((item) => item.provider), ['antigravity']);
  assert.equal(arranged.activity[0].countsAsQuota, false);
  assert.equal(arranged.activity[0].windows[0].percent, 90);
  assert.equal(arranged.activity[0].windows[0].tone, 'activity');
  assert.equal(arranged.activity[0].windows[0].reset.state, 'none');
  assert.equal(arranged.missingCount, 1);
  assert.equal(arranged.folded[0].kind, 'unavailable');
  assert.equal(arranged.folded[0].hidden, true);
});

test('live quota sorts pinned first then highest used, and balance does not change rank', () => {
  const arranged = arrangeQuota({
    cards: [
      { kind: 'Live', provider: 'glm', title: 'GLM', method: 'provider_fraction', balance: { amount: 999, currency: 'CNY' }, limits: [{ label: '5h', used_pct: 10, resets_in_ms: DAY, status: 'ok' }] },
      { kind: 'Live', provider: 'codex', title: 'Codex', method: 'provider_fraction', limits: [{ label: '5h', used_pct: 55, resets_in_ms: DAY, status: 'ok' }, { label: 'week', used_pct: 70, resets_in_ms: 2 * DAY, status: 'ok' }] },
      { kind: 'Live', provider: 'goat', title: 'GOAT', method: 'goat_credits', limits: [{ label: '5h', used_pct: 40, resets_in_ms: DAY, status: 'ok' }] },
      { kind: 'Live', provider: 'cursor', title: 'Cursor', method: 'provider_fraction', limits: [{ label: 'month', used_pct: 95, resets_in_ms: DAY, status: 'exhausted' }] },
    ],
    hiddenCards: ['cursor'],
    pinnedCards: ['glm'],
    observedAtMs,
    now: observedAtMs,
    stale: false,
    remainingView: false,
  });
  assert.deepEqual(arranged.quota.map((item) => item.provider), ['glm', 'codex', 'goat']);
  assert.equal(arranged.quota[1].peakUsed, 70);
  assert.equal(arranged.quota[1].windows.length, 2);
  assert.equal(arranged.hiddenCount, 1);
  assert.equal(arranged.missingCount, 0);
  assert.equal(arranged.folded[0].provider, 'cursor');
});

test('subscription expiry is expired at the instant and is not a quota reset', () => {
  const now = new Date(2026, 9, 15, 12).getTime();
  assert.equal(projectExpiry(now, now).state, 'expired');
  assert.equal(projectExpiry(now - 1, now).state, 'expired');
  const later = projectExpiry(new Date(2026, 9, 20, 12).getTime(), now);
  assert.equal(later.state, 'active');
  assert.equal(later.warning, true);
  assert.equal(later.md, '10-20');
  assert.equal(formatObservedAt(new Date(2026, 9, 15, 8, 5).getTime()), '2026-10-15 08:05');
  assert.equal(formatObservedAt(0), null);
});
