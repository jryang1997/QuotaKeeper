import test from 'node:test';
import assert from 'node:assert/strict';
import { filterCommands, makeCommand, moveIndex } from './command-palette.js';

const commands = [
  makeCommand({ id: 'page-quota', group: 'page', label: '配额', keywords: 'quota quota-card' }),
  makeCommand({ id: 'page-usage', group: 'page', label: '用量', keywords: 'usage' }),
  makeCommand({ id: 'module-heat', group: 'module', label: '热力图 · 26W', keywords: 'heat' }),
  makeCommand({ id: 'provider-codex', group: 'provider', label: 'Codex', hint: '用量' }),
];

test('empty query keeps the original order', () => {
  assert.deepEqual(filterCommands(commands, '   ').map((entry) => entry.id), commands.map((entry) => entry.id));
});

test('prefix hits outrank contains hits and keywords', () => {
  assert.deepEqual(filterCommands(commands, 'co').map((entry) => entry.id), ['provider-codex']);
  const usage = filterCommands(commands, '用量').map((entry) => entry.id);
  assert.equal(usage[0], 'page-usage', 'label prefix wins');
  assert.ok(usage.includes('provider-codex'), 'hint match still listed');
});

test('keyword search finds latin ids for chinese labels', () => {
  assert.deepEqual(filterCommands(commands, 'heat').map((entry) => entry.id), ['module-heat']);
  assert.deepEqual(filterCommands(commands, 'quota').map((entry) => entry.id), ['page-quota']);
  assert.deepEqual(filterCommands(commands, 'nope'), []);
});

test('index movement wraps in both directions', () => {
  assert.equal(moveIndex(0, 1, 3), 1);
  assert.equal(moveIndex(2, 1, 3), 0);
  assert.equal(moveIndex(0, -1, 3), 2);
  assert.equal(moveIndex(4, 1, 0), 0);
});
