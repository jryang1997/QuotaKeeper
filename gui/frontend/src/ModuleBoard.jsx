import React, { memo, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { AnimatePresence, animate, motion, useMotionValue } from 'motion/react';
import { STR } from './ui-data.js';

import { PANEL_COLS, PANEL_ROW_H, PANEL_GAP, PANELS_MAX, compactPanels, resolveModuleLayout, resolvePanelMove, resolvePanelResize, collectAlignLines, readPanelState, savePanelState } from './panel-layout.js';
import { FADE, LAYOUT_SPRING, panelRect, project, releaseVelocity, rubberband } from './motion-tuning.js';
import { PopoverSurface, useMotionPreference } from './ui-motion.jsx';

const ModuleContent = memo(function ModuleContent({ id, renderModule, modular }) {
  return <div className={`panel-body${modular ? ' module-body' : ''}`}>{renderModule(id)}</div>;
});

// 模块缩略示意:36×22 单色小图形,与真实模块同族(数字砖/柱阵/点阵/环),选择器卡片用。
const PANEL_THUMBS = {
  tokens: <><rect x='4' y='4' width='13' height='2.5' rx='1.25' opacity='0.4' /><rect x='4' y='10' width='26' height='8' rx='2' opacity='0.85' /></>,
  requests: <><rect x='4' y='4' width='13' height='2.5' rx='1.25' opacity='0.4' /><rect x='4' y='10' width='26' height='8' rx='2' opacity='0.85' /></>,
  cost: <><rect x='4' y='4' width='13' height='2.5' rx='1.25' opacity='0.4' /><rect x='4' y='10' width='26' height='8' rx='2' opacity='0.85' /></>,
  value: <><rect x='4' y='4' width='13' height='2.5' rx='1.25' opacity='0.4' /><rect x='4' y='10' width='26' height='8' rx='2' opacity='0.85' /></>,
  trend: <><g opacity='0.85'><rect x='4' y='11' width='4' height='7' rx='1' /><rect x='10' y='8' width='4' height='10' rx='1' /><rect x='16' y='13' width='4' height='5' rx='1' /><rect x='22' y='6' width='4' height='12' rx='1' /><rect x='28' y='10' width='4' height='8' rx='1' /></g><g opacity='0.4'><rect x='4' y='8' width='4' height='3' rx='1' /><rect x='10' y='5' width='4' height='3' rx='1' /><rect x='22' y='3.5' width='4' height='2.5' rx='1' /><rect x='28' y='8' width='4' height='2' rx='1' /></g></>,
  achievements: <><rect x='4' y='6' width='8' height='8' rx='2' opacity='0.85' /><rect x='14' y='6' width='8' height='8' rx='2' opacity='0.4' /><rect x='24' y='6' width='8' height='8' rx='2' opacity='0.4' /><rect x='4' y='17.5' width='28' height='1.5' rx='0.75' opacity='0.4' /></>,
  cache: <><path d='M8 15a10 10 0 0 1 20 0' fill='none' stroke='currentColor' strokeWidth='3' strokeLinecap='round' opacity='0.35' /><path d='M8 15A10 10 0 0 1 12.12 6.91' fill='none' stroke='currentColor' strokeWidth='3' strokeLinecap='round' opacity='0.9' /><rect x='6' y='18' width='24' height='2' rx='1' opacity='0.4' /></>,
  speed: <><g opacity='0.85'><rect x='4' y='5' width='20' height='3' rx='1.5' /><rect x='4' y='10' width='14' height='3' rx='1.5' /><rect x='4' y='15' width='24' height='3' rx='1.5' /></g><g opacity='0.4'><rect x='26' y='5.5' width='6' height='2' rx='1' /><rect x='20' y='10.5' width='6' height='2' rx='1' /><circle cx='31' cy='16.5' r='1.5' /></g></>,
  models: <g><rect x='4' y='4' width='22' height='3' rx='1.5' opacity='0.85' /><rect x='4' y='9' width='16' height='3' rx='1.5' opacity='0.7' /><rect x='4' y='14' width='11' height='3' rx='1.5' opacity='0.55' /><rect x='4' y='19' width='7' height='2.5' rx='1.25' opacity='0.35' /></g>,
  modelMix: <><rect x='4' y='8' width='12' height='5' rx='1.5' opacity='0.85' /><rect x='17.5' y='8' width='6.5' height='5' opacity='0.5' /><rect x='25.5' y='8' width='6.5' height='5' rx='1.5' opacity='0.3' /><circle cx='5.5' cy='17.5' r='1.5' opacity='0.85' /><circle cx='11.5' cy='17.5' r='1.5' opacity='0.5' /><rect x='17' y='16.5' width='10' height='2' rx='1' opacity='0.35' /></>,
  heat: <g fill='none' stroke='currentColor' strokeLinecap='round'><path d='M6 6h26' strokeWidth='3' strokeDasharray='0.1 4.9' opacity='0.3' /><path d='M6 10.5h26' strokeWidth='3' strokeDasharray='0.1 4.9' opacity='0.55' /><path d='M6 15h26' strokeWidth='3' strokeDasharray='0.1 4.9' opacity='0.85' /><path d='M6 19.5h26' strokeWidth='3' strokeDasharray='0.1 4.9' opacity='0.3' /></g>,
  punch: <g fill='none' stroke='currentColor' strokeLinecap='round'><path d='M6 5v14' strokeWidth='3' strokeDasharray='0.1 4.4' opacity='0.3' /><path d='M11.6 5v14' strokeWidth='3' strokeDasharray='0.1 4.4' opacity='0.8' /><path d='M17.2 5v14' strokeWidth='3' strokeDasharray='0.1 4.4' opacity='0.4' /><path d='M22.8 5v14' strokeWidth='3' strokeDasharray='0.1 4.4' opacity='0.6' /><path d='M28.4 5v14' strokeWidth='3' strokeDasharray='0.1 4.4' opacity='0.3' /></g>,
  week: <><path d='M5 11v7M9.6 7v11M14.2 13v5M18.8 9v9M23.4 15v3M28 8v10M32.6 12v6' stroke='currentColor' strokeWidth='2.6' strokeLinecap='round' opacity='0.4' fill='none' /><path d='M9.6 7v11' stroke='currentColor' strokeWidth='2.6' strokeLinecap='round' opacity='0.9' fill='none' /></>,
  clock: <><circle cx='18' cy='11' r='8' fill='none' stroke='currentColor' strokeWidth='3.5' strokeDasharray='0.5 4.1' opacity='0.4' /><path d='M22 17.93A8 8 0 0 0 24.93 7' fill='none' stroke='currentColor' strokeWidth='3.5' strokeLinecap='round' opacity='0.9' /><circle cx='18' cy='11' r='1.6' opacity='0.9' /></>,
  sources: <><circle cx='18' cy='11' r='7.5' fill='none' stroke='currentColor' strokeWidth='4' opacity='0.25' /><circle cx='18' cy='11' r='7.5' fill='none' stroke='currentColor' strokeWidth='4' strokeDasharray='23 24.1' transform='rotate(-90 18 11)' opacity='0.85' /><circle cx='18' cy='11' r='7.5' fill='none' stroke='currentColor' strokeWidth='4' strokeDasharray='9 38.1' strokeDashoffset='-24' transform='rotate(-90 18 11)' opacity='0.55' /></>,
  projects: <g><circle cx='6' cy='6.5' r='1.5' opacity='0.85' /><rect x='10' y='5' width='18' height='3' rx='1.5' opacity='0.85' /><circle cx='6' cy='12' r='1.5' opacity='0.6' /><rect x='10' y='10.5' width='12' height='3' rx='1.5' opacity='0.6' /><circle cx='6' cy='17.5' r='1.5' opacity='0.4' /><rect x='10' y='16' width='8' height='3' rx='1.5' opacity='0.4' /></g>,
  sessions: <g><rect x='4' y='4' width='3' height='3' rx='1' opacity='0.6' /><rect x='9' y='4.5' width='14' height='2' rx='1' opacity='0.85' /><rect x='26' y='4.5' width='6' height='2' rx='1' opacity='0.4' /><rect x='4' y='10' width='3' height='3' rx='1' opacity='0.6' /><rect x='9' y='10.5' width='10' height='2' rx='1' opacity='0.85' /><rect x='26' y='10.5' width='6' height='2' rx='1' opacity='0.4' /><rect x='4' y='16' width='3' height='3' rx='1' opacity='0.6' /><rect x='9' y='16.5' width='17' height='2' rx='1' opacity='0.85' /><rect x='26' y='16.5' width='6' height='2' rx='1' opacity='0.4' /></g>,
  alltime: <><g opacity='0.85'><rect x='5' y='9' width='7' height='6' rx='1.5' /><rect x='14.5' y='5' width='7' height='10' rx='1.5' /><rect x='24' y='11' width='7' height='4' rx='1.5' /></g><g opacity='0.35'><rect x='5' y='17.5' width='7' height='1.5' rx='0.75' /><rect x='14.5' y='17.5' width='7' height='1.5' rx='0.75' /><rect x='24' y='17.5' width='7' height='1.5' rx='0.75' /></g></>,
  streak: <><rect x='5' y='6' width='11' height='10' rx='2.5' opacity='0.85' /><rect x='20' y='6' width='11' height='10' rx='2.5' fill='none' stroke='currentColor' strokeWidth='1.5' opacity='0.5' /><rect x='8' y='12.5' width='5' height='1.5' rx='0.75' opacity='0.5' /><rect x='23' y='12.5' width='5' height='1.5' rx='0.75' opacity='0.5' /></>,
  records: <g><rect x='4' y='4.5' width='9' height='2' rx='1' opacity='0.5' /><rect x='24' y='4.5' width='8' height='2' rx='1' opacity='0.85' /><rect x='4' y='10' width='11' height='2' rx='1' opacity='0.5' /><rect x='26' y='10' width='6' height='2' rx='1' opacity='0.85' /><rect x='4' y='15.5' width='8' height='2' rx='1' opacity='0.5' /><rect x='20' y='15.5' width='12' height='2' rx='1' opacity='0.85' /></g>,
};

function PanelThumb({ id }) {
  return <span className="panel-menu-thumb" aria-hidden="true"><svg viewBox="0 0 36 22">{PANEL_THUMBS[id] || PANEL_THUMBS.tokens}</svg></span>;
}

function FluidPanel({ panel, gridWidth, active, catalog, renderModule, meta, modular, narrow, lang, reduced, editing, onStart, onLock, onTop, onRemove }) {
  const rect = panelRect(panel, gridWidth);
  const x = useMotionValue(rect.x);
  const y = useMotionValue(rect.y);
  const width = useMotionValue(rect.width);
  const height = useMotionValue(rect.height);
  const controls = useRef([]);
  const previousGridWidth = useRef(null);
  const values = useRef({ x, y, width, height, release: null, stop: () => controls.current.forEach((control) => control.stop()) }).current;
  useLayoutEffect(() => {
    values.stop();
    if (active) return;
    const viewportResize = previousGridWidth.current !== gridWidth;
    previousGridWidth.current = gridWidth;
    const velocity = values.release || {};
    values.release = null;
    controls.current = Object.entries(rect).map(([key, target]) => {
      if (reduced || viewportResize) { values[key].set(target); return { stop() {} }; }
      return animate(values[key], target, { ...LAYOUT_SPRING, ...(velocity[key] != null ? { velocity: velocity[key] } : {}) });
    });
    return values.stop;
  }, [rect.x, rect.y, rect.width, rect.height, active, reduced, values]);
  const spec = catalog.find((item) => item.id === panel.id);
  const title = spec?.title || panel.id;
  const bare = !!spec?.bare;
  const lockTitle = lang === 0 ? '锁定/解锁拖拽' : 'Lock/unlock';
  const removeTitle = lang === 0 ? '移除面板' : 'Remove panel';
  return <motion.div data-panel-id={panel.id} className={`panel-card${active ? ` is-${active === 'move' ? 'dragging' : 'resizing'}` : ''}${panel.locked ? ' is-locked' : ''}${editing ? '' : ' is-static'}${bare && !editing ? ' is-bare' : ''}`}
    style={{ x, y, width, height }} initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={FADE}>
    {(!bare || editing) && <div className="panel-head" onPointerDown={(event) => editing && !narrow && onStart(event, panel.id, 'move', values)}>
      {!bare && <span className="panel-title" title={title}>{title}</span>}
      {!bare && <span className="panel-meta">{meta?.(panel.id)}</span>}
      {editing && <span className="panel-head-btns" onPointerDown={(event) => event.stopPropagation()}>
        <button type="button" className="panel-btn" disabled={panel.locked} onClick={() => onTop(panel.id)} aria-label={`${title} ${lang === 0 ? '移到顶部' : 'Move to top'}`} title={lang === 0 ? '移到顶部' : 'Move to top'}><svg viewBox="0 0 24 24"><path d="M12 19V5" /><path d="M5 12l7-7 7 7" /></svg></button>
        <button type="button" className={`panel-btn${panel.locked ? ' is-on' : ''}`} onClick={() => onLock(panel.id)} title={lockTitle} aria-label={lockTitle}>
          <svg viewBox="0 0 24 24"><rect x="5" y="11" width="14" height="9" rx="2" /><path d="M8 11V7a4 4 0 0 1 8 0v4" /></svg>
        </button>
        <button type="button" className="panel-btn" onClick={() => onRemove(panel.id)} title={removeTitle} aria-label={removeTitle}><svg viewBox="0 0 24 24"><path d="M6 6l12 12" /><path d="M18 6L6 18" /></svg></button>
      </span>}
    </div>}
    <ModuleContent id={panel.id} renderModule={renderModule} modular={modular} />
    {editing && !panel.locked && !narrow && <div className="panel-resize" role="slider" tabIndex={0} aria-label={`${title} ${lang === 0 ? '调整大小，方向键调整' : 'resize, use arrow keys'}`} aria-valuemin={1} aria-valuemax={12} aria-valuenow={panel.w} aria-valuetext={`${panel.w} × ${panel.h}`}
      onPointerDown={(event) => onStart(event, panel.id, 'resize', values)} onKeyDown={(event) => {
        if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
        event.preventDefault();
        onStart(null, panel.id, 'keyboard', { dw: event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0, dh: event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : 0 });
      }} />}
  </motion.div>;
}

export default function ModuleBoard({ catalog, defaults, storageKey, renderModule, lang, meta, modular = true, editing = true, hideTools = false, hiddenIds = [], onRestoreHidden }) {
  const t = (key) => STR[key][lang];
  const reduced = useMotionPreference();
  const gridRef = useRef(null);
  const [initial] = useState(() => readPanelState(globalThis.localStorage, storageKey, catalog));
  // 新增模块自动入局:旧存档里没有的 catalog 默认面板补到末尾,既有排布不动.
  const addedDefaults = initial ? defaults.filter(({ id }) => !initial.panels.some((p) => p.id === id)) : [];
  const [panels, setPanels] = useState(() => initial
    ? [...initial.panels, ...addedDefaults.map((panel) => ({ ...panel }))]
    : defaults.map((panel) => ({ ...panel })));
  const [enabled, setEnabled] = useState(() => initial
    ? [...initial.enabled, ...addedDefaults.map(({ id }) => id).filter((id) => !initial.enabled.includes(id))]
    : defaults.map(({ id }) => id));
  const panelsRef = useRef(panels);
  panelsRef.current = panels;
  const gesture = useRef(null);
  const finishRef = useRef(null);
  const [drag, setDrag] = useState(null);
  const [gridWidth, setGridWidth] = useState(920);
  const [menuOpen, setMenuOpen] = useState(false);
  useEffect(() => { savePanelState(panels, enabled, globalThis.localStorage, storageKey); }, [panels, enabled]);
  useLayoutEffect(() => {
    let measuredWidth = gridRef.current?.clientWidth || 920;
    const observer = new ResizeObserver(([entry]) => {
      if (Math.abs(entry.contentRect.width - measuredWidth) < 0.5) return;
      measuredWidth = entry.contentRect.width;
      if (gesture.current) finishRef.current?.(true);
      setGridWidth(entry.contentRect.width);
    });
    if (gridRef.current) { setGridWidth(gridRef.current.clientWidth); observer.observe(gridRef.current); }
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    if (!menuOpen) return;
    const close = (event) => { if (event.type !== 'keydown' || event.key === 'Escape') setMenuOpen(false); };
    document.addEventListener('pointerdown', close);
    document.addEventListener('keydown', close);
    return () => { document.removeEventListener('pointerdown', close); document.removeEventListener('keydown', close); };
  }, [menuOpen]);

  useEffect(() => {
    const resolve = (d, projected = false) => {
      const velocity = projected && !reduced ? releaseVelocity(d.samples, performance.now()) : { x: 0, y: 0 };
      const offsetX = projected ? Math.max(-d.stepX * 0.75, Math.min(d.stepX * 0.75, project(velocity.x))) : 0;
      const offsetY = projected ? Math.max(-d.stepY * 0.75, Math.min(d.stepY * 0.75, project(velocity.y))) : 0;
      const target = d.mode === 'move' ? { ...d.panel,
        x: Math.max(0, Math.min(PANEL_COLS - d.panel.w, Math.round((d.values.x.get() + offsetX) / d.stepX))),
        y: Math.max(0, Math.min(400, Math.round((d.values.y.get() + offsetY) / d.stepY))),
      } : { ...d.panel,
        w: Math.max(d.minW, Math.min(PANEL_COLS - d.panel.x, Math.round((d.values.width.get() + PANEL_GAP) / d.stepX))),
        h: Math.max(d.minH, Math.min(10, Math.round((d.values.height.get() + PANEL_GAP) / d.stepY))),
      };
      const key = `${target.x}:${target.y}:${target.w}:${target.h}`;
      if (key === d.candidate) return;
      d.candidate = key;
      d.preview = modular ? resolveModuleLayout(d.list, d.id, target) : d.mode === 'move' ? resolvePanelMove(d.list, d.id, target)
        : compactPanels(d.list.map((panel) => panel.id === d.id ? resolvePanelResize(d.list, d.id, target) : panel));
      d.target = d.preview.find((panel) => panel.id === d.id);
      setDrag({ id: d.id, mode: d.mode, panels: d.preview, target: d.target, align: collectAlignLines(d.preview, d.id, d.target.x, d.target.y) });
    };
    const update = () => {
      const d = gesture.current;
      if (!d) return;
      d.frame = null;
      const grid = gridRef.current?.getBoundingClientRect();
      if (!grid) return;
      const dx = d.pointer.x - d.startX + d.grid.left - grid.left;
      const dy = d.pointer.y - d.startY + d.grid.top - grid.top;
      d.moved ||= Math.hypot(dx, dy) >= 4;
      if (!d.moved) return;
      if (d.mode === 'move') {
        d.values.x.set(rubberband(d.rect.x + dx, 0, grid.width - d.rect.width));
        d.values.y.set(rubberband(d.rect.y + dy, 0, 400 * d.stepY));
      } else {
        // Real dimensions keep text crisp; only the active card needs layout during resize.
        d.values.width.set(rubberband(d.rect.width + dx, d.minW * d.stepX - PANEL_GAP, grid.width - d.panel.x * d.stepX));
        d.values.height.set(rubberband(d.rect.height + dy, d.minH * d.stepY - PANEL_GAP, 10 * d.stepY - PANEL_GAP));
      }
      resolve(d);
      const scroller = gridRef.current.closest('.main');
      let scrolling = false;
      if (scroller) {
        const box = scroller.getBoundingClientRect();
        const speed = d.pointer.y < box.top + 48 ? -Math.min(12, (box.top + 48 - d.pointer.y) / 4)
          : d.pointer.y > box.bottom - 48 ? Math.min(12, (d.pointer.y - box.bottom + 48) / 4) : 0;
        const before = scroller.scrollTop;
        if (speed) scroller.scrollTop += speed;
        scrolling = before !== scroller.scrollTop;
      }
      if (scrolling) d.frame = requestAnimationFrame(update);
    };
    const finish = (cancel = false, event) => {
      const d = gesture.current;
      if (!d || (event?.pointerId != null && event.pointerId !== d.pointerId)) return;
      if (d.frame != null) cancelAnimationFrame(d.frame);
      if (!cancel && event?.clientX != null) { d.pointer = { x: event.clientX, y: event.clientY }; update(); }
      if (d.frame != null) cancelAnimationFrame(d.frame);
      if (!cancel && d.moved) {
        if (d.mode === 'move') resolve(d, true);
        const velocity = reduced ? { x: 0, y: 0 } : releaseVelocity(d.samples, performance.now());
        d.values.release = d.mode === 'move' ? velocity : { width: velocity.x, height: velocity.y };
        setPanels(d.preview);
      } else d.values.release = { x: 0, y: 0, width: 0, height: 0 };
      gesture.current = null;
      setDrag(null);
      if (d.capture.hasPointerCapture?.(d.pointerId)) d.capture.releasePointerCapture(d.pointerId);
    };
    finishRef.current = finish;
    const move = (event) => {
      const d = gesture.current;
      if (!d || event.pointerId !== d.pointerId) return;
      d.pointer = { x: event.clientX, y: event.clientY };
      d.samples.push({ ...d.pointer, t: performance.now() });
      d.samples = d.samples.filter((sample) => performance.now() - sample.t <= 120);
      if (d.frame == null) d.frame = requestAnimationFrame(update);
    };
    const up = (event) => finish(false, event);
    const cancel = (event) => finish(true, event);
    const key = (event) => { if (event.key === 'Escape' && gesture.current) { event.preventDefault(); finish(true); } };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', cancel);
    window.addEventListener('blur', cancel);
    window.addEventListener('keydown', key);
    const lost = (event) => { if (gesture.current?.pointerId === event.pointerId) finish(true); };
    window.addEventListener('lostpointercapture', lost);
    return () => {
      finish(true);
      window.removeEventListener('pointermove', move); window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', cancel); window.removeEventListener('blur', cancel);
      window.removeEventListener('keydown', key); window.removeEventListener('lostpointercapture', lost);
    };
  }, [reduced, modular]);

  const startDrag = (event, id, mode, values) => {
    if (!editing) return;
    const panel = panelsRef.current.find((item) => item.id === id);
    if (!panel || panel.locked || gesture.current || (event && (event.button !== 0 || !event.isPrimary))) return;
    const cat = catalog.find((item) => item.id === id);
    if (mode === 'keyboard') {
      const target = { ...panel,
        w: Math.max(cat.minW || 1, Math.min(PANEL_COLS - panel.x, panel.w + values.dw)),
        h: Math.max(cat.minH || 1, Math.min(10, panel.h + values.dh)),
      };
      if (modular) { setPanels(resolveModuleLayout(panelsRef.current, id, target)); return; }
      setPanels(compactPanels(panelsRef.current.map((item) => item.id === id ? { ...item,
        w: Math.max(cat.minW || 1, Math.min(PANEL_COLS - item.x, item.w + values.dw)),
        h: Math.max(cat.minH || 1, Math.min(10, item.h + values.dh)),
      } : item)));
      return;
    }
    const grid = gridRef.current.getBoundingClientRect();
    event.preventDefault();
    values.stop();
    const rect = { x: values.x.get(), y: values.y.get(), width: values.width.get(), height: values.height.get() };
    // Capture on the stable board, so React reordering a card cannot lose the pointer.
    const capture = gridRef.current;
    capture.setPointerCapture(event.pointerId);
    gesture.current = { id, mode, panel, minW: cat.minW || 1, minH: cat.minH || 1, values, rect, capture, pointerId: event.pointerId, grid,
      startX: event.clientX, startY: event.clientY, pointer: { x: event.clientX, y: event.clientY },
      stepX: (grid.width + PANEL_GAP) / PANEL_COLS, stepY: PANEL_ROW_H + PANEL_GAP,
      list: panelsRef.current, preview: panelsRef.current, target: panel, moved: false, candidate: null, frame: null,
      samples: [{ x: event.clientX, y: event.clientY, t: performance.now() }],
    };
    setDrag({ id, mode, panels: panelsRef.current, target: panel, align: { v: [], h: [] } });
  };
  const removePanel = (id) => {
    setEnabled((items) => items.filter((item) => item !== id));
    setPanels((items) => modular ? items.filter((item) => item.id !== id) : compactPanels(items.filter((item) => item.id !== id)));
  };
  const addPanel = (id) => {
    if (enabled.includes(id)) { removePanel(id); return; }
    if (enabled.length >= PANELS_MAX) return;
    setEnabled((items) => [...items, id]);
    const cat = catalog.find((item) => item.id === id);
    const bottom = Math.max(0, ...panelsRef.current.map((panel) => panel.y + panel.h));
    setPanels((items) => {
      const next = [...items.filter((panel) => panel.id !== id), { id, x: 0, y: Math.min(400, bottom), w: cat.w, h: cat.h, locked: false }];
      return modular ? resolveModuleLayout(next, id, next.at(-1)) : compactPanels(next);
    });
  };
  const moveToTop = (id) => setPanels((items) => {
    const target = items.find((item) => item.id === id);
    if (!target || target.locked) return items;
    if (modular) return resolveModuleLayout(items, id, { ...target, x: 0, y: 0 });
    return compactPanels([{ ...target, x: 0, y: 0 }, ...items.filter((item) => item.id !== id).map((item) => item.locked ? item : { ...item, y: item.y + target.h })]);
  });
  const toggleLock = (id) => setPanels((items) => items.map((panel) => panel.id === id ? { ...panel, locked: !panel.locked } : panel));
  const allLocked = panels.length > 0 && panels.every((panel) => panel.locked);
  const resetLayout = () => { setPanels(defaults.map((panel) => ({ ...panel }))); setEnabled(defaults.map(({ id }) => id)); };
  const narrow = modular && gridWidth < 740;
  const previewById = drag ? new Map(drag.panels.map((panel) => [panel.id, panel])) : null;
  const renderList = panels.filter((panel) => enabled.includes(panel.id) && !hiddenIds.includes(panel.id)).map((panel) => previewById?.get(panel.id) || panel);
  const maxBottom = Math.max(3, ...renderList.map((panel) => panel.y + panel.h));
  const ghost = drag?.target && panelRect(drag.target, gridWidth);
  return <div className={`panel-page${modular ? ' module-board' : ''}${narrow ? ' is-narrow' : ''}${drag ? ' is-manipulating' : ''}${editing ? ' is-editing' : ' is-reading'}`}>
    {editing && !hideTools && <div className="panel-tools">
      <span className="panel-hint">{`${renderList.length}/${PANELS_MAX}`}</span>
      <button type="button" className="panel-tool" onClick={() => setPanels((items) => items.map((panel) => ({ ...panel, locked: !allLocked })))}>{allLocked ? t('panels_unlock_all') : t('panels_lock_all')}</button>
      <button type="button" className="panel-tool" onClick={resetLayout}>{t('panels_reset')}</button>
      <div className="panel-add" onPointerDown={(event) => event.stopPropagation()}>
        <button type="button" className="panel-tool panel-tool-primary" aria-expanded={menuOpen} aria-controls="panel-add-menu" onClick={() => setMenuOpen((open) => !open)}>+ {t('panels_add')}</button>
        <AnimatePresence>{menuOpen && <PopoverSurface id="panel-add-menu" className="panel-menu">
          {catalog.map((cat) => <button key={cat.id} type="button" className={`panel-menu-item${enabled.includes(cat.id) ? ' is-on' : ''}`} aria-pressed={enabled.includes(cat.id) && !hiddenIds.includes(cat.id)} onClick={() => (hiddenIds.includes(cat.id) ? onRestoreHidden?.(cat.id) : addPanel(cat.id))} disabled={!hiddenIds.includes(cat.id) && !enabled.includes(cat.id) && enabled.length >= PANELS_MAX}>
            <PanelThumb id={cat.id} />
            <span className="panel-menu-copy"><b>{cat.title}</b><small>{hiddenIds.includes(cat.id) ? (lang === 0 ? '已隐藏 · 点击恢复' : 'Hidden · click to restore') : cat.description}</small></span><i className={`panel-menu-dot${enabled.includes(cat.id) ? ' is-on' : ''}`} />
          </button>)}
        </PopoverSurface>}</AnimatePresence>
      </div>
    </div>}
    <div ref={gridRef} className="panel-grid" style={{ height: narrow ? undefined : maxBottom * (PANEL_ROW_H + PANEL_GAP) - PANEL_GAP }}>
      {drag?.align.v.map((x) => <i key={`v${x}`} className="panel-align-v" style={{ left: x * (gridWidth + PANEL_GAP) / PANEL_COLS }} />)}
      {drag?.align.h.map((y) => <i key={`h${y}`} className="panel-align-h" style={{ top: y * (PANEL_ROW_H + PANEL_GAP) }} />)}
      {ghost && <motion.div className="panel-ghost" initial={false} animate={{ x: ghost.x, y: ghost.y, width: ghost.width, height: ghost.height }} transition={reduced ? { duration: 0 } : LAYOUT_SPRING} />}
      <AnimatePresence initial={false}>{gridWidth != null && renderList.map((panel) => <FluidPanel key={panel.id} panel={panel} gridWidth={gridWidth} active={drag?.id === panel.id ? drag.mode : null} catalog={catalog} renderModule={renderModule} meta={meta} modular={modular} narrow={narrow} lang={lang} reduced={reduced} editing={editing} onStart={startDrag} onLock={toggleLock} onTop={moveToTop} onRemove={removePanel} />)}</AnimatePresence>
      {renderList.length === 0 && <div className="panel-empty">{t('no_data')}</div>}
    </div>
  </div>;
}
