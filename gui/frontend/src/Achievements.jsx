// 成就墙:面板(用量页模块) + 全屏弹窗 + 解锁 toast。
// 数据来自 achievements.js 的纯函数评估;本文件只做渲染与动效。
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { STR, fmtInt } from './ui-data.js';
import { applyUnlocks, evaluateAchievements, readAchievementState, saveAchievementState } from './achievements.js';
import { CONTROL_SPRING, FADE, LAYOUT_SPRING } from './motion-tuning.js';
import { useMotionPreference } from './ui-motion.jsx';

// 统一 24×24 描边图形;填充件用 currentColor,线条件走 stroke。
const GLYPHS = {
  spark: <path d="M12 2.6 13.9 10.1 21.4 12 13.9 13.9 12 21.4 10.1 13.9 2.6 12 10.1 10.1Z" fill="currentColor" stroke="none" />,
  bolt: <path d="M13.4 2 5 13.4h5.4L9 22l8.6-11.6h-5.4z" fill="currentColor" stroke="none" />,
  layers: <><path d="M12 2.8 21 8l-9 5.2L3 8Z" fill="currentColor" stroke="none" opacity="0.9" /><path d="m3.6 12.4 8.4 4.9 8.4-4.9" stroke="currentColor" strokeWidth="1.6" fill="none" strokeLinecap="round" /><path d="m3.6 16.3 8.4 4.9 8.4-4.9" stroke="currentColor" strokeWidth="1.6" fill="none" strokeLinecap="round" opacity="0.55" /></>,
  flame: <path d="M12 2.5c2.8 3.7 5.2 6.3 5.2 10.2a5.2 5.2 0 0 1-10.4 0c0-2.1 1-3.7 2.6-5.3.4 1.6 1.4 2.2 2.5 2.7-.6-2.4-1.4-5.1.1-7.6Z" fill="currentColor" stroke="none" />,
  sun: <><circle cx="12" cy="12" r="4.2" fill="currentColor" stroke="none" /><path d="M12 2.4v2.2M12 19.4v2.2M2.4 12h2.2M19.4 12h2.2M5.2 5.2l1.6 1.6M17.2 17.2l1.6 1.6M18.8 5.2l-1.6 1.6M6.8 17.2l-1.6 1.6" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></>,
  sunrise: <><path d="M4 18h16" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /><path d="M7.4 14.2a4.6 4.6 0 0 1 9.2 0Z" fill="currentColor" stroke="none" /><path d="M12 3.6v2.4M4.4 7l1.7 1.7M19.6 7l-1.7 1.7" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></>,
  moon: <path d="M20.4 14.6A8.8 8.8 0 1 1 9.4 3.6a7.2 7.2 0 0 0 11 11Z" fill="currentColor" stroke="none" />,
  star: <path d="M12 2.8 14.9 9l6.6.6-5 4.4 1.5 6.5L12 17l-6 3.5L7.5 14l-5-4.4L9.1 9Z" fill="currentColor" stroke="none" />,
  diamond: <><path d="M12 2.4 20.6 12 12 21.6 3.4 12Z" fill="currentColor" stroke="none" opacity="0.9" /><path d="M7.2 12h9.6M12 6.4 12 17.6" stroke="var(--ach-glyph-cut, rgba(0,0,0,0.35))" strokeWidth="1.2" /></>,
  stack: <><rect x="3.4" y="13.4" width="17.2" height="6.4" rx="1.8" fill="currentColor" stroke="none" opacity="0.55" /><rect x="3.4" y="8" width="17.2" height="6.4" rx="1.8" fill="currentColor" stroke="none" opacity="0.78" /><rect x="3.4" y="2.6" width="17.2" height="6.4" rx="1.8" fill="currentColor" stroke="none" /></>,
  compass: <><circle cx="12" cy="12" r="8.6" stroke="currentColor" strokeWidth="1.6" fill="none" /><path d="m15.4 8.6-2 4.8-4.8 2 2-4.8Z" fill="currentColor" stroke="none" /></>,
  gauge: <><path d="M4.2 17.6a9 9 0 1 1 15.6 0" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" /><path d="M12 14.4 16.2 9" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" /><circle cx="12" cy="14.8" r="1.8" fill="currentColor" stroke="none" /></>,
  rocket: <><path d="M12 2.6c3.1 2.6 4.7 6.1 4.7 9.7L12 16.9l-4.7-4.6c0-3.6 1.6-7.1 4.7-9.7Z" fill="currentColor" stroke="none" /><path d="M9.6 16.4 7.4 21l3.1-1.5L12 21l1.5-1.5L16.6 21l-2.2-4.6" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" fill="none" /></>,
  wave: <path d="M3 7.6l3.6 4.4L3 16.4M9.4 7.6l3.6 4.4-3.6 4.4M15.8 7.6l3.6 4.4-3.6 4.4" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" strokeLinejoin="round" />,
  crown: <path d="M3.4 18.6h17.2l1.6-9.8-5.4 3.6L12 5.6 7.2 12.4 1.8 8.8Z" fill="currentColor" stroke="none" />,
};

export function AchievementGlyph({ glyph, className = '' }) {
  return <svg className={`ach-glyph ${className}`} viewBox="0 0 24 24" aria-hidden="true">{GLYPHS[glyph] || GLYPHS.spark}</svg>;
}

function progressText(item, lang) {
  return item.value >= item.target ? '' : `${fmtInt(Math.round(item.value))} / ${fmtInt(item.target)}`;
}

export function AchievementsPanel({ ledger, lang, onOpen }) {
  const t = (key) => STR[key][lang];
  const items = useMemo(() => evaluateAchievements(ledger), [ledger]);
  const unlocked = items.filter((item) => item.unlocked);
  const next = items.filter((item) => !item.unlocked).sort((a, b) => b.pct - a.pct).slice(0, Math.max(0, 6 - unlocked.length));
  const wall = [...unlocked, ...next].slice(0, 12);
  return (
    <div className="ach-panel">
      <div className="ach-panel-head">
        <span className="ach-count"><b>{unlocked.length}</b> / {items.length} {t('ach_unlocked')}</span>
        <button type="button" className="ach-open" onClick={onOpen}>{t('ach_view_all')} →</button>
      </div>
      <div className="ach-wall">
        {wall.map((item) => (
          <div className={`ach-tile is-${item.tier}${item.unlocked ? ' is-unlocked' : ' is-locked'}`} key={item.id}>
            <span className="ach-tile-glyph"><AchievementGlyph glyph={item.glyph} /></span>
            <span className="ach-tile-copy">
              <b>{item.title[lang]}</b>
              <small>{item.unlocked ? t('ach_done') : progressText(item, lang) || item.hint[lang]}</small>
            </span>
            {!item.unlocked && <i className="ach-tile-track"><motion.i initial={false} animate={{ transform: `scaleX(${item.pct})` }} transition={LAYOUT_SPRING} /></i>}
          </div>
        ))}
      </div>
    </div>
  );
}

export function AchievementsModal({ ledger, lang, state, onClose }) {
  const t = (key) => STR[key][lang];
  const items = useMemo(() => evaluateAchievements(ledger), [ledger]);
  const unlocked = items.filter((item) => item.unlocked).length;
  const dateText = (id) => {
    const at = state?.unlocked?.[id];
    if (!at) return '';
    const date = new Date(at);
    return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
  };
  useEffect(() => {
    const onKey = (event) => { if (event.key === 'Escape') onClose(); };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);
  return (
    <motion.div className="ach-backdrop" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={FADE} onClick={onClose}>
      <motion.section
        className="ach-modal"
        role="dialog"
        aria-modal="true"
        aria-label={t('ach_title')}
        initial={{ opacity: 0, transform: 'translateY(10px) scale(0.985)' }}
        animate={{ opacity: 1, transform: 'translateY(0px) scale(1)' }}
        exit={{ opacity: 0, transform: 'translateY(10px) scale(0.985)' }}
        transition={CONTROL_SPRING}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="ach-modal-head">
          <div>
            <span className="eyebrow">// {t('ach_title')}</span>
            <h2>{unlocked} <span>/ {items.length}</span></h2>
            <p>{t('ach_modal_hint')}</p>
          </div>
          <button type="button" className="ach-close" onClick={onClose} aria-label={t('ach_close')}>×</button>
        </header>
        <div className="ach-grid">
          {items.map((item) => (
            <div className={`ach-card is-${item.tier}${item.unlocked ? ' is-unlocked' : ' is-locked'}`} key={item.id}>
              <span className="ach-card-glyph"><AchievementGlyph glyph={item.glyph} /></span>
              <div className="ach-card-copy">
                <b>{item.title[lang]}</b>
                <p>{item.hint[lang]}</p>
                {item.unlocked ? <small className="ach-card-date">{t('ach_done')}{dateText(item.id) ? ` · ${dateText(item.id)}` : ''}</small> : (
                  <span className="ach-card-progress"><i><motion.i initial={false} animate={{ transform: `scaleX(${item.pct})` }} transition={LAYOUT_SPRING} /></i><em>{Math.round(item.pct * 100)}%</em></span>
                )}
              </div>
            </div>
          ))}
        </div>
      </motion.section>
    </motion.div>
  );
}

export function AchievementToasts({ items, lang, onDismiss }) {
  const reduced = useMotionPreference();
  return <div className="ach-toasts" role="status" aria-live="polite">
    <AnimatePresence>
      {items.map((toast) => (
        <motion.div
          key={toast.key}
          className={`ach-toast is-${toast.tier}`}
          initial={{ opacity: 0, transform: reduced ? 'none' : 'translateY(14px) scale(0.96)' }}
          animate={{ opacity: 1, transform: 'translateY(0px) scale(1)' }}
          exit={{ opacity: 0, transform: reduced ? 'none' : 'translateY(10px) scale(0.98)' }}
          transition={reduced ? FADE : { ...CONTROL_SPRING, bounce: 0.22 }}
          onClick={() => onDismiss(toast.key)}
        >
          <span className="ach-toast-glyph"><AchievementGlyph glyph={toast.glyph} /></span>
          <span className="ach-toast-copy">
            <em>{toast.multi ? STR.ach_multi[lang] : STR.ach_new[lang]}</em>
            <b>{toast.label}</b>
          </span>
        </motion.div>
      ))}
    </AnimatePresence>
  </div>;
}

// 壳层 hook:评估账本 → 与存档比对 → 新解锁进队列(>3 个合并成一条,避免首跑刷屏)。
export function useAchievements(ledger, lang = 0) {
  const [state, setState] = useState(readAchievementState);
  const [toasts, setToasts] = useState([]);
  const counter = useRef(0);
  const items = useMemo(() => evaluateAchievements(ledger), [ledger]);
  useEffect(() => {
    if (!ledger) return;
    setState((current) => {
      const result = applyUnlocks(items, current);
      if (result.fresh.length === 0) return current;
      saveAchievementState(result.state);
      if (result.fresh.length > 3) {
        counter.current += 1;
        const key = `multi-${counter.current}`;
        setToasts((list) => [...list.slice(-2), { key, multi: true, label: STR.ach_multi_label[lang], tier: 'gold', glyph: 'crown' }]);
      } else {
        for (const id of result.fresh) {
          const item = items.find((entry) => entry.id === id);
          if (!item) continue;
          counter.current += 1;
          setToasts((list) => [...list.slice(-2), { key: `${id}-${counter.current}`, label: item.title[lang], tier: item.tier, glyph: item.glyph }]);
        }
      }
      return result.state;
    });
  }, [items, ledger, lang]);
  useEffect(() => {
    if (toasts.length === 0) return;
    const timer = window.setTimeout(() => setToasts((list) => list.slice(1)), 6500);
    return () => window.clearTimeout(timer);
  }, [toasts]);
  const dismiss = (key) => setToasts((list) => list.filter((toast) => toast.key !== key));
  return { state, items, toasts, dismiss };
}
