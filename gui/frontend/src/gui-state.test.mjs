import test from 'node:test';
import assert from 'node:assert/strict';
import { readGuiPreferences, saveGuiPreferences } from './gui-preferences.js';
import { registerNativeCallbacks, sendCommand } from './gui-bridge.js';

test('preferences recover from invalid values and keep valid provider choices', () => {
  const store = {
    getItem: () => JSON.stringify({
      lang: 1, theme: 'unknown', pinnedCards: ['codex', 'not-a-provider'],
      providerColors: { codex: '#123abc', cursor: 'red' },
    }),
  };
  const result = readGuiPreferences(store);
  assert.equal(result.lang, 1);
  assert.equal(result.theme, 'system');
  assert.equal(result.currency, 'USD');
  assert.equal(result.quotaRemainingView, false);
  assert.deepEqual(result.pinnedCards, ['codex']);
  assert.deepEqual(result.providerColors, { codex: '#123abc' });
});

test('cassette theme preference is persisted and validated', () => {
  const store = {
    getItem: () => JSON.stringify({ theme: 'cassette' }),
  };
  const result = readGuiPreferences(store);
  assert.equal(result.theme, 'cassette');
});

test('premium theme preferences persist and invalid names are rejected', () => {
  for (const theme of ['washi', 'palace', 'aurora', 'amber', 'mocha']) {
    const result = readGuiPreferences({ getItem: () => JSON.stringify({ theme }) });
    assert.equal(result.theme, theme);
  }
  const rejected = readGuiPreferences({ getItem: () => JSON.stringify({ theme: 'vaporwave' }) });
  assert.equal(rejected.theme, 'system');
});

test('quota remaining view persists only when explicitly true', () => {
  const truthy = { getItem: () => JSON.stringify({ quotaRemainingView: true }) };
  const falsy = { getItem: () => JSON.stringify({ quotaRemainingView: 'yes' }) };
  assert.equal(readGuiPreferences(truthy).quotaRemainingView, true);
  assert.equal(readGuiPreferences(falsy).quotaRemainingView, false);
  assert.equal(readGuiPreferences({ getItem: () => null }).quotaRemainingView, false);
});

test('preference writes can fail without losing in-memory choice', () => {
  const next = { theme: 'dark' };
  assert.equal(saveGuiPreferences(next, { setItem: () => { throw new Error('disabled'); } }), next);
});

test('bridge serializes commands and unregisters only its own callbacks', () => {
  const sent = [];
  const host = { ipc: { postMessage: (message) => sent.push(message) } };
  const callback = () => 'first';
  const unregister = registerNativeCallbacks({ __onPush: callback }, host);
  sendCommand({ cmd: 'refresh' }, host);
  assert.deepEqual(sent.map(JSON.parse), [{ cmd: 'refresh' }]);
  assert.equal(host.__onPush(), 'first');
  host.__onPush = () => 'newer';
  unregister();
  assert.equal(host.__onPush(), 'newer');
});
