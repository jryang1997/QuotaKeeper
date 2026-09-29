import test from 'node:test';
import assert from 'node:assert/strict';
import {
  DEFAULT_PANELS,
  PANEL_CATALOG,
  readPanelState,
  savePanelState,
  resolvePanelMove,
  resolvePanelResize,
  panelOverlap,
} from './panel-layout.js';

const storage = (state) => ({
  getItem: () => JSON.stringify(state),
  setItem: (_, value) => { state = JSON.parse(value); },
  current: () => state,
});
const defaultState = () => ({
  panels: DEFAULT_PANELS.map((panel) => ({ ...panel })),
  enabled: PANEL_CATALOG.map(({ id }) => id),
});

test('saved default layout can be restored', () => {
  const memory = storage(null);
  const state = defaultState();
  savePanelState(state.panels, state.enabled, memory);
  assert.deepEqual(readPanelState(memory), state);
});

test('damaged layouts fall back instead of placing panels off-grid', () => {
  const cases = [
    (state) => { state.panels[0].x = -1; },
    (state) => { state.panels[0].w = 13; },
    (state) => { state.panels[0].y = 100_000; },
    (state) => { state.panels[0].h = Infinity; },
    (state) => { state.panels.push({ ...state.panels[0] }); },
    (state) => { state.enabled.push(state.enabled[0]); },
    (state) => { state.panels[1].x = 0; state.panels[1].y = 0; },
  ];
  for (const damage of cases) {
    const state = defaultState();
    damage(state);
    assert.equal(readPanelState(storage(state)), null);
  }
});

test('moving one panel keeps distinct panels without overlap', () => {
  const panels = resolvePanelMove(defaultState().panels, 'alltime', {
    ...DEFAULT_PANELS.find((panel) => panel.id === 'alltime'), x: 4, y: 4,
  });
  assert.equal(new Set(panels.map(({ id }) => id)).size, DEFAULT_PANELS.length);
  assert.ok(panels.every((panel, i) => panels.slice(i + 1).every((other) => !panelOverlap(panel, other))));
});

test('resizing a locked panel does not cover another locked panel', () => {
  const panels = [
    { id: 'alltime', x: 0, y: 0, w: 2, h: 1, locked: true },
    { id: 'streak', x: 3, y: 0, w: 2, h: 1, locked: true },
  ];
  const result = resolvePanelResize(panels, 'alltime', { ...panels[0], w: 4 });
  assert.equal(result.w, 3);
  assert.equal(panelOverlap(result, panels[1]), false);
});
