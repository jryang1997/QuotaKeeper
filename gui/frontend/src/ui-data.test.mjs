import test from 'node:test';
import assert from 'node:assert/strict';
import { fmtTokens } from './ui-data.js';

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
