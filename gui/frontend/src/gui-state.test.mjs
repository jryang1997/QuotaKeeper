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
  assert.deepEqual(result.pinnedCards, ['codex']);
  assert.deepEqual(result.providerColors, { codex: '#123abc' });
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
