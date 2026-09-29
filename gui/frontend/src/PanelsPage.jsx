import React, { useEffect, useRef, useState } from 'react';
import { STR } from './ui-data.js';
import { PANEL_BODIES, PanelErrorBoundary, panelMeta } from './PanelBodies.jsx';
import { PANEL_COLS, PANEL_ROW_H, PANEL_GAP, PANELS_MAX, PANEL_CATALOG, DEFAULT_PANELS, panelOverlap, panelOverlapArea, compactPanels, resolvePanelMove, resolvePanelResize, collectAlignLines, readPanelState, savePanelState } from './panel-layout.js';

export default function PanelsPage({ ledger, lang }) {
  const t = (k) => STR[k][lang];
  const gridRef = useRef(null);
  const panelsRef = useRef(null);
  const dragRef = useRef(null);
  const [panels, setPanels] = useState(() => readPanelState()?.panels || DEFAULT_PANELS.map((p) => ({ ...p })));
  const [enabled, setEnabled] = useState(() => readPanelState()?.enabled || PANEL_CATALOG.map((c) => c.id));
  const [drag, setDrag] = useState(null);
  const [align, setAlign] = useState({ v: [], h: [] });
  const [menuOpen, setMenuOpen] = useState(false);
  useEffect(() => { panelsRef.current = panels; }, [panels]);
  useEffect(() => {
    savePanelState(panels, enabled);
  }, [panels, enabled]);
  useEffect(() => {
    if (!menuOpen) return;
    const close = () => setMenuOpen(false);
    document.addEventListener('click', close);
    return () => document.removeEventListener('click', close);
  }, [menuOpen]);

  const startDrag = (e, id, mode) => {
    const list = panelsRef.current || [];
    const panel = list.find((p) => p.id === id);
    if (!panel || (mode === 'move' && panel.locked)) return;
    e.preventDefault();
    const grid = gridRef.current?.getBoundingClientRect();
    if (!grid) return;
    const colW = (grid.width - (PANEL_COLS - 1) * PANEL_GAP) / PANEL_COLS;
    dragRef.current = { id, mode, panel, startX: e.clientX, startY: e.clientY, colW, stepY: PANEL_ROW_H + PANEL_GAP };
    setDrag({ id, mode, dx: 0, dy: 0, swapWith: null, w: panel.w, h: panel.h, nx: panel.x, ny: panel.y });
  };

  useEffect(() => {
    if (!drag) return;
    const onMove = (e) => {
      const d = dragRef.current;
      if (!d) return;
      const list = panelsRef.current || [];
      const dx = e.clientX - d.startX;
      const dy = e.clientY - d.startY;
      if (d.mode === 'move') {
        // 指针所在格位;拖拽卡跟手浮起,其余卡不动,仅当重叠时让一张卡平移到原位(iOS 式让位)
        const nx = Math.max(0, Math.min(PANEL_COLS - d.panel.w, d.panel.x + Math.round(dx / (d.colW + PANEL_GAP))));
        const ny = Math.max(0, d.panel.y + Math.round(dy / d.stepY));
        const target = { ...d.panel, x: nx, y: ny };
        const overlapping = list.filter((p) => p.id !== d.id && !p.locked && panelOverlap(target, p));
        const swapWith = overlapping.length ? overlapping.reduce((best, p) => (panelOverlapArea(target, p) > panelOverlapArea(target, best) ? p : best), overlapping[0]).id : null;
        setAlign(collectAlignLines(list, d.id, nx, ny));
        setDrag((cur) => (cur ? { ...cur, dx, dy, nx, ny, swapWith } : cur));
      } else {
        const nw = Math.max(1, Math.min(PANEL_COLS - d.panel.x, d.panel.w + Math.round(dx / (d.colW + PANEL_GAP))));
        const nh = Math.max(1, Math.min(10, d.panel.h + Math.round(dy / d.stepY)));
        setDrag((cur) => (cur ? { ...cur, w: nw, h: nh } : cur));
      }
    };
    const onUp = () => {
      const d = dragRef.current;
      const cur = drag;
      if (d && cur) {
        const list = panelsRef.current || [];
        if (d.mode === 'move') {
          const target = { ...d.panel, x: cur.nx, y: cur.ny };
          setPanels(resolvePanelMove(list, d.id, target));
        } else {
          const target = resolvePanelResize(list, d.id, { ...d.panel, w: cur.w, h: cur.h });
          setPanels(compactPanels(list.map((p) => (p.id === d.id ? target : p))));
        }
      }
      dragRef.current = null;
      setDrag(null);
      setAlign({ v: [], h: [] });
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  }, [drag]);

  const removePanel = (id) => {
    setEnabled((es) => es.filter((x) => x !== id));
    setPanels((ps) => ps.filter((p) => p.id !== id));
  };
  const addPanel = (id) => {
    if (enabled.includes(id)) { removePanel(id); return; }
    if (enabled.length >= PANELS_MAX) return;
    setEnabled((es) => [...es, id]);
    const cat = PANEL_CATALOG.find((c) => c.id === id);
    const maxBottom = Math.max(0, ...(panelsRef.current || []).map((p) => p.y + p.h));
    setPanels((ps) => compactPanels([...ps.filter((p) => p.id !== id), { id, x: 0, y: maxBottom, w: cat.w, h: cat.h, locked: false }]));
  };
  const toggleLock = (id) => setPanels((ps) => ps.map((p) => (p.id === id ? { ...p, locked: !p.locked } : p)));
  const allLocked = panels.length > 0 && panels.every((p) => p.locked);
  const toggleAllLock = () => setPanels((ps) => ps.map((p) => ({ ...p, locked: !allLocked })));
  const resetLayout = () => {
    setPanels(DEFAULT_PANELS.map((p) => ({ ...p })));
    setEnabled(PANEL_CATALOG.map((c) => c.id));
  };

  const origin = drag ? panels.find((p) => p.id === drag.id) : null;
  const renderList = panels
    .filter((p) => enabled.includes(p.id))
    .map((p) => {
      if (drag?.mode === 'move' && drag.swapWith && p.id === drag.swapWith && origin) {
        return { ...p, x: origin.x, y: origin.y };
      }
      if (drag?.mode === 'resize' && p.id === drag.id) {
        return { ...p, w: drag.w, h: drag.h };
      }
      return p;
    });
  const bottomPanels = drag?.mode === 'move' && origin ? renderList.filter((p) => p.id !== drag.id) : renderList;
  const maxBottom = Math.max(3, ...renderList.map((p) => p.y + p.h));
  const lockTitle = lang === 0 ? '锁定/解锁拖拽' : 'Lock/unlock';
  const removeTitle = lang === 0 ? '移除面板' : 'Remove panel';
  const posStyle = (p) => ({
    '--x': p.x, '--y': p.y, '--w': p.w, '--h': p.h,
  });
  return (
    <div className="panel-page">
      <div className="panel-tools">
        <span className="panel-hint">{`${renderList.length}/${PANELS_MAX}`}</span>
        <button type="button" className="panel-tool" onClick={toggleAllLock}>{allLocked ? t('panels_unlock_all') : t('panels_lock_all')}</button>
        <button type="button" className="panel-tool" onClick={resetLayout}>{t('panels_reset')}</button>
        <div className="panel-add" onClick={(e) => e.stopPropagation()}>
          <button type="button" className="panel-tool panel-tool-primary" onClick={() => setMenuOpen((o) => !o)}>+ {t('panels_add')}</button>
          {menuOpen && (
            <div className="panel-menu">
              {PANEL_CATALOG.map((c) => {
                const on = enabled.includes(c.id);
                return (
                  <button key={c.id} type="button" className={`panel-menu-item${on ? ' is-on' : ''}`} onClick={() => addPanel(c.id)} disabled={!on && enabled.length >= PANELS_MAX}>
                    <span className="panel-menu-copy"><b>{t(`panel_${c.id}`)}</b><small>{t(`catalog_${c.id}`)}</small></span>
                    <i className={`panel-menu-dot${on ? ' is-on' : ''}`} />
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </div>
      <div ref={gridRef} className="panel-grid" style={{ height: maxBottom * PANEL_ROW_H + (maxBottom - 1) * PANEL_GAP }}>
        {align.v.map((v, i) => <i key={`v${i}`} className="panel-align-v" style={{ left: `calc(${(v / PANEL_COLS) * 100}% + ${v * PANEL_GAP}px)` }} />)}
        {align.h.map((y, i) => <i key={`h${i}`} className="panel-align-h" style={{ top: y * (PANEL_ROW_H + PANEL_GAP) }} />)}
        {drag?.mode === 'move' && origin && <div className="panel-ghost" style={posStyle(origin)} />}
        {bottomPanels.map((p) => {
          const Body = PANEL_BODIES[p.id];
          return (
            <div key={p.id} className={`panel-card${drag?.mode === 'resize' && drag.id === p.id ? ' is-resizing' : ''}${p.locked ? ' is-locked' : ''}`} style={posStyle(p)}>
              <div className="panel-head" onPointerDown={(e) => startDrag(e, p.id, 'move')}>
                <span className="panel-title">{t(`panel_${p.id}`)}</span>
                <span className="panel-meta">{panelMeta(p.id, ledger, lang)}</span>
                <span className="panel-head-btns" onPointerDown={(e) => e.stopPropagation()}>
                  <button type="button" className={`panel-btn${p.locked ? ' is-on' : ''}`} onClick={() => toggleLock(p.id)} title={lockTitle} aria-label={lockTitle}>
                    <svg viewBox="0 0 24 24"><rect x="5" y="11" width="14" height="9" rx="2" /><path d="M8 11V7a4 4 0 0 1 8 0v4" /></svg>
                  </button>
                  <button type="button" className="panel-btn" onClick={() => removePanel(p.id)} title={removeTitle} aria-label={removeTitle}>×</button>
                </span>
              </div>
              <div className="panel-body"><PanelErrorBoundary>{ledger ? <Body ledger={ledger} lang={lang} /> : null}</PanelErrorBoundary></div>
              <div className="panel-resize" onPointerDown={(e) => startDrag(e, p.id, 'resize')} aria-hidden="true" />
            </div>
          );
        })}
        {drag && origin && (() => {
          const Body = PANEL_BODIES[origin.id];
          return (
            <div className={`panel-card is-dragging${drag.mode === 'resize' ? ' is-resizing' : ''}`} style={{ ...posStyle(origin), transform: `translate(${drag.dx}px, ${drag.dy}px) scale(1.025)` }}>
              <div className="panel-head">
                <span className="panel-title">{t(`panel_${origin.id}`)}</span>
                <span className="panel-meta">{panelMeta(origin.id, ledger, lang)}</span>
              </div>
              <div className="panel-body"><PanelErrorBoundary>{ledger ? <Body ledger={ledger} lang={lang} /> : null}</PanelErrorBoundary></div>
            </div>
          );
        })()}
        {renderList.length === 0 && <div className="panel-empty">{t('no_data')}</div>}
      </div>
    </div>
  );
}
