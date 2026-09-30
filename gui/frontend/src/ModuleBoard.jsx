import React, { memo, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { AnimatePresence, animate, motion, useMotionValue } from 'motion/react';
import { STR } from './ui-data.js';

import { PANEL_COLS, PANEL_ROW_H, PANEL_GAP, PANELS_MAX, compactPanels, resolveModuleLayout, resolvePanelMove, resolvePanelResize, collectAlignLines, readPanelState, savePanelState } from './panel-layout.js';
import { FADE, LAYOUT_SPRING, panelRect, project, releaseVelocity, rubberband } from './motion-tuning.js';
import { PopoverSurface, useMotionPreference } from './ui-motion.jsx';

const ModuleContent = memo(function ModuleContent({ id, renderModule, modular }) {
  return <div className={`panel-body${modular ? ' module-body' : ''}`}>{renderModule(id)}</div>;
});

function FluidPanel({ panel, gridWidth, active, catalog, renderModule, meta, modular, narrow, lang, reduced, onStart, onLock, onTop, onRemove }) {
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
  const title = catalog.find((item) => item.id === panel.id)?.title || panel.id;
  const lockTitle = lang === 0 ? '锁定/解锁拖拽' : 'Lock/unlock';
  const removeTitle = lang === 0 ? '移除面板' : 'Remove panel';
  return <motion.div data-panel-id={panel.id} className={`panel-card${active ? ` is-${active === 'move' ? 'dragging' : 'resizing'}` : ''}${panel.locked ? ' is-locked' : ''}`}
    style={{ x, y, width, height }} initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={FADE}>
    <div className="panel-head" onPointerDown={(event) => !narrow && onStart(event, panel.id, 'move', values)}>
      <span className="panel-title" title={title}>{title}</span>
      <span className="panel-meta">{meta?.(panel.id)}</span>
      <span className="panel-head-btns" onPointerDown={(event) => event.stopPropagation()}>
        <button type="button" className="panel-btn" disabled={panel.locked} onClick={() => onTop(panel.id)} aria-label={`${title} ${lang === 0 ? '移到顶部' : 'Move to top'}`} title={lang === 0 ? '移到顶部' : 'Move to top'}><svg viewBox="0 0 24 24"><path d="M12 19V5" /><path d="M5 12l7-7 7 7" /></svg></button>
        <button type="button" className={`panel-btn${panel.locked ? ' is-on' : ''}`} onClick={() => onLock(panel.id)} title={lockTitle} aria-label={lockTitle}>
          <svg viewBox="0 0 24 24"><rect x="5" y="11" width="14" height="9" rx="2" /><path d="M8 11V7a4 4 0 0 1 8 0v4" /></svg>
        </button>
        <button type="button" className="panel-btn" onClick={() => onRemove(panel.id)} title={removeTitle} aria-label={removeTitle}><svg viewBox="0 0 24 24"><path d="M6 6l12 12" /><path d="M18 6L6 18" /></svg></button>
      </span>
    </div>
    <ModuleContent id={panel.id} renderModule={renderModule} modular={modular} />
    {!panel.locked && !narrow && <div className="panel-resize" role="slider" tabIndex={0} aria-label={`${title} ${lang === 0 ? '调整大小，方向键调整' : 'resize, use arrow keys'}`} aria-valuemin={1} aria-valuemax={12} aria-valuenow={panel.w} aria-valuetext={`${panel.w} × ${panel.h}`}
      onPointerDown={(event) => onStart(event, panel.id, 'resize', values)} onKeyDown={(event) => {
        if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
        event.preventDefault();
        onStart(null, panel.id, 'keyboard', { dw: event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0, dh: event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : 0 });
      }} />}
  </motion.div>;
}

export default function ModuleBoard({ catalog, defaults, storageKey, renderModule, lang, meta, modular = true, hiddenIds = [] }) {
  const t = (key) => STR[key][lang];
  const reduced = useMotionPreference();
  const gridRef = useRef(null);
  const [initial] = useState(() => readPanelState(globalThis.localStorage, storageKey, catalog));
  const [panels, setPanels] = useState(() => initial?.panels || defaults.map((panel) => ({ ...panel })));
  const [enabled, setEnabled] = useState(() => initial?.enabled || defaults.map(({ id }) => id));
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
  return <div className={`panel-page${modular ? ' module-board' : ''}${narrow ? ' is-narrow' : ''}${drag ? ' is-manipulating' : ''}`}>
    <div className="panel-tools">
      <span className="panel-hint">{`${renderList.length}/${PANELS_MAX}`}</span>
      <button type="button" className="panel-tool" onClick={() => setPanels((items) => items.map((panel) => ({ ...panel, locked: !allLocked })))}>{allLocked ? t('panels_unlock_all') : t('panels_lock_all')}</button>
      <button type="button" className="panel-tool" onClick={resetLayout}>{t('panels_reset')}</button>
      <div className="panel-add" onPointerDown={(event) => event.stopPropagation()}>
        <button type="button" className="panel-tool panel-tool-primary" aria-expanded={menuOpen} aria-controls="panel-add-menu" onClick={() => setMenuOpen((open) => !open)}>+ {(lang === 0 ? '选择模块' : 'Choose modules')}</button>
        <AnimatePresence>{menuOpen && <PopoverSurface id="panel-add-menu" className="panel-menu">
          {catalog.map((cat) => <button key={cat.id} type="button" className={`panel-menu-item${enabled.includes(cat.id) ? ' is-on' : ''}`} aria-pressed={enabled.includes(cat.id) && !hiddenIds.includes(cat.id)} onClick={() => addPanel(cat.id)} disabled={hiddenIds.includes(cat.id) || (!enabled.includes(cat.id) && enabled.length >= PANELS_MAX)}>
            <span className="panel-menu-copy"><b>{cat.title}</b><small>{hiddenIds.includes(cat.id) ? (lang === 0 ? '在设置中恢复此套餐' : 'Restore in Settings') : cat.description}</small></span><i className={`panel-menu-dot${enabled.includes(cat.id) ? ' is-on' : ''}`} />
          </button>)}
        </PopoverSurface>}</AnimatePresence>
      </div>
    </div>
    <div ref={gridRef} className="panel-grid" style={{ height: narrow ? undefined : maxBottom * (PANEL_ROW_H + PANEL_GAP) - PANEL_GAP }}>
      {drag?.align.v.map((x) => <i key={`v${x}`} className="panel-align-v" style={{ left: x * (gridWidth + PANEL_GAP) / PANEL_COLS }} />)}
      {drag?.align.h.map((y) => <i key={`h${y}`} className="panel-align-h" style={{ top: y * (PANEL_ROW_H + PANEL_GAP) }} />)}
      {ghost && <motion.div className="panel-ghost" initial={false} animate={{ x: ghost.x, y: ghost.y, width: ghost.width, height: ghost.height }} transition={reduced ? { duration: 0 } : LAYOUT_SPRING} />}
      <AnimatePresence initial={false}>{gridWidth != null && renderList.map((panel) => <FluidPanel key={panel.id} panel={panel} gridWidth={gridWidth} active={drag?.id === panel.id ? drag.mode : null} catalog={catalog} renderModule={renderModule} meta={meta} modular={modular} narrow={narrow} lang={lang} reduced={reduced} onStart={startDrag} onLock={toggleLock} onTop={moveToTop} onRemove={removePanel} />)}</AnimatePresence>
      {renderList.length === 0 && <div className="panel-empty">{t('no_data')}</div>}
    </div>
  </div>;
}
