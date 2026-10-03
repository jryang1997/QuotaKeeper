export const PANEL_COLS = 12;
export const PANEL_ROW_H = 110;
export const PANEL_GAP = 14;
export const PANELS_KEY = 'token-monitor.panels.v1';
export const PANELS_MAX = 20;

// 用量页模块目录(2026-10 合并原"面板"页):几何只在这里定义,
// 标题/描述/内容由 UsageDashboard 按语言装配。w/h 是默认落位与添加时的尺寸。
export const PANEL_CATALOG = [
  { id: 'tokens', w: 3, h: 2, minW: 3, minH: 1 },
  { id: 'requests', w: 3, h: 2, minW: 3, minH: 1 },
  { id: 'cost', w: 3, h: 2, minW: 3, minH: 1 },
  { id: 'value', w: 3, h: 2, minW: 3, minH: 1 },
  { id: 'trend', w: 12, h: 4, minW: 6, minH: 3 },
  { id: 'achievements', w: 12, h: 3, minW: 6, minH: 2 },
  { id: 'cache', w: 4, h: 2, minW: 3, minH: 2 },
  { id: 'speed', w: 8, h: 2, minW: 4, minH: 2 },
  { id: 'models', w: 6, h: 4, minW: 4, minH: 3 },
  { id: 'modelMix', w: 6, h: 4, minW: 4, minH: 3 },
  { id: 'heat', w: 12, h: 4, minW: 6, minH: 3 },
  { id: 'punch', w: 8, h: 2, minW: 5, minH: 2 },
  { id: 'week', w: 4, h: 2, minW: 3, minH: 2 },
  { id: 'clock', w: 4, h: 3, minW: 3, minH: 3 },
  { id: 'sources', w: 4, h: 3, minW: 3, minH: 2 },
  { id: 'projects', w: 4, h: 3, minW: 3, minH: 2 },
  { id: 'sessions', w: 8, h: 3, minW: 4, minH: 2 },
  { id: 'alltime', w: 4, h: 1, minW: 3, minH: 1 },
  { id: 'streak', w: 4, h: 1, minW: 3, minH: 1 },
  { id: 'records', w: 4, h: 1, minW: 3, minH: 1 },
];

export const DEFAULT_PANELS = [
  { id: 'tokens', x: 0, y: 0, w: 3, h: 1, locked: false },
  { id: 'requests', x: 3, y: 0, w: 3, h: 1, locked: false },
  { id: 'cost', x: 6, y: 0, w: 3, h: 1, locked: false },
  { id: 'value', x: 9, y: 0, w: 3, h: 1, locked: false },
  { id: 'trend', x: 0, y: 1, w: 12, h: 4, locked: false },
  { id: 'achievements', x: 0, y: 5, w: 12, h: 3, locked: false },
  { id: 'cache', x: 0, y: 8, w: 4, h: 3, locked: false },
  { id: 'speed', x: 4, y: 8, w: 8, h: 3, locked: false },
  { id: 'models', x: 0, y: 11, w: 6, h: 4, locked: false },
  { id: 'modelMix', x: 6, y: 11, w: 6, h: 4, locked: false },
  { id: 'heat', x: 0, y: 15, w: 12, h: 4, locked: false },
  { id: 'punch', x: 0, y: 19, w: 8, h: 2, locked: false },
  { id: 'week', x: 8, y: 19, w: 4, h: 2, locked: false },
  { id: 'alltime', x: 0, y: 21, w: 4, h: 1, locked: false },
  { id: 'streak', x: 4, y: 21, w: 4, h: 1, locked: false },
  { id: 'records', x: 8, y: 21, w: 4, h: 1, locked: false },
  { id: 'projects', x: 0, y: 22, w: 4, h: 3, locked: false },
  { id: 'sessions', x: 4, y: 22, w: 8, h: 3, locked: false },
];

export const panelOverlap = (a, b) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
export const panelOverlapArea = (a, b) =>
  Math.max(0, Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x)) *
  Math.max(0, Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y));

/* 锁定卡是钉子,其余自由卡按原序落到不重叠的最小行 */
export function compactPanels(panels) {
  const locked = panels.filter((p) => p.locked);
  const free = panels.filter((p) => !p.locked).sort((a, b) => a.y - b.y || a.x - b.x);
  const placed = [...locked];
  for (const p of free) {
    let y = 0;
    while (y < 400 && placed.some((q) => panelOverlap({ ...p, y }, q))) y++;
    placed.push({ ...p, y });
  }
  return placed.sort((a, b) => a.y - b.y || a.x - b.x);
}

export function resolvePanelMove(panels, id, target) {
  const origin = panels.find((p) => p.id === id);
  const others = panels.filter((p) => p.id !== id);
  const overlapping = others.filter((p) => panelOverlap(target, p) && !p.locked);
  if (overlapping.length === 0) return compactPanels([target, ...others]);
  const first = overlapping.reduce((best, p) => (panelOverlapArea(target, p) > panelOverlapArea(target, best) ? p : best), overlapping[0]);
  const swapped = { ...first, x: origin.x, y: origin.y };
  const rest = others.filter((p) => p.id !== first.id);
  return compactPanels([target, swapped, ...rest]);
}

// Locked panels stay fixed; constrain a locked resize before compacting free panels.
export function resolvePanelResize(panels, id, target) {
  if (!target.locked) return target;
  const locked = panels.filter((panel) => panel.id !== id && panel.locked);
  let { w, h } = target;
  while (w > 1 && locked.some((panel) => panelOverlap({ ...target, w, h }, panel))) w--;
  while (h > 1 && locked.some((panel) => panelOverlap({ ...target, w, h }, panel))) h--;
  return { ...target, w, h };
}

export function collectAlignLines(panels, id, nx, ny) {
  const v = [];
  const h = [];
  panels.forEach((p) => {
    if (p.id === id) return;
    if (p.x === nx) v.push(p.x);
    if (p.x + p.w === nx) v.push(p.x + p.w);
    if (p.y === ny) h.push(p.y);
    if (p.y + p.h === ny) h.push(p.y + p.h);
  });
  return { v: [...new Set(v)], h: [...new Set(h)] };
}

// Invalid or outdated layouts fall back to defaults instead of rendering off-grid cards.
export function readPanelState(storage = globalThis.localStorage, key = PANELS_KEY, catalog = PANEL_CATALOG) {
  try {
    const raw = JSON.parse(storage.getItem(key) || 'null');
    if (!raw || !Array.isArray(raw.panels) || !Array.isArray(raw.enabled)) return null;
    if (raw.panels.length > PANELS_MAX || raw.enabled.length > PANELS_MAX) return null;
    const known = new Set(catalog.map((c) => c.id));
    const specs = new Map(catalog.map((c) => [c.id, c]));
    const panelIds = new Set();
    for (const p of raw.panels) {
      if (!p || !known.has(p.id) || panelIds.has(p.id)
        || !Number.isInteger(p.x) || p.x < 0
        || !Number.isInteger(p.y) || p.y < 0 || p.y > 400
        || !Number.isInteger(p.w) || p.w < (specs.get(p.id)?.minW || 1) || p.x + p.w > PANEL_COLS
        || !Number.isInteger(p.h) || p.h < (specs.get(p.id)?.minH || 1) || p.h > 10
        || typeof p.locked !== 'boolean') return null;
      panelIds.add(p.id);
    }
    if (raw.enabled.some((id) => !panelIds.has(id)) || new Set(raw.enabled).size !== raw.enabled.length) return null;
    if (raw.panels.some((p, i) => raw.panels.slice(i + 1).some((q) => panelOverlap(p, q)))) return null;
    return { panels: raw.panels, enabled: raw.enabled };
  } catch {
    return null;
  }
}

export function savePanelState(panels, enabled, storage = globalThis.localStorage, key = PANELS_KEY) {
  try {
    storage.setItem(key, JSON.stringify({ panels, enabled }));
  } catch { /* Layout remains available for this session. */ }
}

// Keep the selected module exactly where it was placed; move only obstructing free modules.
export function resolveModuleLayout(panels, id, target) {
  const fixed = panels.filter((panel) => panel.id !== id && panel.locked);
  if (fixed.some((panel) => panelOverlap(target, panel))) return panels;
  const placed = [...fixed, target];
  for (const panel of panels.filter((item) => item.id !== id && !item.locked).sort((a, b) => a.y - b.y || a.x - b.x)) {
    let candidate = { ...panel };
    let collisions;
    while ((collisions = placed.filter((item) => panelOverlap(candidate, item))).length) {
      candidate.y = Math.max(...collisions.map((item) => item.y + item.h));
      if (candidate.y > 400) return panels;
    }
    placed.push(candidate);
  }
  return placed.sort((a, b) => a.y - b.y || a.x - b.x);
}
